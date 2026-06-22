//! Resilient chat-completion routing: Matchmaker model selection plus a
//! retry → fallback-chain → circuit-break wrapper around the LLM client.
//!
//! All in-process LLM calls funnel through [`route_chat_completion`] so they
//! gain transient-error retries, automatic fallback to alternate models, and a
//! circuit breaker that mirrors the agent loop's [`RecoveryState`] semantics.
//! Model *selection* (the Matchmaker) is opt-in via `auto_route`; resilience is
//! always on.

use std::future::Future;

use agenticos_core::error::{AgentError, Result};
use agenticos_core::llm::{LlmClient, LlmResponse};
use agenticos_core::model_router::{ModelCapability, ModelRouter, TaskComplexity};
use agenticos_core::state::{Message, RecoveryAction, RecoveryConfig, RecoveryState, ToolDefinition};

/// Options controlling a single routed completion.
pub struct RouteOptions<'a> {
    /// When true, pick the model from the router catalog by task complexity +
    /// budget. When false, the caller's requested model is used as the primary.
    pub auto_route: bool,
    /// Extra capabilities the chosen model must support (Chat is always implied;
    /// FunctionCalling is added automatically when tools are present).
    pub required_capabilities: &'a [ModelCapability],
    /// Remaining token budget, used to bias selection toward cheap models.
    pub token_budget_remaining: Option<i64>,
    /// Retry/backoff/circuit-break policy applied per model in the chain.
    pub recovery: RecoveryConfig,
}

/// A tight recovery policy tuned for routing: a couple of quick retries, then
/// fall back to the next model rather than stalling on one provider.
fn routing_recovery() -> RecoveryConfig {
    RecoveryConfig {
        max_retries: 3,
        base_delay_ms: 250,
        max_delay_ms: 2_000,
        max_consecutive_failures: 3,
    }
}

impl<'a> RouteOptions<'a> {
    /// Resilience only: keep the requested model, no Matchmaker selection.
    pub fn resilient() -> Self {
        Self {
            auto_route: false,
            required_capabilities: &[],
            token_budget_remaining: None,
            recovery: routing_recovery(),
        }
    }
}

impl Default for RouteOptions<'_> {
    fn default() -> Self {
        Self::resilient()
    }
}

/// Classify an LLM error as transient (worth retrying) or permanent.
///
/// Network/timeout failures and 5xx/408/409/429 responses are transient; 4xx
/// (other than the above), parse failures, and empty responses are permanent.
pub fn is_retryable(err: &AgentError) -> bool {
    match err {
        AgentError::LlmProvider(msg) => {
            let m = msg.as_str();
            if m.contains("Request failed") || m.contains("request failed") {
                return true; // reqwest transport/timeout error
            }
            match parse_status(m) {
                Some(code) => code == 408 || code == 409 || code == 429 || (500..600).contains(&code),
                None => false, // "Parse error", "No choices", etc.
            }
        }
        _ => false,
    }
}

/// Extract the HTTP status from an error string like `API error (429): ...`.
fn parse_status(msg: &str) -> Option<u16> {
    let idx = msg.find("error (")?;
    let rest = &msg[idx + "error (".len()..];
    let end = rest.find(')')?;
    rest[..end].trim().parse().ok()
}

/// Build the ordered list of models to attempt: the primary first, then the
/// router's fallback chain (deduplicated).
pub fn attempt_chain(primary: &str, router: Option<&ModelRouter>) -> Vec<String> {
    let mut chain = vec![primary.to_string()];
    if let Some(r) = router {
        for m in r.fallback_chain() {
            if !chain.iter().any(|c| c == m) {
                chain.push(m.clone());
            }
        }
    }
    chain
}

/// Derive the capabilities a model must support for this request.
pub fn derive_capabilities(base: &[ModelCapability], has_tools: bool) -> Vec<ModelCapability> {
    let mut caps = vec![ModelCapability::Chat];
    if has_tools {
        caps.push(ModelCapability::FunctionCalling);
    }
    for c in base {
        if !caps.contains(c) {
            caps.push(c.clone());
        }
    }
    caps
}

