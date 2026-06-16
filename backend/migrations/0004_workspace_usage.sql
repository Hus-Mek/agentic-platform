CREATE TABLE IF NOT EXISTS workspace_usage (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    date        DATE NOT NULL DEFAULT CURRENT_DATE,
    total_tokens BIGINT NOT NULL DEFAULT 0,
    total_cost  NUMERIC(12,8) NOT NULL DEFAULT 0,
    total_runs  INTEGER NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(workspace_id, date)
);

CREATE INDEX IF NOT EXISTS idx_workspace_usage_workspace_id ON workspace_usage(workspace_id);
CREATE INDEX IF NOT EXISTS idx_workspace_usage_date ON workspace_usage(date);

DROP TRIGGER IF EXISTS trg_workspace_usage_updated_at ON workspace_usage;
CREATE TRIGGER trg_workspace_usage_updated_at
    BEFORE UPDATE ON workspace_usage
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
