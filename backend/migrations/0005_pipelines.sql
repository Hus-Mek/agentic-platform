-- 0005_pipelines.sql — Workflow Designer persistence
-- Up: pipelines, pipeline_versions, pipeline_runs; extends run_steps for pipeline node trace.

-- Pipelines (workflow definitions)
CREATE TABLE IF NOT EXISTS pipelines (
  id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  name         TEXT NOT NULL,
  description  TEXT,
  version      TEXT NOT NULL DEFAULT '1.0',
  graph        JSONB NOT NULL DEFAULT '{"nodes":[],"edges":[]}'::jsonb,
  created_by   UUID,
  is_active    BOOLEAN NOT NULL DEFAULT true,
  created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at   TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_pipelines_workspace_id ON pipelines(workspace_id);
CREATE INDEX IF NOT EXISTS idx_pipelines_is_active ON pipelines(is_active) WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS trg_pipelines_updated_at ON pipelines;
CREATE TRIGGER trg_pipelines_updated_at
  BEFORE UPDATE ON pipelines
  FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Pipeline versions (config snapshots — mirrors agent_versions)
CREATE TABLE IF NOT EXISTS pipeline_versions (
  id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  pipeline_id     UUID NOT NULL REFERENCES pipelines(id) ON DELETE CASCADE,
  version_number  INTEGER NOT NULL,
  config_snapshot JSONB NOT NULL,
  created_by      UUID,
  created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE(pipeline_id, version_number)
);

CREATE INDEX IF NOT EXISTS idx_pipeline_versions_pipeline_id ON pipeline_versions(pipeline_id);

-- Pipeline runs (mirrors runs columns so workspace_usage rollup can UNION it)
CREATE TABLE IF NOT EXISTS pipeline_runs (
  id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  pipeline_id  UUID REFERENCES pipelines(id) ON DELETE SET NULL,
  status       run_status NOT NULL DEFAULT 'pending',
  input        JSONB,
  execution    JSONB,
  error        TEXT,
  duration_ms  BIGINT,
  total_tokens INT NOT NULL DEFAULT 0,
  total_cost   NUMERIC(12,8) NOT NULL DEFAULT 0,
  created_by   UUID,
  created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_pipeline_runs_workspace_id ON pipeline_runs(workspace_id);
CREATE INDEX IF NOT EXISTS idx_pipeline_runs_pipeline_id ON pipeline_runs(pipeline_id);
CREATE INDEX IF NOT EXISTS idx_pipeline_runs_status ON pipeline_runs(status);
CREATE INDEX IF NOT EXISTS idx_pipeline_runs_created_at ON pipeline_runs(created_at DESC);

DROP TRIGGER IF EXISTS trg_pipeline_runs_updated_at ON pipeline_runs;
CREATE TRIGGER trg_pipeline_runs_updated_at
  BEFORE UPDATE ON pipeline_runs
  FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Extend run_steps to also own pipeline-node trace rows.
-- A row belongs to exactly one of runs (agent) or pipeline_runs (workflow).
ALTER TABLE run_steps ADD COLUMN IF NOT EXISTS pipeline_run_id UUID REFERENCES pipeline_runs(id) ON DELETE CASCADE;
ALTER TABLE run_steps ALTER COLUMN run_id DROP NOT NULL;

DO $$ BEGIN
  ALTER TABLE run_steps ADD CONSTRAINT chk_run_steps_owner
    CHECK (num_nonnulls(run_id, pipeline_run_id) = 1);
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

CREATE INDEX IF NOT EXISTS idx_run_steps_pipeline_run_id ON run_steps(pipeline_run_id);

-- Reconcile workspace_usage with the column the code actually reads/writes
-- (`request_count`); 0004 shipped `total_runs`. Idempotent forward-fix so
-- pipeline + agent usage rollups land in the same place.
ALTER TABLE workspace_usage ADD COLUMN IF NOT EXISTS request_count INTEGER NOT NULL DEFAULT 0;
