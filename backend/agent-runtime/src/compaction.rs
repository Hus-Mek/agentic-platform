use agenticos_core::llm::LiteLLMClient;
use agenticos_core::state::{AgentState, Message};
use agenticos_core::udm::{AgentEvent, EventType, Observable};

/// Estimates token count from messages (rough: chars / 4).
pub fn estimate_context_tokens(messages: &[Message]) -> i64 {
    let total_chars: usize = messages.iter().map(|m| m.content.len()).sum();
    (total_chars / 4) as i64
}

/// Checks if context compaction is needed.
pub fn should_compact(estimated_tokens: i64, threshold: i64) -> bool {
    estimated_tokens >= threshold
}

/// Compacts the message history by summarizing older messages.
/// Keeps the anchor block (original instructions), a structured summary, and the last N messages.
pub async fn compact_context(
    state: &mut AgentState,
    llm_client: &LiteLLMClient,
) -> anyhow::Result<()> {
    const KEEP_RECENT: usize = 5;

    // Build anchor block
    let anchor = format!(
        "[Task]\n{}\n\n[Constraints]\nToken budget: {} used of {}\nMax iterations: {}\nCurrent iteration: {}",
        state.anchor_block,
        state.total_tokens_used,
        state.token_budget,
        state.max_iterations,
        state.iteration,
    );

    // Collect messages to summarize (everything except system and last KEEP_RECENT)
    let messages_to_summarize: Vec<&Message> = state
        .messages
        .iter()
        .filter(|m| m.role != "system")
        .take(state.messages.len().saturating_sub(KEEP_RECENT + 1))
        .collect();

    if messages_to_summarize.is_empty() {
        return Ok(());
    }

    // Build summarization prompt
    let conversation_text = messages_to_summarize
        .iter()
        .map(|m| format!("[{}] {}", m.role, m.content))
        .collect::<Vec<_>>()
        .join("\n\n");

    let summary_prompt = format!(
        r#"Summarize this agent conversation into a structured summary with these sections:
- **Intent**: What the user asked for
- **Decisions**: Key decisions made
- **Actions**: Tools called and their results (compacted)
- **Failed Attempts**: What didn't work (IMPORTANT: preserve these)
- **Next Steps**: What was in progress
- **Key Facts**: Important facts discovered

Conversation:
{}

Return ONLY the structured summary, no other text."#,
        conversation_text
    );

    let summary_messages = vec![
        Message::new("system", "You are a conversation summarizer. Create structured summaries that preserve intent, decisions, failed attempts, and key facts."),
        Message::new("user", &summary_prompt),
    ];

    let summary = match llm_client.chat_completion(&state.model, &summary_messages, &[]).await {
        Ok(resp) => resp.content.unwrap_or_else(|| {
            // Fallback: just note that summarization failed
            format!("[Summary unavailable. {} messages compacted.]", messages_to_summarize.len())
        }),
        Err(e) => {
            tracing::warn!("Context compaction LLM call failed: {e}. Using fallback.");
            format!("[Summary unavailable. {} messages compacted.]", messages_to_summarize.len())
        }
    };

    // Build new message array: [anchor_system, summary_system, ...recent_messages]
    let mut new_messages: Vec<Message> = Vec::new();

    // Anchor as system message
    new_messages.push(Message {
        id: format!("msg_anchor_{}", uuid::Uuid::new_v4()),
        role: "system".into(),
        content: anchor,
        tool_calls: None,
        tool_call_id: None,
        name: None,
        timestamp: chrono::Utc::now(),
    });

    // Summary as system message
    new_messages.push(Message {
        id: format!("msg_summary_{}", uuid::Uuid::new_v4()),
        role: "system".into(),
        content: format!("[Session Summary]\n{}", summary),
        tool_calls: None,
        tool_call_id: None,
        name: None,
        timestamp: chrono::Utc::now(),
    });

    // Keep last KEEP_RECENT non-system messages at full fidelity
    let recent: Vec<Message> = state
        .messages
        .iter()
        .filter(|m| m.role != "system")
        .rev()
        .take(KEEP_RECENT)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    new_messages.extend(recent);

    // Emit UDM event
    AgentEvent::new(
        EventType::SystemWarning,
        state.workspace_id.clone(),
        state.run_id.clone(),
        state.agent_id.clone(),
        state.user_id.clone(),
    )
    .with_observable(Observable::message(&format!(
        "Context compacted: {} messages summarized, {} kept",
        messages_to_summarize.len(),
        KEEP_RECENT
    )))
    .emit();

    state.messages = new_messages;
    state.estimated_context_tokens = estimate_context_tokens(&state.messages);

    Ok(())
}
