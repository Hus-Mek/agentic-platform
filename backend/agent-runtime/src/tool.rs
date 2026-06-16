use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use agenticos_core::udm::{AgentEvent, EventResult, EventType, Observable};
use agenticos_core::state::ToolCall;

pub type ToolHandler = Arc<
    dyn Fn(serde_json::Value) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send>>
        + Send
        + Sync,
>;

#[derive(Debug, Clone)]
pub struct ToolMetadata {
    pub requires_approval: bool,
    pub retryable: bool,
    pub timeout_secs: u64,
}

impl Default for ToolMetadata {
    fn default() -> Self {
        Self {
            requires_approval: false,
            retryable: true,
            timeout_secs: 30,
        }
    }
}

pub struct ToolImpl {
    pub name: String,
    pub description: String,
    pub schema: serde_json::Value,
    pub handler: ToolHandler,
    pub metadata: ToolMetadata,
}

pub struct ToolRegistry {
    tools: HashMap<String, ToolImpl>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            tools: HashMap::new(),
        };
        reg.register_builtins();
        reg
    }

    #[cfg(test)]
    pub fn empty() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(
        &mut self,
        name: &str,
        description: &str,
        schema: serde_json::Value,
        handler: ToolHandler,
    ) {
        self.tools.insert(
            name.to_string(),
            ToolImpl {
                name: name.to_string(),
                description: description.to_string(),
                schema,
                handler,
                metadata: ToolMetadata::default(),
            },
        );
    }

    pub fn register_with_metadata(
        &mut self,
        name: &str,
        description: &str,
        schema: serde_json::Value,
        handler: ToolHandler,
        metadata: ToolMetadata,
    ) {
        self.tools.insert(
            name.to_string(),
            ToolImpl {
                name: name.to_string(),
                description: description.to_string(),
                schema,
                handler,
                metadata,
            },
        );
    }

    pub fn get(&self, name: &str) -> Option<&ToolImpl> {
        self.tools.get(name)
    }

    pub fn list(&self) -> Vec<&ToolImpl> {
        self.tools.values().collect()
    }

    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    pub async fn execute(
        &self,
        tool_call: &mut ToolCall,
        workspace_id: Option<&str>,
        run_id: Option<&str>,
        agent_id: Option<&str>,
    ) -> anyhow::Result<String> {
        let tool = self
            .tools
            .get(&tool_call.name)
            .ok_or_else(|| anyhow::anyhow!("Unknown tool: {}", tool_call.name))?;

        // Validate arguments against schema
        if let Err(validation_err) = self.validate_tool_args(&tool_call.name, &tool_call.args) {
            tracing::error!(
                tool = %tool_call.name,
                error = %validation_err,
                "Tool argument validation failed"
            );
            return Err(anyhow::anyhow!("Tool validation error: {}", validation_err));
        }

        AgentEvent::new(
            EventType::ToolCallInitiated,
            workspace_id.map(|s| s.to_string()),
            run_id.map(|s| s.to_string()),
            agent_id.map(|s| s.to_string()),
            None,
        )
        .with_observable(Observable::tool_call(&tool_call.name, &tool_call.args))
        .emit();

        let start = Instant::now();
        let timeout_duration = std::time::Duration::from_secs(tool.metadata.timeout_secs);
        let result = match tokio::time::timeout(
            timeout_duration,
            (tool.handler)(tool_call.args.clone()),
        )
        .await
        {
            Ok(inner) => inner,
            Err(_) => anyhow::bail!(
                "Tool '{}' timed out after {}s",
                tool_call.name,
                tool.metadata.timeout_secs
            ),
        };
        let duration_ms = start.elapsed().as_millis() as i64;
        tool_call.duration_ms = Some(duration_ms);

        match result {
            Ok(output) => {
                tool_call.result = Some(output.clone());
                AgentEvent::new(
                    EventType::ToolCallCompleted,
                    workspace_id.map(|s| s.to_string()),
                    run_id.map(|s| s.to_string()),
                    agent_id.map(|s| s.to_string()),
                    None,
                )
                .with_result(
                    EventResult::success()
                        .with_duration(duration_ms),
                )
                .with_observable(Observable::tool_call(&tool_call.name, &tool_call.args))
                .emit();
                Ok(output)
            }
            Err(e) => {
                let err_msg = e.to_string();
                tool_call.error = Some(err_msg.clone());
                AgentEvent::new(
                    EventType::ToolCallFailed,
                    workspace_id.map(|s| s.to_string()),
                    run_id.map(|s| s.to_string()),
                    agent_id.map(|s| s.to_string()),
                    None,
                )
                .with_result(
                    EventResult::failure(&err_msg)
                        .with_duration(duration_ms),
                )
                .emit();
                Err(e)
            }
        }
    }

    pub fn validate_tool_args(&self, tool_name: &str, args: &serde_json::Value) -> Result<(), String> {
        let tool = self.tools.get(tool_name)
            .ok_or_else(|| format!("Unknown tool: {}", tool_name))?;
        let schema = &tool.schema;

        if let Some(required) = schema.get("required").and_then(|v| v.as_array()) {
            for field in required {
                let field_name = field.as_str()
                    .ok_or("Schema 'required' contains non-string value")?;
                if args.get(field_name).is_none() {
                    return Err(format!("Missing required argument: '{}'", field_name));
                }
            }
        }

        if let Some(properties) = schema.get("properties").and_then(|v| v.as_object()) {
            for (key, val) in args.as_object().unwrap_or(&serde_json::Map::new()) {
                if let Some(prop_schema) = properties.get(key) {
                    if let Some(expected_type) = prop_schema.get("type").and_then(|t| t.as_str()) {
                        let type_matches = match expected_type {
                            "string" => val.is_string(),
                            "number" | "integer" => val.is_number(),
                            "boolean" => val.is_boolean(),
                            "array" => val.is_array(),
                            "object" => val.is_object(),
                            _ => true,
                        };
                        if !type_matches {
                            return Err(format!(
                                "Argument '{}' expected {} but got {}",
                                key, expected_type,
                                match val {
                                    serde_json::Value::Null => "null",
                                    serde_json::Value::Bool(_) => "boolean",
                                    serde_json::Value::Number(_) => "number",
                                    serde_json::Value::String(_) => "string",
                                    serde_json::Value::Array(_) => "array",
                                    serde_json::Value::Object(_) => "object",
                                }
                            ));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn execute_with_retry(
        &self,
        tool_call: &mut ToolCall,
        workspace_id: Option<&str>,
        run_id: Option<&str>,
        agent_id: Option<&str>,
    ) -> anyhow::Result<String> {
        use agenticos_core::state::{RecoveryAction, RecoveryConfig, RecoveryState};
        use crate::error::classify_error;
        use crate::recovery::apply_recovery;

        let tool = self
            .tools
            .get(&tool_call.name)
            .ok_or_else(|| anyhow::anyhow!("Unknown tool: {}", tool_call.name))?;

        if !tool.metadata.retryable {
            return self.execute(tool_call, workspace_id, run_id, agent_id).await;
        }

        let recovery_config = RecoveryConfig::default();
        let mut recovery_state = RecoveryState::new();

        loop {
            let mut tc_attempt = tool_call.clone();
            match self.execute(&mut tc_attempt, workspace_id, run_id, agent_id).await {
                Ok(output) => {
                    tool_call.result = tc_attempt.result;
                    tool_call.duration_ms = tc_attempt.duration_ms;
                    return Ok(output);
                }
                Err(e) => {
                    let classified = classify_error(&e);
                    if !classified.retryable {
                        tool_call.error = tc_attempt.error;
                        tool_call.duration_ms = tc_attempt.duration_ms;
                        return Err(e);
                    }

                    let action = apply_recovery(&mut recovery_state, &recovery_config, true);
                    match action {
                        RecoveryAction::Retry(delay_ms) => {
                            tracing::warn!(
                                tool = %tool_call.name,
                                attempt = recovery_state.total_retries,
                                delay_ms = delay_ms,
                                "Tool retry with backoff"
                            );
                            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                            continue;
                        }
                        RecoveryAction::CircuitBreak => {
                            tool_call.error = Some(format!(
                                "Circuit breaker opened after {} retries",
                                recovery_state.total_retries
                            ));
                            return Err(anyhow::anyhow!(
                                "Circuit breaker opened after {} retries",
                                recovery_state.total_retries
                            ));
                        }
                        RecoveryAction::Fail(msg) => {
                            tool_call.error = Some(msg.clone());
                            return Err(anyhow::anyhow!(msg));
                        }
                    }
                }
            }
        }
    }

    fn register_builtins(&mut self) {
        self.register(
            "calculator",
            "Evaluate a mathematical expression. Supports +, -, *, /, parentheses, and basic math functions.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "description": "The mathematical expression to evaluate"
                    }
                },
                "required": ["expression"]
            }),
            Arc::new(|args| {
                Box::pin(handle_calculator(args))
            }),
        );

        self.register(
            "current_datetime",
            "Get the current date and time in ISO 8601 format.",
            serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
            Arc::new(|_args| {
                Box::pin(async { Ok(chrono::Utc::now().to_rfc3339()) })
            }),
        );

        self.register(
            "echo",
            "Echo back the input text. Useful for testing.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "The text to echo back"
                    }
                },
                "required": ["text"]
            }),
            Arc::new(|args| {
                Box::pin(handle_echo(args))
            }),
        );
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

