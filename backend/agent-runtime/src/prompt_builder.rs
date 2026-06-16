use agenticos_core::state::AgentState;

/// Build a dynamic system prompt from agent state and memory.
///
/// Composes prompt from:
/// 1. Base instructions (from agent config)
/// 2. Relevant facts from memory (top 5)
/// 3. Relevant knowledge from KB (top 3)
/// 4. Working memory (current task context)
///
/// Ensures total prompt stays under ~4K tokens (16K chars).
pub fn build_system_prompt(state: &AgentState, memory_contents: &[String]) -> String {
    let mut parts = vec![state.instructions.clone()];
    let mut budget = state.instructions.len();
    let max_chars = 16_000; // ~4K tokens

    // Add relevant facts from memory (top 5)
    if !memory_contents.is_empty() {
        let facts: Vec<&str> = memory_contents.iter().take(5).map(|s| s.as_str()).collect();
        let section = format!("\n\n## Relevant Facts\n- {}", facts.join("\n- "));
        budget += section.len();
        parts.push(section);
    }

    // Add KB contents (top 3)
    if state.memory_config.enabled && !state.kb_contents.is_empty() {
        let kb: Vec<&str> = state.kb_contents.iter().take(3).map(|s| s.as_str()).collect();
        let section = format!("\n\n## Knowledge\n{}", kb.join("\n"));
        if budget + section.len() < max_chars {
            budget += section.len();
            parts.push(section);
        }
    }

    // Add working memory (current task)
    if let Some(last_user) = state.messages.iter().rev().find(|m| m.role == "user") {
        let section = format!("\n\n## Current Task\n{}", last_user.content);
        if budget + section.len() < max_chars {
            budget += section.len();
            parts.push(section);
        }
    }

    let result = parts.join("\n");

    // Final truncation if still over budget
    if result.len() > max_chars {
        let mut truncated = result[..max_chars].to_string();
        truncated.push_str("\n\n[Context truncated due to length]");
        truncated
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> AgentState {
        AgentState::new(
            Some("agent1".into()),
            Some("run1".into()),
            Some("ws1".into()),
            Some("user1".into()),
            "You are a helpful assistant.",
            "gpt-4",
        )
    }

    #[test]
    fn test_basic_prompt() {
        let state = test_state();
        let prompt = build_system_prompt(&state, &[]);
        assert!(prompt.contains("You are a helpful assistant"));
    }

    #[test]
    fn test_prompt_with_memories() {
        let state = test_state();
        let memories = vec![
            "User prefers concise answers".into(),
            "User is a Rust developer".into(),
        ];
        let prompt = build_system_prompt(&state, &memories);
        assert!(prompt.contains("Relevant Facts"));
        assert!(prompt.contains("User prefers concise answers"));
        assert!(prompt.contains("User is a Rust developer"));
    }

    #[test]
    fn test_prompt_with_kb_contents() {
        let mut state = test_state();
        state.memory_config.enabled = true;
        state.kb_contents = vec![
            "Fact 1 about the codebase".into(),
            "Fact 2 about the codebase".into(),
        ];
        let prompt = build_system_prompt(&state, &[]);
        assert!(prompt.contains("Knowledge"));
        assert!(prompt.contains("Fact 1 about the codebase"));
    }

    #[test]
    fn test_prompt_with_current_task() {
        let mut state = test_state();
        state.add_message(agenticos_core::state::Message::new(
            "user",
            "Help me refactor the auth module",
        ));
        let prompt = build_system_prompt(&state, &[]);
        assert!(prompt.contains("Current Task"));
        assert!(prompt.contains("refactor the auth module"));
    }

    #[test]
    fn test_prompt_budget_truncation() {
        let mut state = test_state();
        // Create a very long instruction that exceeds budget
        state.instructions = "x".repeat(17_000);
        let prompt = build_system_prompt(&state, &[]);
        assert!(prompt.len() <= 16_100); // max_chars + truncation message
        assert!(prompt.contains("truncated"));
    }

    #[test]
    fn test_prompt_memory_limit_top5() {
        let state = test_state();
        let memories: Vec<String> = (0..10).map(|i| format!("Fact {}", i)).collect();
        let prompt = build_system_prompt(&state, &memories);
        // Should only include first 5
        assert!(prompt.contains("Fact 0"));
        assert!(prompt.contains("Fact 4"));
        assert!(!prompt.contains("Fact 5"));
    }
}
