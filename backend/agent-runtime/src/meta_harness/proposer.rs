use std::path::PathBuf;
use std::time::Duration;
use crate::meta_harness::{HarnessCandidate, HarnessSearchConfig};
use tracing;

// ── Result types ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CandidateProposal {
    pub source_code: String,
    pub description: String,
    pub file_extension: String,
}

#[derive(Debug, Clone)]
pub struct Proposal {
    pub descriptions: Vec<CandidateProposal>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProposerError {
    #[error("OpenCode execution failed: {0}")]
    OpenCodeFailed(String),
    #[error("Proposer returned no candidates")]
    NoCandidates,
    #[error("Proposer output parse error: {0}")]
    ParseError(String),
}

// ── Proposer trait ────────────────────────────────────────────

#[async_trait::async_trait]
pub trait Proposer: Send + Sync {
    async fn propose(
        &self,
        config: &HarnessSearchConfig,
        candidates: &[HarnessCandidate],
        registry_work_dir: &PathBuf,
    ) -> Result<Proposal, ProposerError>;
}

// ── OpenCode Proposer ─────────────────────────────────────────

pub struct OpenCodeProposer {
    model: String,
    binary: Option<String>,
    timeout_secs: u64,
}

impl OpenCodeProposer {
    pub fn new(model: &str) -> Self {
        Self { model: model.to_string(), binary: None, timeout_secs: 600 }
    }

    pub fn with_binary(mut self, binary: &str) -> Self {
        self.binary = Some(binary.to_string());
        self
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    fn build_prompt(&self, _config: &HarnessSearchConfig, candidates: &[HarnessCandidate], work_dir: &PathBuf) -> String {
        let mut prompt = String::from(
            "You are a harness engineering agent.\n\n\
             Your task is to design a NEW harness — the code that wraps an LLM to manage \
             prompt construction, retrieval, memory, and orchestration logic.\n\n"
        );

        prompt.push_str("## Prior Harnesses\n\n");
        if candidates.is_empty() {
            prompt.push_str("No prior harnesses yet. Design a baseline harness.\n");
        } else {
            for (i, c) in candidates.iter().enumerate() {
                prompt.push_str(&format!(
                    "{}. **{}** (iter {}, score: {:.3}, tokens: {})\n   Desc: {}\n   Source: {}\n",
                    i + 1, c.id, c.iteration, c.aggregate_score, c.context_tokens, c.description,
                    work_dir.join(&c.id).join("harness.rs").display()
                ));
            }
        }

        prompt.push_str(&format!(
            "\n## Instructions\n\n\
             1. Inspect prior harness code & traces in: `{}`\n\
             2. Diagnose WHY low-scoring harnesses failed\n\
             3. Propose a NEW harness that improves on the best\n\
             4. Write harness as a single Rust file `harness.rs` with:\n\
                - `pub struct {{Name}}Harness`\n\
                - `pub async fn run(&self, input: &str, history: &[String]) -> String`\n\
             5. End with: `// DESCRIPTION: <one-line summary>`\n\n\
             Focus on ONE mechanism: prompt construction, retrieval, context mgmt, \
             memory, or error recovery. Change the algorithm, not parameters.",
            work_dir.display()
        ));
        prompt
    }

    /// Try to read the output file if it exists
    async fn try_read_output(output_file: &PathBuf) -> Option<(String, String)> {
        if tokio::fs::try_exists(output_file).await.unwrap_or(false) {
            let code = tokio::fs::read_to_string(output_file).await.ok()?;
            let desc = extract_description(&code);
            Some((code, desc))
        } else {
            None
        }
    }

    fn make_proposal(code: String, desc: String) -> Proposal {
        Proposal { descriptions: vec![CandidateProposal { source_code: code, description: desc, file_extension: "rs".into() }] }
    }
}

#[async_trait::async_trait]
impl Proposer for OpenCodeProposer {
    async fn propose(
        &self,
        config: &HarnessSearchConfig,
        candidates: &[HarnessCandidate],
        registry_work_dir: &PathBuf,
    ) -> Result<Proposal, ProposerError> {
        // Write prompt file
        let prompt = self.build_prompt(config, candidates, registry_work_dir);
        let prompt_path = registry_work_dir.join("_proposer_prompt.txt");
        tokio::fs::write(&prompt_path, &prompt)
            .await
            .map_err(|e| ProposerError::OpenCodeFailed(format!("write prompt: {e}")))?;

        // Create output directory
        let output_dir = registry_work_dir.join("_latest_proposal");
        let output_file = output_dir.join("harness.rs");
        tokio::fs::create_dir_all(&output_dir).await.ok();

        let binary = self.binary.as_deref().unwrap_or("opencode");
        let instr = format!(
            "Read prompt at '{}'. Write harness to '{}'. Rust file with `pub async fn run(...)`.",
            prompt_path.to_string_lossy().replace('\\', "/"),
            output_file.to_string_lossy().replace('\\', "/"),
        );

        let cmd = format!(r#"{} run --model {} '{}' --format json"#, binary, self.model, instr);
        tracing::info!("Proposer: {}", cmd);

        let child = tokio::process::Command::new("bash")
            .arg("-c").arg(&cmd)
            .current_dir(registry_work_dir)
            .kill_on_drop(true)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| ProposerError::OpenCodeFailed(format!("spawn: {e}")))?;

        let result = tokio::time::timeout(Duration::from_secs(self.timeout_secs), child.wait_with_output()).await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                if !output.status.success() {
                    if let Some((code, desc)) = Self::try_read_output(&output_file).await {
                        tracing::info!("Wrote proposal despite non-zero exit");
                        return Ok(Self::make_proposal(code, desc));
                    }
                    tracing::warn!("Failed: exit={}, stderr={}", output.status, &stderr[..200.min(stderr.len())]);
                    return Err(ProposerError::OpenCodeFailed(format!("exit={}, stderr={}", output.status, &stderr[..200.min(stderr.len())])));
                }

                if let Some((code, desc)) = Self::try_read_output(&output_file).await {
                    if let Some((name, score)) = parse_opencode_json_output(&stdout) {
                        tracing::info!("Candidate {} score: {}", name, score);
                    }
                    tracing::info!("Wrote: {}", output_file.display());
                    return Ok(Self::make_proposal(code, desc));
                }

                tracing::warn!("No output. stdout: {}", &stdout[..200.min(stdout.len())]);
                Err(ProposerError::NoCandidates)
            }
            Ok(Err(e)) => Err(ProposerError::OpenCodeFailed(format!("process: {e}"))),
            Err(_) => {
                tracing::warn!("Timed out after {}s", self.timeout_secs);
                if let Some((code, desc)) = Self::try_read_output(&output_file).await {
                    tracing::info!("Wrote before timeout");
                    return Ok(Self::make_proposal(code, desc));
                }
                Err(ProposerError::OpenCodeFailed(format!("timed out after {}s", self.timeout_secs)))
            }
        }
    }
}

