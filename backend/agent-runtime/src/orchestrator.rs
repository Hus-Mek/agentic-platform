use std::sync::Arc;
use std::time::Instant;

use agenticos_core::state::*;
use futures::future;
use uuid::Uuid;

use crate::loop_mod::run_agent;
use crate::plan::PlanResult;
use crate::tool::{ToolImpl, ToolRegistry};

pub struct TeamOrchestrator {
    pub config: TeamConfig,
    pub llm_client: crate::plan::LiteLLMClient,
    pub tool_registry: Arc<ToolRegistry>,
    pub guard_engine: Arc<agenticos_guards::GuardEngine>,
    pub memory_client: Arc<agenticos_memory::MemoryClient>,
    pub db: sqlx::postgres::PgPool,
}

impl TeamOrchestrator {
    pub fn new(
        config: TeamConfig,
        llm_client: crate::plan::LiteLLMClient,
        tool_registry: Arc<ToolRegistry>,
        guard_engine: Arc<agenticos_guards::GuardEngine>,
        memory_client: Arc<agenticos_memory::MemoryClient>,
        db: sqlx::postgres::PgPool,
    ) -> Self {
        Self {
            config,
            llm_client,
            tool_registry,
            guard_engine,
            memory_client,
            db,
        }
    }

    pub async fn run(
        &self,
        input: &str,
        workspace_id: Option<String>,
        user_id: Option<String>,
    ) -> anyhow::Result<TeamRunState> {
        let run_id = format!("team_run_{}", Uuid::new_v4());
        let supervisor_prompt = self.build_supervisor_prompt();

        let mut supervisor_state = AgentState::new(
            Some(self.config.id.clone()),
            Some(run_id.clone()),
            workspace_id.clone(),
            user_id.clone(),
            &supervisor_prompt,
            &self.config.supervisor_model,
        );

        supervisor_state.add_message(Message::new("user", input));

        let delegate_tool = create_delegate_tool(&self.config.sub_agents);
        supervisor_state.tools.push(ToolDefinition {
            name: delegate_tool.name.clone(),
            description: delegate_tool.description.clone(),
            parameters: delegate_tool.schema.clone(),
            required: vec!["sub_agent_id".into(), "task".into()],
        });

        for tool in self.tool_registry.list() {
            let required = tool
                .schema
                .get("required")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            supervisor_state.tools.push(ToolDefinition {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.schema.clone(),
                required,
            });
        }

        let mut delegations: Vec<DelegationResult> = Vec::new();
        let mut delegation_count = 0u32;

        while !supervisor_state.should_stop() && delegation_count < self.config.max_delegations {
            let plan_result = crate::plan::plan_step(
                &mut supervisor_state,
                &self.llm_client,
                &self.memory_client,
                None,
            )
            .await?;

            match plan_result {
                PlanResult::Text(_) => {
                    break;
                }
                PlanResult::ToolCalls(tool_calls) => {
                    if self.config.execution_mode == ExecutionMode::Parallel
                        && tool_calls.iter().any(|tc| tc.name == "delegate")
                    {
                        let (delegates, regular): (Vec<_>, Vec<_>) =
                            tool_calls.into_iter().partition(|tc| tc.name == "delegate");

                        let sub_agents = self.config.sub_agents.clone();
                        let llm = self.llm_client.clone();
                        let registry = Arc::clone(&self.tool_registry);
                        let guards = Arc::clone(&self.guard_engine);
                        let memory = Arc::clone(&self.memory_client);
                        let db = self.db.clone();
                        let run_id_str = run_id.clone();
                        let ws_id = workspace_id.clone();
                        let uid = user_id.clone();

                        let mut handles = Vec::new();
                        for tc in &delegates {
                            let tc_args = tc.args.clone();
                            let sub_agent_id = tc_args
                                .get("sub_agent_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let task = tc_args
                                .get("task")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let sa_config = sub_agents.iter().find(|sa| sa.id == sub_agent_id).cloned();
                            let llm = llm.clone();
                            let reg = Arc::clone(&registry);
                            let ge = Arc::clone(&guards);
                            let mem = Arc::clone(&memory);
                            let pool = db.clone();
                            let pid = run_id_str.clone();
                            let wid = ws_id.clone();
                            let usr = uid.clone();

                            handles.push(tokio::spawn(async move {
                                match sa_config {
                                    Some(sa) => {
                                        let sub_run_id = format!("{}_{}", pid, sa.id);
                                        let mut sub_state = AgentState::new(
                                            Some(sa.id.clone()),
                                            Some(sub_run_id),
                                            wid.clone(),
                                            usr.clone(),
                                            &sa.instructions,
                                            &sa.model,
                                        );
                                        sub_state.max_iterations = 10;
                                        sub_state.tools = sa.tools.clone();
                                        sub_state.guard_config = sa.guard_config.clone();
                                        sub_state.add_message(Message::new("user", &task));

                                        let _ = crate::loop_mod::run_agent(
                                            &mut sub_state, &llm, &reg, &ge, &mem, None, &pool,
                                        )
                                        .await;

                                        let output = sub_state
                                            .messages
                                            .iter()
                                            .rev()
                                            .find(|m| m.role == "assistant" && !m.content.is_empty())
                                            .map(|m| m.content.clone())
                                            .unwrap_or_default();

                                        (
                                            tc_args.clone(),
                                            DelegationResult {
                                                sub_agent_id: sa.id,
                                                sub_agent_name: sa.name,
                                                task,
                                                result: output,
                                                tokens_used: sub_state.total_tokens_used,
                                                cost: sub_state.total_cost,
                                                duration_ms: 0,
                                                success: true,
                                                error: None,
                                            },
                                        )
                                    }
                                    None => (
                                        tc_args.clone(),
                                        DelegationResult {
                                            sub_agent_id: sub_agent_id.clone(),
                                            sub_agent_name: "unknown".to_string(),
                                            task,
                                            result: String::new(),
                                            tokens_used: 0,
                                            cost: 0.0,
                                            duration_ms: 0,
                                            success: false,
                                            error: Some(format!(
                                                "Sub-agent '{}' not found",
                                                sub_agent_id
                                            )),
                                        },
                                    ),
                                }
                            }));
                        }

                        let results = futures::future::join_all(handles).await;
                        for result in results {
                            match result {
                                Ok((args, delegation_result)) => {
                                    delegation_count += 1;
                                    let output = if delegation_result.success {
                                        delegation_result.result.clone()
                                    } else {
                                        format!(
                                            "Sub-agent error: {}",
                                            delegation_result
                                                .error
                                                .as_deref()
                                                .unwrap_or("unknown error")
                                        )
                                    };
                                    let tc_id = args
                                        .get("sub_agent_id")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("delegate");
                                    supervisor_state.add_message(Message::tool_result(
                                        "delegate",
                                        tc_id,
                                        &output,
                                    ));
                                    delegations.push(delegation_result);
                                }
                                Err(join_err) => {
                                    delegation_count += 1;
                                    let err_msg =
                                        format!("Sub-agent task panicked: {:?}", join_err);
                                    supervisor_state.add_message(Message::tool_result(
                                        "delegate", "unknown", &err_msg,
                                    ));
                                    delegations.push(DelegationResult {
                                        sub_agent_id: "unknown".into(),
                                        sub_agent_name: "unknown".into(),
                                        task: String::new(),
                                        result: String::new(),
                                        tokens_used: 0,
                                        cost: 0.0,
                                        duration_ms: 0,
                                        success: false,
                                        error: Some(err_msg),
                                    });
                                }
                            }
                        }

                        for tc in regular {
                            let mut core_tc =
                                agenticos_core::state::ToolCall::new(&tc.name, tc.args.clone());
                            let exec_result = self
                                .tool_registry
                                .execute(
                                    &mut core_tc,
                                    supervisor_state.workspace_id.as_deref(),
                                    supervisor_state.run_id.as_deref(),
                                    supervisor_state.agent_id.as_deref(),
                                )
                                .await;

                            match exec_result {
                                Ok(output) => {
                                    supervisor_state.add_message(Message::tool_result(
                                        &tc.name,
                                        &core_tc.id,
                                        &output,
                                    ));
                                }
                                Err(e) => {
                                    let err_msg = format!("{:?}", e);
                                    tracing::warn!(
                                        tool = %tc.name,
                                        error = %err_msg,
                                        "Regular tool execution failed"
                                    );
                                    supervisor_state.add_message(Message::tool_result(
                                        &tc.name,
                                        &core_tc.id,
                                        &err_msg,
                                    ));
                                }
                            }
                        }
                    } else {
                        for tc in &tool_calls {
                            if tc.name == "delegate" {
                                delegation_count += 1;
                                let sub_agent_id = tc
                                    .args
                                    .get("sub_agent_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let task = tc
                                    .args
                                    .get("task")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                tracing::info!(
                                    sub_agent_id = %sub_agent_id,
                                    task = %task,
                                    delegation = delegation_count,
                                    "Supervisor delegating task"
                                );

                                let sub_agent = self
                                    .config
                                    .sub_agents
                                    .iter()
                                    .find(|sa| sa.id == sub_agent_id)
                                    .cloned();

                                let delegate_result = match sub_agent {
                                    Some(sa) => {
                                        self.run_sub_agent(&sa, &task, &run_id, &workspace_id, &user_id)
                                            .await
                                    }
                                    None => DelegationResult {
                                        sub_agent_id: sub_agent_id.clone(),
                                        sub_agent_name: "unknown".to_string(),
                                        task: task.clone(),
                                        result: String::new(),
                                        tokens_used: 0,
                                        cost: 0.0,
                                        duration_ms: 0,
                                        success: false,
                                        error: Some(format!(
                                            "Sub-agent '{}' not found in team config",
                                            sub_agent_id
                                        )),
                                    },
                                };

                                let output = if delegate_result.success {
                                    delegate_result.result.clone()
                                } else {
                                    format!(
                                        "Sub-agent error: {}",
                                        delegate_result.error.as_deref().unwrap_or("unknown error")
                                    )
                                };

                                supervisor_state
                                    .add_message(Message::tool_result(&tc.name, &tc.id, &output));
                                delegations.push(delegate_result);
                            } else {
                                let mut core_tc =
                                    agenticos_core::state::ToolCall::new(&tc.name, tc.args.clone());
                                let exec_result = self
                                    .tool_registry
                                    .execute(
                                        &mut core_tc,
                                        supervisor_state.workspace_id.as_deref(),
                                        supervisor_state.run_id.as_deref(),
                                        supervisor_state.agent_id.as_deref(),
                                    )
                                    .await;

                                match exec_result {
                                    Ok(output) => {
                                        supervisor_state.add_message(Message::tool_result(
                                            &tc.name,
                                            &core_tc.id,
                                            &output,
                                        ));
                                    }
                                    Err(e) => {
                                        let err_msg = format!("{:?}", e);
                                        tracing::warn!(
                                            tool = %tc.name,
                                            error = %err_msg,
                                            "Regular tool execution failed"
                                        );
                                        supervisor_state.add_message(Message::tool_result(
                                            &tc.name,
                                            &core_tc.id,
                                            &err_msg,
                                        ));
                                    }
                                }
                            }
                        }

                        supervisor_state.increment_iteration();
                    }
                }
                PlanResult::Stop | PlanResult::Retry => {
                    break;
                }
            }
        }

        let total_tokens = supervisor_state.total_tokens_used
            + delegations.iter().map(|d| d.tokens_used).sum::<i64>();
        let total_cost = supervisor_state.total_cost
            + delegations.iter().map(|d| d.cost).sum::<f64>();

        tracing::info!(
            team_id = %self.config.id,
            delegations = delegations.len(),
            total_tokens = total_tokens,
            total_cost = total_cost,
            "Team run completed"
        );

        Ok(TeamRunState {
            team_id: self.config.id.clone(),
            run_id,
            workspace_id,
            user_id,
            input: input.to_string(),
            supervisor_state,
            delegations,
            status: "completed".to_string(),
            total_tokens_used: total_tokens,
            total_cost,
        })
    }

    async fn run_sub_agent(
        &self,
        config: &SubAgentConfig,
        task: &str,
        run_id: &str,
        workspace_id: &Option<String>,
        user_id: &Option<String>,
    ) -> DelegationResult {
        let start = Instant::now();
        let sub_run_id = format!("{}_{}", run_id, config.id);

        let mut sub_state = AgentState::new(
            Some(config.id.clone()),
            Some(sub_run_id),
            workspace_id.clone(),
            user_id.clone(),
            &config.instructions,
            &config.model,
        );
        sub_state.max_iterations = 10;
        sub_state.tools = config.tools.clone();
        sub_state.guard_config = config.guard_config.clone();
        sub_state.add_message(Message::new("user", task));

        tracing::info!(
            sub_agent_id = %config.id,
            sub_agent_name = %config.name,
            "Running sub-agent"
        );

        let agent_result = run_agent(
            &mut sub_state,
            &self.llm_client,
            self.tool_registry.as_ref(),
            self.guard_engine.as_ref(),
            self.memory_client.as_ref(),
            None,
            &self.db,
        )
        .await;

        let duration_ms = start.elapsed().as_millis() as i64;

        match agent_result {
            Ok(()) => {
                let output = sub_state
                    .messages
                    .iter()
                    .rev()
                    .find(|m| m.role == "assistant" && !m.content.is_empty())
                    .map(|m| m.content.clone())
                    .unwrap_or_default();

                tracing::info!(
                    sub_agent_id = %config.id,
                    tokens = sub_state.total_tokens_used,
                    cost = sub_state.total_cost,
                    duration_ms = duration_ms,
                    "Sub-agent completed successfully"
                );

                DelegationResult {
                    sub_agent_id: config.id.clone(),
                    sub_agent_name: config.name.clone(),
                    task: task.to_string(),
                    result: output,
                    tokens_used: sub_state.total_tokens_used,
                    cost: sub_state.total_cost,
                    duration_ms,
                    success: true,
                    error: None,
                }
            }
            Err(e) => {
                let err_msg = format!("{:?}", e);
                tracing::error!(
                    sub_agent_id = %config.id,
                    error = %err_msg,
                    "Sub-agent failed"
                );

                DelegationResult {
                    sub_agent_id: config.id.clone(),
                    sub_agent_name: config.name.clone(),
                    task: task.to_string(),
                    result: String::new(),
                    tokens_used: sub_state.total_tokens_used,
                    cost: sub_state.total_cost,
                    duration_ms,
                    success: false,
                    error: Some(err_msg),
                }
            }
        }
    }

    fn build_supervisor_prompt(&self) -> String {
        let mut prompt = self.config.supervisor_instructions.clone();

        prompt.push_str("\n\n## Available Sub-Agents\n");
        for sa in &self.config.sub_agents {
            let summary: String = sa.instructions.chars().take(200).collect();
            prompt.push_str(&format!(
                "\n- **{}** (id: `{}`): {}",
                sa.name, sa.id, summary
            ));
        }

        prompt.push_str(&format!(
            "\n\nYou have access to the `delegate` tool to assign tasks to sub-agents. \
             You may delegate up to {} times. Use sub-agents for specialized work \
             and synthesize their outputs yourself.",
            self.config.max_delegations
        ));

        prompt
    }
}

pub fn create_delegate_tool(sub_agents: &[SubAgentConfig]) -> ToolImpl {
    let agent_ids: Vec<String> = sub_agents.iter().map(|sa| sa.id.clone()).collect();
    let agent_descriptions: Vec<String> = sub_agents
        .iter()
        .map(|sa| {
            let summary: String = sa.instructions.chars().take(150).collect();
            format!("{}: {}", sa.name, summary)
        })
        .collect();

    ToolImpl {
        name: "delegate".into(),
        description: format!(
            "Delegate a task to a specialized sub-agent. \
             After delegation, you will receive the sub-agent's response. \
             Available sub-agents: {}",
            agent_descriptions.join("; ")
        ),
        schema: serde_json::json!({
            "type": "object",
            "properties": {
                "sub_agent_id": {
                    "type": "string",
                    "enum": agent_ids,
                    "description": "The ID of the sub-agent to delegate the task to"
                },
                "task": {
                    "type": "string",
                    "description": "A detailed description of the task to delegate"
                }
            },
            "required": ["sub_agent_id", "task"]
        }),
        handler: Arc::new(|_args| {
            Box::pin(async move {
                anyhow::bail!("delegate tool is handled by the orchestrator run loop directly")
            })
        }),
        metadata: crate::tool::ToolMetadata::default(),
    }
}

pub fn collect_team_stats(state: &TeamRunState) -> (i64, f64) {
    let total_tokens = state.delegations.iter().map(|d| d.tokens_used).sum();
    let total_cost = state.delegations.iter().map(|d| d.cost).sum();
    (total_tokens, total_cost)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_team_stats_empty() {
        let state = TeamRunState {
            team_id: "t1".into(),
            run_id: "r1".into(),
            workspace_id: None,
            user_id: None,
            input: "test".into(),
            supervisor_state: AgentState::new(None, None, None, None, "be helpful", "gpt-4"),
            delegations: vec![],
            status: "completed".into(),
            total_tokens_used: 0,
            total_cost: 0.0,
        };
        let (tokens, cost) = collect_team_stats(&state);
        assert_eq!(tokens, 0);
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_collect_team_stats_with_delegations() {
        let state = TeamRunState {
            team_id: "t1".into(),
            run_id: "r1".into(),
            workspace_id: None,
            user_id: None,
            input: "test".into(),
            supervisor_state: AgentState::new(None, None, None, None, "be helpful", "gpt-4"),
            delegations: vec![
                DelegationResult {
                    sub_agent_id: "s1".into(),
                    sub_agent_name: "Search".into(),
                    task: "find x".into(),
                    result: "found".into(),
                    tokens_used: 100,
                    cost: 0.002,
                    duration_ms: 500,
                    success: true,
                    error: None,
                },
                DelegationResult {
                    sub_agent_id: "s2".into(),
                    sub_agent_name: "Summarize".into(),
                    task: "summarize".into(),
                    result: "done".into(),
                    tokens_used: 200,
                    cost: 0.005,
                    duration_ms: 300,
                    success: true,
                    error: None,
                },
            ],
            status: "completed".into(),
            total_tokens_used: 0,
            total_cost: 0.0,
        };
        let (tokens, cost) = collect_team_stats(&state);
        assert_eq!(tokens, 300);
        assert!((cost - 0.007).abs() < 1e-6);
    }

    #[tokio::test]
    async fn test_create_delegate_tool_schema() {
        let agents = vec![
            SubAgentConfig {
                id: "searcher".into(),
                name: "Searcher".into(),
                instructions: "Search the web for information.".into(),
                model: "gpt-4".into(),
                tools: vec![],
                guard_config: GuardConfig::default(),
            },
            SubAgentConfig {
                id: "summarizer".into(),
                name: "Summarizer".into(),
                instructions: "Summarize the given text.".into(),
                model: "gpt-4".into(),
                tools: vec![],
                guard_config: GuardConfig::default(),
            },
        ];

        let tool = create_delegate_tool(&agents);
        assert_eq!(tool.name, "delegate");
        assert!(tool.description.contains("Searcher"));
        assert!(tool.description.contains("Summarizer"));

        let schema = &tool.schema;
        assert_eq!(schema["type"], "object");
        let agent_enum = schema["properties"]["sub_agent_id"]["enum"].as_array().unwrap();
        let ids: Vec<&str> = agent_enum.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(ids.contains(&"searcher"));
        assert!(ids.contains(&"summarizer"));
    }

    #[test]
    fn test_delegate_tool_description_includes_agents() {
        let agents = vec![
            SubAgentConfig {
                id: "searcher".into(),
                name: "Searcher".into(),
                instructions: "Search for information.".into(),
                model: "gpt-4".into(),
                tools: vec![],
                guard_config: GuardConfig::default(),
            },
        ];
        let tool = create_delegate_tool(&agents);
        assert!(tool.description.contains("Searcher"));
        assert!(tool.description.contains("Delegate a task"));
    }

    #[test]
    fn test_sub_agent_default() {
        let sa = SubAgentConfig::default();
        assert_eq!(sa.model, "gpt-4");
        assert!(sa.tools.is_empty());
    }

    #[test]
    fn test_team_config_default() {
        let tc = TeamConfig::default();
        assert_eq!(tc.max_delegations, 10);
        assert!(matches!(tc.execution_mode, ExecutionMode::Sequential));
    }
}
