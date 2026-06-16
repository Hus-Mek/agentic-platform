-- ============================================================
-- Supabase Migration: 20260615000002_team_agents.sql
-- Source: infra/supabase/migrations/002_team_agents.sql
-- Adds TeamAgent multi-agent supervisor support.
-- ============================================================

-- Team agents
CREATE TABLE team_agents (
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

CREATE INDEX idx_team_agents_workspace_id ON team_agents(workspace_id);

CREATE TRIGGER trg_team_agents_updated_at
  BEFORE UPDATE ON team_agents
  FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Team runs
CREATE TABLE team_runs (
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

CREATE INDEX idx_team_runs_team_id ON team_runs(team_id);
CREATE INDEX idx_team_runs_workspace_id ON team_runs(workspace_id);
CREATE INDEX idx_team_runs_status ON team_runs(status);

CREATE TRIGGER trg_team_runs_updated_at
  BEFORE UPDATE ON team_runs
  FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Row-Level Security
ALTER TABLE team_agents ENABLE ROW LEVEL SECURITY;
ALTER TABLE team_runs ENABLE ROW LEVEL SECURITY;

-- RLS policies — Team Agents
CREATE POLICY "team_agents_select" ON team_agents
  FOR SELECT USING (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid())
  );

CREATE POLICY "team_agents_insert_write" ON team_agents
  FOR INSERT WITH CHECK (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin', 'member'))
  );

CREATE POLICY "team_agents_update_write" ON team_agents
  FOR UPDATE USING (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin', 'member'))
  );

CREATE POLICY "team_agents_delete_admin" ON team_agents
  FOR DELETE USING (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin'))
  );

-- RLS policies — Team Runs
CREATE POLICY "team_runs_select" ON team_runs
  FOR SELECT USING (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid())
  );

CREATE POLICY "team_runs_insert_write" ON team_runs
  FOR INSERT WITH CHECK (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin', 'member'))
  );

CREATE POLICY "team_runs_update_write" ON team_runs
  FOR UPDATE USING (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin', 'member'))
  );
