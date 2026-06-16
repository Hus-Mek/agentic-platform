use agenticos_core::llm_provider::{ProviderKind, detect_provider};

#[test]
fn test_detect_provider_openai() {
    assert_eq!(detect_provider("gpt-4o"), ProviderKind::OpenAI);
    assert_eq!(detect_provider("gpt-3.5-turbo"), ProviderKind::OpenAI);
    assert_eq!(detect_provider("text-davinci-003"), ProviderKind::OpenAI);
}

#[test]
fn test_detect_provider_anthropic() {
    assert_eq!(detect_provider("claude-3-5-sonnet-20241022"), ProviderKind::Anthropic);
    assert_eq!(detect_provider("claude-3-haiku-20240307"), ProviderKind::Anthropic);
    assert_eq!(detect_provider("anthropic.claude-v2"), ProviderKind::Anthropic);
}

#[test]
fn test_detect_provider_google() {
    assert_eq!(detect_provider("gemini-pro"), ProviderKind::Google);
    assert_eq!(detect_provider("gemini-1.5-pro"), ProviderKind::Google);
    assert_eq!(detect_provider("google-gemini"), ProviderKind::Google);
}