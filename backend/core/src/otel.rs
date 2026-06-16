use opentelemetry::trace::TracerProvider;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub fn init_otel(service_name: &str) -> anyhow::Result<()> {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .build()?;

    let provider = opentelemetry_sdk::trace::TracerProvider::builder()
        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
        .build();

    let tracer = provider.tracer(service_name.to_string());

    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(telemetry)
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .ok(); // Ignore error if already initialized

    Ok(())
}

/// Create a span for an agent run
pub fn agent_run_span(agent_id: &str, run_id: &str, workspace_id: &str) -> tracing::Span {
    tracing::info_span!(
        "agent.run",
        agent_id = agent_id,
        run_id = run_id,
        workspace_id = workspace_id,
    )
}

/// Create a span for a tool call
pub fn tool_call_span(tool_name: &str, run_id: &str) -> tracing::Span {
    tracing::info_span!(
        "agent.tool_call",
        tool_name = tool_name,
        run_id = run_id,
    )
}

/// Create a span for a plan step
pub fn plan_step_span(iteration: u32, model: &str) -> tracing::Span {
    tracing::info_span!(
        "agent.plan",
        iteration = iteration,
        model = model,
    )
}
