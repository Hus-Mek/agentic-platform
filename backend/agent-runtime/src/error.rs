#[derive(Debug, Clone, PartialEq)]
pub enum ErrorKind {
    Transient,
    Permanent,
    Guard,
    Context,
}

#[derive(Debug, Clone)]
pub struct ClassifiedError {
    pub kind: ErrorKind,
    pub message: String,
    pub retryable: bool,
    pub retry_after_ms: Option<u64>,
}

pub fn classify_error(error: &anyhow::Error) -> ClassifiedError {
    let msg = error.to_string().to_lowercase();

    if msg.contains("timeout")
        || msg.contains("429")
        || msg.contains("502")
        || msg.contains("503")
        || msg.contains("504")
        || msg.contains("connection")
        || msg.contains("rate limit")
        || msg.contains("try again")
    {
        return ClassifiedError {
            kind: ErrorKind::Transient,
            message: error.to_string(),
            retryable: true,
            retry_after_ms: Some(1000),
        };
    }

    if msg.contains("denied") || msg.contains("aborted") || msg.contains("policy") {
        return ClassifiedError {
            kind: ErrorKind::Guard,
            message: error.to_string(),
            retryable: false,
            retry_after_ms: None,
        };
    }

    if msg.contains("token budget") || msg.contains("context length") || msg.contains("too long") {
        return ClassifiedError {
            kind: ErrorKind::Context,
            message: error.to_string(),
            retryable: false,
            retry_after_ms: None,
        };
    }

    ClassifiedError {
        kind: ErrorKind::Permanent,
        message: error.to_string(),
        retryable: false,
        retry_after_ms: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_transient_timeout() {
        let err = anyhow::anyhow!("Request timeout after 30s");
        let classified = classify_error(&err);
        assert_eq!(classified.kind, ErrorKind::Transient);
        assert!(classified.retryable);
    }

    #[test]
    fn test_classify_transient_rate_limit() {
        let err = anyhow::anyhow!("HTTP 429 Too Many Requests");
        let classified = classify_error(&err);
        assert_eq!(classified.kind, ErrorKind::Transient);
        assert!(classified.retryable);
    }

    #[test]
    fn test_classify_transient_server_error() {
        let err = anyhow::anyhow!("HTTP 503 Service Unavailable");
        let classified = classify_error(&err);
        assert_eq!(classified.kind, ErrorKind::Transient);
        assert!(classified.retryable);
    }

    #[test]
    fn test_classify_permanent_bad_json() {
        let err = anyhow::anyhow!("failed to parse JSON: expected value at line 1");
        let classified = classify_error(&err);
        assert_eq!(classified.kind, ErrorKind::Permanent);
        assert!(!classified.retryable);
    }

    #[test]
    fn test_classify_guard_denied() {
        let err = anyhow::anyhow!("Tool access denied: policy violation");
        let classified = classify_error(&err);
        assert_eq!(classified.kind, ErrorKind::Guard);
        assert!(!classified.retryable);
    }

    #[test]
    fn test_classify_context_overflow() {
        let err = anyhow::anyhow!("token budget exceeded: context length too long");
        let classified = classify_error(&err);
        assert_eq!(classified.kind, ErrorKind::Context);
        assert!(!classified.retryable);
    }
}