/// Run `attempt` across the model chain with retry + backoff + circuit-break.
///
/// Per model: transient errors back off and retry until the circuit breaks,
/// then we advance to the next model; permanent errors advance immediately.
/// Returns the last error once every model is exhausted.
pub async fn run_with_resilience<F, Fut>(
    chain: &[String],
    recovery: &RecoveryConfig,
    mut attempt: F,
) -> Result<LlmResponse>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<LlmResponse>>,
{
    let mut last_err: Option<AgentError> = None;
    for model in chain {
        let mut rec = RecoveryState::new();
        loop {
            match attempt(model.clone()).await {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    if !is_retryable(&e) {
                        tracing::warn!(model = %model, error = %e, "LLM permanent error; advancing to fallback");
                        last_err = Some(e);
                        break;
                    }
                    match rec.record_failure(recovery) {
                        RecoveryAction::Retry(delay) => {
                            tracing::warn!(model = %model, delay_ms = delay, error = %e, "LLM transient error; retrying");
                            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                        }
                        RecoveryAction::CircuitBreak => {
                            tracing::warn!(model = %model, "LLM circuit broke; advancing to fallback");
                            last_err = Some(e);
                            break;
                        }
                        RecoveryAction::Fail(msg) => {
                            last_err = Some(AgentError::LlmProvider(msg));
                            break;
                        }
                    }
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| AgentError::LlmProvider("No model attempted".into())))
}

/// Resilient chat completion with optional Matchmaker model selection.
pub async fn route_chat_completion(
    client: &LlmClient,
    requested_model: &str,
    messages: &[Message],
    tools: &[ToolDefinition],
    router: Option<&ModelRouter>,
    opts: &RouteOptions<'_>,
) -> Result<LlmResponse> {
    // 1. Matchmaker: pick the primary model when auto-routing is enabled.
    let primary = match (opts.auto_route, router) {
        (true, Some(r)) => {
            let complexity = TaskComplexity::estimate(messages);
            let caps = derive_capabilities(opts.required_capabilities, !tools.is_empty());
            let chosen = r.select_model(complexity, &caps, opts.token_budget_remaining);
            tracing::info!(requested = %requested_model, chosen = %chosen, ?complexity, "Matchmaker selected model");
            chosen
        }
        _ => requested_model.to_string(),
    };

    // 2. Attempt the primary, then fall back through the chain, each with retries.
    let chain = attempt_chain(&primary, router);
    run_with_resilience(&chain, &opts.recovery, |model| async move {
        client.chat_completion(&model, messages, tools).await
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use agenticos_core::llm::{LlmResponse, LlmUsage};
    use std::cell::RefCell;

    fn ok_response(content: &str) -> LlmResponse {
        LlmResponse {
            content: Some(content.to_string()),
            tool_calls: None,
            finish_reason: Some("stop".into()),
            usage: LlmUsage {
                prompt_tokens: 1,
                completion_tokens: 1,
                total_tokens: 2,
                cost: 0.0,
            },
        }
    }

    fn transient() -> AgentError {
        AgentError::LlmProvider("API error (503): upstream unavailable".into())
    }

    fn permanent() -> AgentError {
        AgentError::LlmProvider("API error (400): bad request".into())
    }

    #[test]
    fn classifies_errors() {
        assert!(is_retryable(&AgentError::LlmProvider("Request failed: timeout".into())));
        assert!(is_retryable(&AgentError::LlmProvider("API error (429): rate limited".into())));
        assert!(is_retryable(&AgentError::LlmProvider("Stream API error (500): boom".into())));
        assert!(!is_retryable(&AgentError::LlmProvider("API error (400): bad request".into())));
        assert!(!is_retryable(&AgentError::LlmProvider("Parse error: bad json".into())));
        assert!(!is_retryable(&AgentError::Internal("nope".into())));
    }

    #[test]
    fn builds_attempt_chain_dedup() {
        let mut router = ModelRouter::new("primary".into());
        router.set_fallback_chain(vec!["primary".into(), "backup".into()]);
        // primary appears once even though it heads the fallback chain too.
        assert_eq!(
            attempt_chain("primary", Some(&router)),
            vec!["primary".to_string(), "backup".to_string()]
        );
        assert_eq!(attempt_chain("solo", None), vec!["solo".to_string()]);
    }

    #[test]
    fn derives_capabilities_with_tools() {
        let caps = derive_capabilities(&[ModelCapability::Reasoning], true);
        assert!(caps.contains(&ModelCapability::Chat));
        assert!(caps.contains(&ModelCapability::FunctionCalling));
        assert!(caps.contains(&ModelCapability::Reasoning));
        let no_tools = derive_capabilities(&[], false);
        assert_eq!(no_tools, vec![ModelCapability::Chat]);
    }

    #[tokio::test]
    async fn retries_then_succeeds_on_same_model() {
        let calls = RefCell::new(0u32);
        let cfg = routing_recovery();
        let result = run_with_resilience(&["m".to_string()], &cfg, |_m| {
            *calls.borrow_mut() += 1;
            let n = *calls.borrow();
            async move {
                if n < 2 { Err(transient()) } else { Ok(ok_response("hi")) }
            }
        })
        .await;
        assert!(result.is_ok());
        assert_eq!(*calls.borrow(), 2); // one transient failure, then success
    }

    #[tokio::test]
    async fn falls_back_to_next_model_after_circuit_break() {
        let seen = RefCell::new(Vec::<String>::new());
        let cfg = routing_recovery();
        let result = run_with_resilience(
            &["bad".to_string(), "good".to_string()],
            &cfg,
            |m| {
                seen.borrow_mut().push(m.clone());
                async move {
                    if m == "good" { Ok(ok_response("ok")) } else { Err(transient()) }
                }
            },
        )
        .await;
        assert!(result.is_ok());
        // "bad" retried until circuit break (3 attempts), then "good" once.
        let s = seen.borrow();
        assert_eq!(s.iter().filter(|m| *m == "bad").count(), 3);
        assert_eq!(s.last().unwrap(), "good");
    }

    #[tokio::test]
    async fn permanent_error_advances_without_retry() {
        let calls = RefCell::new(0u32);
        let cfg = routing_recovery();
        let result = run_with_resilience(
            &["a".to_string(), "b".to_string()],
            &cfg,
            |_m| {
                *calls.borrow_mut() += 1;
                async move { Err(permanent()) }
            },
        )
        .await;
        assert!(result.is_err());
        // one attempt per model, no retries on permanent errors.
        assert_eq!(*calls.borrow(), 2);
    }

    #[tokio::test]
    async fn exhausts_all_models_and_returns_last_error() {
        let cfg = RecoveryConfig {
            max_retries: 1,
            base_delay_ms: 1,
            max_delay_ms: 1,
            max_consecutive_failures: 1,
        };
        let result = run_with_resilience(&["only".to_string()], &cfg, |_m| async move {
            Err(transient())
        })
        .await;
        assert!(result.is_err());
    }
}
