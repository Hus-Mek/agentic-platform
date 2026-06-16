use std::path::PathBuf;
use tokio::fs;
use anyhow::Result;

fn workspace_dir(run_id: &str) -> PathBuf {
    PathBuf::from(format!("/tmp/agenticos/workspaces/{}", run_id))
}

async fn ensure_workspace(run_id: &str) -> Result<PathBuf> {
    let dir = workspace_dir(run_id);
    fs::create_dir_all(&dir).await?;
    Ok(dir)
}

pub async fn fs_read_file(run_id: &str, path: &str) -> Result<String> {
    let ws = ensure_workspace(run_id).await?;
    let full_path = ws.join(path);
    if !full_path.starts_with(&ws) {
        anyhow::bail!("Path traversal detected: {}", path);
    }
    let content = fs::read_to_string(&full_path).await?;
    Ok(content)
}

pub async fn fs_write_file(run_id: &str, path: &str, content: &str) -> Result<String> {
    let ws = ensure_workspace(run_id).await?;
    let full_path = ws.join(path);
    if !full_path.starts_with(&ws) {
        anyhow::bail!("Path traversal detected: {}", path);
    }
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent).await?;
    }
    fs::write(&full_path, content).await?;
    Ok(format!("Written {} bytes to {}", content.len(), path))
}

pub async fn fs_list_dir(run_id: &str, path: &str) -> Result<String> {
    let ws = ensure_workspace(run_id).await?;
    let full_path = ws.join(path);
    if !full_path.starts_with(&ws) {
        anyhow::bail!("Path traversal detected: {}", path);
    }
    let mut entries = fs::read_dir(&full_path).await?;
    let mut results = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type().await?.is_dir();
        results.push(format!("{}/{}", name, if is_dir { "dir" } else { "file" }));
    }
    Ok(results.join("\n"))
}
