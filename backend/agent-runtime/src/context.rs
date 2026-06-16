use agenticos_core::state::Message;

#[derive(Debug, Clone)]
pub struct ContextConfig {
    pub max_tokens: i64,
    pub summarize_threshold: f64,
    pub keep_recent: usize,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_tokens: 8000,
            summarize_threshold: 0.8,
            keep_recent: 5,
        }
    }
}

pub struct ContextManager {
    config: ContextConfig,
}

impl ContextManager {
    pub fn new(config: ContextConfig) -> Self {
        Self { config }
    }

    pub fn estimate_tokens(messages: &[Message]) -> i64 {
        let total_chars: usize = messages.iter().map(|m| m.content.len()).sum();
        (total_chars / 4) as i64
    }

    pub fn should_summarize(&self, messages: &[Message]) -> bool {
        let tokens = Self::estimate_tokens(messages);
        (tokens as f64) >= (self.config.max_tokens as f64 * self.config.summarize_threshold)
    }

    pub fn prune_messages(&self, messages: &[Message]) -> Vec<Message> {
        if messages.len() <= self.config.keep_recent + 1 {
            return messages.to_vec();
        }

        let mut result = Vec::new();

        if !messages.is_empty() {
            result.push(messages[0].clone());
        }

        let middle_count = messages.len() - 1 - self.config.keep_recent;
        result.push(Message::new(
            "system",
            &format!(
                "[Summary of earlier conversation: {} messages condensed during context management]",
                middle_count
            ),
        ));

        let start = messages.len() - self.config.keep_recent;
        for msg in &messages[start..] {
            result.push(msg.clone());
        }

        result
    }

    pub fn maybe_prune(&self, messages: &mut Vec<Message>) -> bool {
        if self.should_summarize(messages) {
            let pruned = self.prune_messages(messages);
            *messages = pruned;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(role: &str, content: &str) -> Message {
        Message::new(role, content)
    }

    #[test]
    fn estimate_tokens_counts_chars() {
        let messages = vec![
            msg("system", "You are helpful"),
            msg("user", "Hello"),
        ];
        assert_eq!(ContextManager::estimate_tokens(&messages), (15 + 5) / 4);
    }

    #[test]
    fn should_not_summarize_short_messages() {
        let mgr = ContextManager::new(ContextConfig::default());
        assert!(!mgr.should_summarize(&[msg("system", "short")]));
    }

    #[test]
    fn should_summarize_long_messages() {
        let cfg = ContextConfig { max_tokens: 10, summarize_threshold: 0.8, keep_recent: 5 };
        let mgr = ContextManager::new(cfg);
        let messages = vec![
            msg("system", "This is a deliberately long system prompt string"),
            msg("user", "And this is a deliberately long user message string too"),
        ];
        assert!(mgr.should_summarize(&messages));
    }

    #[test]
    fn prune_keeps_system_and_recent() {
        let cfg = ContextConfig { max_tokens: 1000, summarize_threshold: 0.8, keep_recent: 2 };
        let mgr = ContextManager::new(cfg);
        let messages = vec![
            msg("system", "sys"),
            msg("user", "old1"),
            msg("assistant", "old2"),
            msg("user", "old3"),
            msg("user", "recent1"),
            msg("assistant", "recent2"),
        ];
        let pruned = mgr.prune_messages(&messages);
        assert_eq!(pruned.len(), 4);
        assert_eq!(pruned[0].content, "sys");
        assert!(pruned[1].content.contains("Summary"));
        assert_eq!(pruned[2].content, "recent1");
        assert_eq!(pruned[3].content, "recent2");
    }

    #[test]
    fn prune_short_list_unchanged() {
        let mgr = ContextManager::new(ContextConfig::default());
        let messages = vec![msg("system", "s"), msg("user", "u")];
        let pruned = mgr.prune_messages(&messages);
        assert_eq!(pruned.len(), 2);
    }

    #[test]
    fn maybe_prune_returns_true_when_pruned() {
        let cfg = ContextConfig { max_tokens: 5, summarize_threshold: 0.5, keep_recent: 1 };
        let mgr = ContextManager::new(cfg);
        let mut messages = vec![
            msg("system", "long system prompt here"),
            msg("user", "long user msg here"),
            msg("assistant", "long assistant response here"),
        ];
        assert!(mgr.maybe_prune(&mut messages));
        assert!(messages.len() < 4);
    }

    #[test]
    fn maybe_prune_returns_false_when_not_needed() {
        let mgr = ContextManager::new(ContextConfig::default());
        let mut messages = vec![msg("system", "hi"), msg("user", "ho")];
        assert!(!mgr.maybe_prune(&mut messages));
        assert_eq!(messages.len(), 2);
    }
}
