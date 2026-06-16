use agenticos_core::state::{RecoveryAction, RecoveryConfig, RecoveryState};

pub fn apply_recovery(
    state: &mut RecoveryState,
    config: &RecoveryConfig,
    retryable: bool,
) -> RecoveryAction {
    if retryable {
        state.record_failure(config)
    } else {
        RecoveryAction::Fail("Non-retryable error".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agenticos_core::state::{RecoveryAction, RecoveryConfig, RecoveryState};

    fn test_config() -> RecoveryConfig {
        RecoveryConfig {
            max_retries: 3,
            base_delay_ms: 1000,
            max_delay_ms: 32000,
            max_consecutive_failures: 5,
        }
    }

    #[test]
    fn test_recovery_backoff_doubles() {
        let config = test_config();
        let mut state = RecoveryState::new();

        let action1 = state.record_failure(&config);
        assert_eq!(action1, RecoveryAction::Retry(1000));

        let action2 = state.record_failure(&config);
        assert_eq!(action2, RecoveryAction::Retry(2000));

        let action3 = state.record_failure(&config);
        assert_eq!(action3, RecoveryAction::Retry(4000));

        let action4 = state.record_failure(&config);
        assert_eq!(action4, RecoveryAction::Retry(8000));
    }

    #[test]
    fn test_recovery_backoff_capped() {
        let mut config = test_config();
        config.max_delay_ms = 5000;
        config.max_consecutive_failures = 8; // ensure circuit breaker doesn't trigger
        let mut state = RecoveryState::new();

        let action1 = state.record_failure(&config);
        assert_eq!(action1, RecoveryAction::Retry(1000));

        let action2 = state.record_failure(&config);
        assert_eq!(action2, RecoveryAction::Retry(2000));

        let action3 = state.record_failure(&config);
        assert_eq!(action3, RecoveryAction::Retry(4000));

        let action4 = state.record_failure(&config);
        assert_eq!(action4, RecoveryAction::Retry(5000));

        let action5 = state.record_failure(&config);
        assert_eq!(action5, RecoveryAction::Retry(5000));
    }

    #[test]
    fn test_recovery_resets_on_success() {
        let config = test_config();
        let mut state = RecoveryState::new();

        state.record_failure(&config);
        state.record_failure(&config);
        assert_eq!(state.backoff_ms, 2000);

        state.record_success();
        assert_eq!(state.consecutive_failures, 0);
        assert_eq!(state.backoff_ms, 0);

        let action = state.record_failure(&config);
        assert_eq!(action, RecoveryAction::Retry(1000));
    }

    #[test]
    fn test_circuit_breaker_after_max_failures() {
        let config = test_config();
        let mut state = RecoveryState::new();

        for _ in 0..4 {
            state.record_failure(&config);
        }

        assert_eq!(state.consecutive_failures, 4);
        assert!(!state.should_circuit_break(&config));

        let action = state.record_failure(&config);
        assert_eq!(state.consecutive_failures, 5);
        assert!(state.should_circuit_break(&config));
        assert_eq!(action, RecoveryAction::CircuitBreak);
    }

    #[test]
    fn test_apply_recovery_retryable() {
        let config = test_config();
        let mut state = RecoveryState::new();

        let action = apply_recovery(&mut state, &config, true);
        assert_eq!(action, RecoveryAction::Retry(1000));
    }

    #[test]
    fn test_apply_recovery_non_retryable() {
        let config = test_config();
        let mut state = RecoveryState::new();

        let action = apply_recovery(&mut state, &config, false);
        assert_eq!(action, RecoveryAction::Fail("Non-retryable error".into()));
        assert_eq!(state.consecutive_failures, 0);
    }
}