fn extract_description(source_code: &str) -> String {
    for line in source_code.lines() {
        if let Some(desc) = line.trim().strip_prefix("// DESCRIPTION:") {
            return desc.trim().to_string();
        }
    }
    "Harness (no description)".to_string()
}

fn parse_opencode_json_output(stdout: &str) -> Option<(String, f64)> {
    for line in stdout.lines() {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line.trim()) {
            if let Some(content) = val.get("content").and_then(|c| c.as_str()) {
                if let Some(idx) = content.find("HARNESS_SCORE:") {
                    let rest = content[idx + 14..].trim();
                    let parts: Vec<&str> = rest.splitn(2, ' ').collect();
                    if let (Some(score_str), Some(name)) = (parts.first(), parts.get(1)) {
                        if let Ok(score) = score_str.parse::<f64>() {
                            return Some((name.trim().to_string(), score));
                        }
                    }
                }
            }
        }
    }
    None
}

// ── Mock Proposer ─────────────────────────────────────────────

pub struct MockProposer {
    pub proposals: Vec<Vec<(f64, u64, String, String)>>,
    pub call_count: std::sync::atomic::AtomicU32,
}

impl MockProposer {
    pub fn new(proposals: Vec<Vec<(f64, u64, String, String)>>) -> Self {
        Self { proposals, call_count: std::sync::atomic::AtomicU32::new(0) }
    }

    pub fn simple(descriptions: Vec<&str>) -> Self {
        let proposals: Vec<Vec<_>> = descriptions.into_iter().map(|desc| {
            vec![(0.5, 1000, desc.to_string(), format!("// DESCRIPTION: {}\npub struct Harness; impl Harness {{}}", desc))]
        }).collect();
        Self::new(proposals)
    }
}

#[async_trait::async_trait]
impl Proposer for MockProposer {
    async fn propose(
        &self,
        _config: &HarnessSearchConfig,
        _candidates: &[HarnessCandidate],
        _registry_work_dir: &PathBuf,
    ) -> Result<Proposal, ProposerError> {
        let idx = self.call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as usize;
        let batch = self.proposals.get(idx).cloned().unwrap_or_default();
        Ok(Proposal {
            descriptions: batch.into_iter().map(|(_score, _tokens, desc, code)| {
                CandidateProposal { source_code: code, description: desc, file_extension: "rs".into() }
            }).collect(),
        })
    }
}