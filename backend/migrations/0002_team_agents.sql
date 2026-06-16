-- 0002_team_agents.sql — SQLx migration
-- Up: Adds team_agents and team_runs tables for multi-agent supervisor support

-- Team agents
CREATE TABLE IF NOT EXISTS team_agents (
  id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  workspace_id            UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  name                    TEXT NOT NULL,
  description             TEXT,
  supervisor_instructions TEXT NOT NULL,
  supervisor_model        TEXT NOT NULL DEFAULT 'gpt-4o',
  sub_agents              JSONB NOT NULL DEFAULT '[]'::jsonb,
  max_delegations         INTEGER NOT NULL DEFAULT 10,
  execution_mode          TEXT NOT NULL DEFAULT 'Sequential',
  created_by              UUID,
  is_active               BOOLEAN NOT NULL DEFAULT true,
  created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_team_agents_workspace_id ON team_agents(workspace_id);

DROP TRIGGER IF EXISTS trg_team_agents_updated_at ON team_agents;
CREATE TRIGGER trg_team_agents_updated_at
  BEFORE UPDATE ON team_agents
  FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Team runs
CREATE TABLE IF NOT EXISTS team_runs (
  id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  team_id       UUID NOT NULL REFERENCES team_agents(id) ON DELETE CASCADE,
  workspace_id  UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  status        TEXT NOT NULL DEFAULT 'running',
  input         TEXT,
  output        TEXT,
  error         TEXT,
  delegations   JSONB NOT NULL DEFAULT '[]'::jsonb,
  total_tokens  INT DEFAULT 0,
  total_cost    NUMERIC(12,8) DEFAULT 0,
  duration_ms   BIGINT,
  created_by    UUID,
  created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_team_runs_team_id ON team_runs(team_id);
CREATE INDEX IF NOT EXISTS idx_team_runs_workspace_id ON team_runs(workspace_id);
CREATE INDEX IF NOT EXISTS idx_team_runs_status ON team_runs(status);

DROP TRIGGER IF EXISTS trg_team_runs_updated_at ON team_runs;
CREATE TRIGGER trg_team_runs_updated_at
  BEFORE UPDATE ON team_runs
  FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
