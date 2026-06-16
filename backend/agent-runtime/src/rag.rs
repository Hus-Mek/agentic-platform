use agenticos_core::llm::LiteLLMClient;

pub struct KbRetriever {
    db: sqlx::PgPool,
    llm_client: LiteLLMClient,
}

impl KbRetriever {
    pub fn new(db: sqlx::PgPool, llm_client: LiteLLMClient) -> Self {
        Self { db, llm_client }
    }

    pub async fn retrieve(
        &self,
        query: &str,
        workspace_id: &str,
        limit: u64,
    ) -> anyhow::Result<Vec<String>> {
        match self.llm_client.embed_text(query).await {
            Ok(embedding) => {
                let emb_str: String = format!(
                    "[{}]",
                    embedding
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                );

                let rows = sqlx::query_as::<_, (String, String)>(
                    r#"
                    SELECT c.content, kb.name
                    FROM kb_chunks c
                    JOIN kb_documents d ON c.document_id = d.id
                    JOIN knowledge_bases kb ON d.knowledge_base_id = kb.id
                    WHERE kb.workspace_id = $1::uuid
                    ORDER BY c.embedding <=> $2::vector ASC
                    LIMIT $3
                    "#,
                )
                .bind(workspace_id)
                .bind(&emb_str)
                .bind(limit as i64)
                .fetch_all(&self.db)
                .await
                .map_err(|e| anyhow::anyhow!("KB vector search failed: {e}"))?;

                Ok(rows
                    .into_iter()
                    .map(|(content, kb_name)| format!("[KB: {}] {}", kb_name, content))
                    .collect())
            }
            Err(e) => {
                tracing::warn!("KB embedding failed, using ILIKE fallback: {e}");

                let rows = sqlx::query_as::<_, (String, String)>(
                    r#"
                    SELECT c.content, kb.name
                    FROM kb_chunks c
                    JOIN kb_documents d ON c.document_id = d.id
                    JOIN knowledge_bases kb ON d.knowledge_base_id = kb.id
                    WHERE kb.workspace_id = $1::uuid
                    AND c.content ILIKE '%' || $2 || '%'
                    LIMIT $3
                    "#,
                )
                .bind(workspace_id)
                .bind(query)
                .bind(limit as i64)
                .fetch_all(&self.db)
                .await
                .map_err(|e| anyhow::anyhow!("KB fallback search failed: {e}"))?;

                Ok(rows
                    .into_iter()
                    .map(|(content, kb_name)| format!("[KB: {}] {}", kb_name, content))
                    .collect())
            }
        }
    }
}