async fn handle_calculator(args: serde_json::Value) -> anyhow::Result<String> {
    let expression = args
        .get("expression")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("calculator: missing 'expression' argument"))?;

    let result = eval_expression(expression)?;
    Ok(result.to_string())
}

async fn handle_echo(args: serde_json::Value) -> anyhow::Result<String> {
    let text = args
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("echo: missing 'text' argument"))?;
    Ok(text.to_string())
}

fn eval_expression(expr: &str) -> anyhow::Result<f64> {
    let expr = expr.trim();
    let tokens = tokenize(expr)?;
    let mut parser = Parser::new(tokens);
    let result = parser.parse_expression()?;
    if !parser.is_at_end() {
        anyhow::bail!("Unexpected token at position {}", parser.pos);
    }
    Ok(result)
}

#[derive(Debug, Clone)]
enum Token {
    Number(f64),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
    Pow,
    Mod,
}

fn tokenize(input: &str) -> anyhow::Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            ' ' | '\t' | '\n' => i += 1,
            '+' => { tokens.push(Token::Plus); i += 1; }
            '-' => { tokens.push(Token::Minus); i += 1; }
            '*' => {
                if i + 1 < chars.len() && chars[i + 1] == '*' {
                    tokens.push(Token::Pow);
                    i += 2;
                } else {
                    tokens.push(Token::Star);
                    i += 1;
                }
            }
            '/' => { tokens.push(Token::Slash); i += 1; }
            '%' => { tokens.push(Token::Mod); i += 1; }
            '(' => { tokens.push(Token::LParen); i += 1; }
            ')' => { tokens.push(Token::RParen); i += 1; }
            '0'..='9' | '.' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let num: f64 = input[start..i]
                    .parse()
                    .map_err(|_| anyhow::anyhow!("Invalid number: {}", &input[start..i]))?;
                tokens.push(Token::Number(num));
            }
            c => anyhow::bail!("Unexpected character: '{}'", c),
        }
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        let tok = self.tokens.get(self.pos).cloned();
        self.pos += 1;
        tok
    }

    fn parse_expression(&mut self) -> anyhow::Result<f64> {
        self.parse_add_sub()
    }

    fn parse_add_sub(&mut self) -> anyhow::Result<f64> {
        let mut left = self.parse_mul_div()?;
        while let Some(token) = self.peek() {
            match token {
                Token::Plus => {
                    self.advance();
                    left += self.parse_mul_div()?;
                }
                Token::Minus => {
                    self.advance();
                    left -= self.parse_mul_div()?;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_mul_div(&mut self) -> anyhow::Result<f64> {
        let mut left = self.parse_power()?;
        while let Some(token) = self.peek() {
            match token {
                Token::Star => {
                    self.advance();
                    left *= self.parse_power()?;
                }
                Token::Slash => {
                    self.advance();
                    let right = self.parse_power()?;
                    if right == 0.0 {
                        anyhow::bail!("Division by zero");
                    }
                    left /= right;
                }
                Token::Mod => {
                    self.advance();
                    left %= self.parse_power()?;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_power(&mut self) -> anyhow::Result<f64> {
        let left = self.parse_unary()?;
        if let Some(Token::Pow) = self.peek() {
            self.advance();
            let right = self.parse_power()?;
            Ok(left.powf(right))
        } else {
            Ok(left)
        }
    }

    fn parse_unary(&mut self) -> anyhow::Result<f64> {
        if let Some(Token::Minus) = self.peek() {
            self.advance();
            let val = self.parse_primary()?;
            Ok(-val)
        } else if let Some(Token::Plus) = self.peek() {
            self.advance();
            self.parse_primary()
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> anyhow::Result<f64> {
        match self.peek() {
            Some(Token::Number(_)) => {
                match self.advance() {
                    Some(Token::Number(n)) => Ok(n),
                    _ => unreachable!(),
                }
            }
            Some(Token::LParen) => {
                self.advance();
                let val = self.parse_expression()?;
                if !matches!(self.advance(), Some(Token::RParen)) {
                    anyhow::bail!("Expected closing parenthesis");
                }
                Ok(val)
            }
            Some(t) => anyhow::bail!("Unexpected token: {:?}", t),
            None => anyhow::bail!("Unexpected end of expression"),
        }
    }
}

#[cfg(test)]
mod tool_registry_tests {
    use super::*;
    use agenticos_core::state::ToolCall;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn register_with_metadata_custom() {
        let mut reg = ToolRegistry::empty();
        reg.register_with_metadata(
            "write_file", "Write to a file",
            serde_json::json!({"type": "object", "properties": {}}),
            Arc::new(|_args| Box::pin(async { Ok("ok".into()) })),
            ToolMetadata { requires_approval: true, retryable: false, timeout_secs: 30 },
        );
        let tool = reg.get("write_file").unwrap();
        assert!(tool.metadata.requires_approval);
        assert!(!tool.metadata.retryable);
    }

    #[test]
    fn default_metadata_values() {
        let m = ToolMetadata::default();
        assert!(!m.requires_approval);
        assert!(m.retryable);
        assert_eq!(m.timeout_secs, 30);
    }

    #[test]
    fn validate_missing_required_arg() {
        let reg = ToolRegistry::new();
        let result = reg.validate_tool_args("calculator", &serde_json::json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expression"));
    }

    #[test]
    fn validate_wrong_type() {
        let reg = ToolRegistry::new();
        let result = reg.validate_tool_args("calculator", &serde_json::json!({"expression": 123}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expected string"));
    }

    #[test]
    fn validate_valid_args() {
        let reg = ToolRegistry::new();
        let result = reg.validate_tool_args("calculator", &serde_json::json!({"expression": "2+2"}));
        assert!(result.is_ok());
    }

    #[test]
    fn validate_unknown_tool() {
        let reg = ToolRegistry::new();
        let result = reg.validate_tool_args("nonexistent", &serde_json::json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown tool"));
    }

    #[test]
    fn empty_registry_has_no_tools() {
        let reg = ToolRegistry::empty();
        assert!(!reg.has_tool("calculator"));
        assert_eq!(reg.list().len(), 0);
    }

    #[tokio::test]
    async fn tool_timeout() {
        let mut reg = ToolRegistry::empty();
        reg.register_with_metadata(
            "slow", "Slow tool",
            serde_json::json!({"type": "object", "properties": {}}),
            Arc::new(|_args| Box::pin(async {
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                Ok("done".into())
            })),
            ToolMetadata { requires_approval: false, retryable: false, timeout_secs: 1 },
        );
        let mut tc = ToolCall::new("slow", serde_json::json!({}));
        let result = reg.execute(&mut tc, None, None, None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timed out"));
    }

    #[tokio::test]
    async fn retry_succeeds_after_transient_failures() {
        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();
        let mut reg = ToolRegistry::empty();
        reg.register_with_metadata(
            "flaky", "Flaky tool",
            serde_json::json!({"type": "object", "properties": {}}),
            Arc::new(move |_args| {
                let c = c.clone();
                Box::pin(async move {
                    let n = c.fetch_add(1, Ordering::SeqCst);
                    if n < 2 {
                        Err(anyhow::anyhow!("HTTP 503 Service Unavailable"))
                    } else {
                        Ok("success".into())
                    }
                })
            }),
            ToolMetadata { requires_approval: false, retryable: true, timeout_secs: 30 },
        );
        let mut tc = ToolCall::new("flaky", serde_json::json!({}));
        let result = reg.execute_with_retry(&mut tc, None, None, None).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn non_retryable_tool_does_not_retry() {
        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();
        let mut reg = ToolRegistry::empty();
        reg.register_with_metadata(
            "no_retry", "No retry tool",
            serde_json::json!({"type": "object", "properties": {}}),
            Arc::new(move |_args| {
                let c = c.clone();
                Box::pin(async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err(anyhow::anyhow!("permanent failure"))
                })
            }),
            ToolMetadata { requires_approval: false, retryable: false, timeout_secs: 30 },
        );
        let mut tc = ToolCall::new("no_retry", serde_json::json!({}));
        // execute_with_retry delegates to execute for non-retryable tools
        let result = reg.execute_with_retry(&mut tc, None, None, None).await;
        assert!(result.is_err());
        // Should only be called once (no retries for non-retryable)
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
