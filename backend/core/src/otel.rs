// OpenTelemetry span helpers — creates tracing spans with OTel-compatible names.
// The actual OTel exporter is initialized separately when the OTLP endpoint is configured.

pub fn init_otel(_service_name: &str) -> anyhow::Result<()> {
    // No-op for now — OTel initialization requires an OTLP collector endpoint.
    // Call this from main.rs to set up the exporter when ready.
    Ok(())
}

pub fn agent_run_span(agent_id: &str, run_id: &str, workspace_id: &str) -> tracing::Span {
    tracing::info_span!("agent.run", agent_id = agent_id, run_id = run_id, workspace_id = workspace_id)
}

pub fn tool_call_span(tool_name: &str, run_id: &str) -> tracing::Span {
    tracing::info_span!("agent.tool_call", tool_name = tool_name, run_id = run_id)
}

pub fn plan_step_span(iteration: u32, model: &str) -> tracing::Span {
    tracing::info_span!("agent.plan", iteration = iteration, model = model)
}
