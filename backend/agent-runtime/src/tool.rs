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

#[derive(Debug, Clone, Default)]
pub struct ToolMetadata {
    pub requires_approval: bool,
    pub retryable: bool,
    pub timeout_secs: u64,
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
        let result = (tool.handler)(tool_call.args.clone()).await;
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
