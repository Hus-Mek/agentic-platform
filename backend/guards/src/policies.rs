use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Severity {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiiFinding {
    pub pattern_name: String,
    pub severity: Severity,
    pub start: usize,
    pub end: usize,
    pub redacted: String,
}

struct CompiledPattern {
    regex: Regex,
    name: &'static str,
    replacement: &'static str,
    severity: Severity,
}

pub struct PiiDetector {
    patterns: Vec<CompiledPattern>,
}

impl PiiDetector {
    pub fn new() -> Self {
        let raw_patterns: [(&str, &'static str, &'static str, Severity); 6] = [
            (
                r"[\w._%+\-]+@[\w.\-]+\.[a-zA-Z]{2,}",
                "email",
                "[EMAIL REDACTED]",
                Severity::Medium,
            ),
            (r"\+?1?\d{10,15}", "phone", "[PHONE REDACTED]", Severity::High),
            (
                r"\d{3}-\d{2}-\d{4}",
                "ssn",
                "[SSN REDACTED]",
                Severity::High,
            ),
            (
                r"\d{4}[- ]?\d{4}[- ]?\d{4}[- ]?\d{4}",
                "credit_card",
                "[CC REDACTED]",
                Severity::High,
            ),
            (
                r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}",
                "ipv4",
                "[IP REDACTED]",
                Severity::Low,
            ),
            (
                r"(sk-|pk-)[a-zA-Z0-9]{20,}",
                "api_key",
                "[API KEY REDACTED]",
                Severity::High,
            ),
        ];

        let patterns = raw_patterns
            .into_iter()
            .map(|(re, name, replacement, severity)| CompiledPattern {
                regex: Regex::new(re).expect("Invalid PII regex pattern"),
                name,
                replacement,
                severity,
            })
            .collect();

        Self { patterns }
    }

    pub fn check(&self, text: &str) -> Vec<PiiFinding> {
        let mut findings = Vec::new();
        for pattern in &self.patterns {
            for mat in pattern.regex.find_iter(text) {
                findings.push(PiiFinding {
                    pattern_name: pattern.name.to_string(),
                    severity: pattern.severity.clone(),
                    start: mat.start(),
                    end: mat.end(),
                    redacted: pattern.replacement.to_string(),
                });
            }
        }
        findings
    }

    pub fn sanitize(&self, text: &str) -> (String, Vec<PiiFinding>) {
        let findings = self.check(text);
        if findings.is_empty() {
            return (text.to_string(), findings);
        }

        let mut sorted = findings.clone();
        sorted.sort_by(|a, b| b.start.cmp(&a.start));

        let mut result = text.to_string();
        for finding in &sorted {
            result.replace_range(finding.start..finding.end, &finding.redacted);
        }

        (result, findings)
    }
}

impl Default for PiiDetector {
    fn default() -> Self {
        Self::new()
    }
}
