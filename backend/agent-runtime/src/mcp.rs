use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Deserialize)]
pub struct McpListResponse {
    pub tools: Vec<McpTool>,
}

#[derive(Debug, Serialize)]
pub struct McpCallRequest {
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Deserialize)]
pub struct McpCallResponse {
    pub content: Vec<McpContent>,
}

#[derive(Debug, Deserialize)]
pub struct McpContent {
    #[serde(rename = "type")]
    pub type_: String,
    pub text: String,
}

pub struct McpClient {
    base_url: String,
    http_client: reqwest::Client,
}

impl McpClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    pub async fn list_tools(&self) -> Result<Vec<McpTool>> {
        let resp = self.http_client
            .get(format!("{}/tools/list", self.base_url))
            .send()
            .await?;
        let list: McpListResponse = resp.json().await?;
        Ok(list.tools)
    }

    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<String> {
        let req = McpCallRequest {
            name: name.to_string(),
            arguments,
        };
        let resp = self.http_client
            .post(format!("{}/tools/call", self.base_url))
            .json(&req)
            .send()
            .await?;
        let result: McpCallResponse = resp.json().await?;
        let text = result.content.iter()
            .filter(|c| c.type_ == "text")
            .map(|c| c.text.clone())
            .collect::<Vec<_>>()
            .join("\n");
        Ok(text)
    }
}

/// Register all tools from an MCP server into a ToolRegistry
pub async fn register_mcp_tools(
    registry: &mut crate::tool::ToolRegistry,
    mcp_url: &str,
) -> Result<usize> {
    let client = McpClient::new(mcp_url);
    let tools = client.list_tools().await?;
    let count = tools.len();
    for tool in tools {
        let url = mcp_url.to_string();
        let name = tool.name.clone();
        registry.register(
            &name,
            &tool.description,
            tool.input_schema,
            std::sync::Arc::new(move |args, _run_id| {
                let url = url.clone();
                let name = name.clone();
                Box::pin(async move {
                    let client = McpClient::new(&url);
                    client.call_tool(&name, args).await
                })
            }),
        );
    }
    Ok(count)
}
