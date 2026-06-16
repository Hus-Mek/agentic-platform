-- 0001_initial.sql — SQLx migration
-- Up: Creates all tables, indexes, RLS, and helper functions

-- Extensions
CREATE EXTENSION IF NOT EXISTS "pgcrypto";
CREATE EXTENSION IF NOT EXISTS "vector";

-- Supabase-compatible auth schema stub (harmless if real auth schema exists)
CREATE SCHEMA IF NOT EXISTS auth;
CREATE OR REPLACE FUNCTION auth.uid() RETURNS UUID LANGUAGE SQL STABLE AS $$ SELECT NULL::UUID; $$;

-- Updated_at trigger
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = now();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Workspaces
CREATE TABLE IF NOT EXISTS workspaces (
  id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name       TEXT NOT NULL,
  slug       TEXT NOT NULL UNIQUE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

DROP TRIGGER IF EXISTS trg_workspaces_updated_at ON workspaces;
CREATE TRIGGER trg_workspaces_updated_at
  BEFORE UPDATE ON workspaces
  FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Workspace members
DO $$ BEGIN
  CREATE TYPE member_role AS ENUM ('owner', 'admin', 'member', 'viewer');
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

CREATE TABLE IF NOT EXISTS workspace_members (
  id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  user_id      UUID NOT NULL,
  role         member_role NOT NULL DEFAULT 'member',
  created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE(workspace_id, user_id)
);

-- API keys
CREATE TABLE IF NOT EXISTS api_keys (
  id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  name         TEXT NOT NULL,
  key_hash     TEXT NOT NULL UNIQUE,
  key_prefix   TEXT NOT NULL,
  scope        TEXT NOT NULL DEFAULT 'write',
  is_active    BOOLEAN NOT NULL DEFAULT true,
  created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  last_used_at TIMESTAMPTZ,
  expires_at   TIMESTAMPTZ
);

-- Agents
CREATE TABLE IF NOT EXISTS agents (
  id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  workspace_id   UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  name           TEXT NOT NULL,
  description    TEXT,
  instructions   TEXT NOT NULL DEFAULT '',
  model          TEXT NOT NULL DEFAULT 'gpt-4o',
  tools          JSONB NOT NULL DEFAULT '[]'::jsonb,
  guard_config   JSONB,
  memory_config  JSONB,
  created_by     UUID,
  is_active      BOOLEAN NOT NULL DEFAULT true,
  created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at     TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_agents_workspace_id ON agents(workspace_id);
CREATE INDEX IF NOT EXISTS idx_agents_is_active ON agents(is_active) WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS trg_agents_updated_at ON agents;
CREATE TRIGGER trg_agents_updated_at
  BEFORE UPDATE ON agents
  FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Agent versions
CREATE TABLE IF NOT EXISTS agent_versions (
  id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  agent_id        UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
  version_number  INTEGER NOT NULL,
  config_snapshot JSONB NOT NULL,
  created_by      UUID,
  created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE(agent_id, version_number)
);

CREATE INDEX IF NOT EXISTS idx_agent_versions_agent_id ON agent_versions(agent_id);

-- Runs
DO $$ BEGIN
  CREATE TYPE run_status AS ENUM ('pending', 'running', 'completed', 'failed', 'cancelled');
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

CREATE TABLE IF NOT EXISTS runs (
  id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  agent_id     UUID REFERENCES agents(id) ON DELETE SET NULL,
  status       run_status NOT NULL DEFAULT 'pending',
  input        TEXT,
  output       TEXT,
  error        TEXT,
  duration_ms  BIGINT,
  total_tokens INT,
  total_cost   NUMERIC(12,8),
  created_by   UUID,
  created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_runs_workspace_id ON runs(workspace_id);
CREATE INDEX IF NOT EXISTS idx_runs_agent_id ON runs(agent_id);
CREATE INDEX IF NOT EXISTS idx_runs_status ON runs(status);
CREATE INDEX IF NOT EXISTS idx_runs_created_at ON runs(created_at DESC);

DROP TRIGGER IF EXISTS trg_runs_updated_at ON runs;
CREATE TRIGGER trg_runs_updated_at
  BEFORE UPDATE ON runs
  FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Run steps
DO $$ BEGIN
  CREATE TYPE step_type AS ENUM ('plan', 'tool', 'evaluate', 'guard', 'message', 'other');
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

CREATE TABLE IF NOT EXISTS run_steps (
  id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  run_id      UUID NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
  step_type   step_type NOT NULL,
  node_name   TEXT,
  input       TEXT,
  output      TEXT,
  duration_ms BIGINT,
  tokens_used INT,
  metadata    JSONB,
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_run_steps_run_id ON run_steps(run_id);

-- Knowledge bases
CREATE TABLE IF NOT EXISTS knowledge_bases (
  id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  workspace_id     UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  name             TEXT NOT NULL,
  description      TEXT,
  embedding_model  TEXT NOT NULL DEFAULT 'text-embedding-3-small',
  chunk_size       INTEGER NOT NULL DEFAULT 512,
  chunk_overlap    INTEGER NOT NULL DEFAULT 64,
  created_by       UUID,
  created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_knowledge_bases_workspace_id ON knowledge_bases(workspace_id);

-- KB documents
CREATE TABLE IF NOT EXISTS kb_documents (
  id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  knowledge_base_id UUID NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
  filename          TEXT NOT NULL,
  content_type      TEXT,
  content           TEXT,
  metadata          JSONB,
  created_by        UUID,
  created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_kb_documents_kb_id ON kb_documents(knowledge_base_id);

-- KB chunks
CREATE TABLE IF NOT EXISTS kb_chunks (
  id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  document_id UUID NOT NULL REFERENCES kb_documents(id) ON DELETE CASCADE,
  chunk_index INTEGER NOT NULL,
  content     TEXT NOT NULL,
  embedding   VECTOR(1536),
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_kb_chunks_document_id ON kb_chunks(document_id);
CREATE INDEX IF NOT EXISTS idx_kb_chunks_embedding ON kb_chunks USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

-- Helper functions
CREATE OR REPLACE FUNCTION get_user_workspaces(p_user_id UUID DEFAULT auth.uid())
RETURNS TABLE(workspace_id UUID, workspace_name TEXT, workspace_slug TEXT, member_role member_role, member_since TIMESTAMPTZ)
LANGUAGE plpgsql SECURITY DEFINER AS $$
BEGIN
  RETURN QUERY
  SELECT w.id, w.name, w.slug, wm.role, wm.created_at
  FROM workspaces w
  JOIN workspace_members wm ON wm.workspace_id = w.id
  WHERE wm.user_id = p_user_id
  ORDER BY wm.created_at DESC;
END;
$$;

CREATE OR REPLACE FUNCTION check_workspace_access(
  p_workspace_id UUID,
  p_user_id      UUID DEFAULT auth.uid(),
  p_min_role     member_role DEFAULT 'viewer'
)
RETURNS BOOLEAN
LANGUAGE plpgsql
SECURITY DEFINER
AS $$
DECLARE
  v_role member_role;
  role_rank TEXT[];
  min_rank INT;
  user_rank INT;
BEGIN
  SELECT role INTO v_role
  FROM workspace_members
  WHERE workspace_id = p_workspace_id AND user_id = p_user_id;

  IF v_role IS NULL THEN RETURN FALSE; END IF;

  role_rank := ARRAY['viewer', 'member', 'admin', 'owner'];
  min_rank := array_position(role_rank, p_min_role::TEXT);
  user_rank := array_position(role_rank, v_role::TEXT);

  RETURN user_rank >= min_rank;
END;
$$;

CREATE OR REPLACE FUNCTION create_workspace_with_owner(p_name TEXT, p_slug TEXT, p_owner_id UUID DEFAULT auth.uid())
RETURNS UUID
LANGUAGE plpgsql SECURITY DEFINER AS $$
DECLARE v_workspace_id UUID;
BEGIN
  INSERT INTO workspaces (name, slug) VALUES (p_name, p_slug) RETURNING id INTO v_workspace_id;
  INSERT INTO workspace_members (workspace_id, user_id, role) VALUES (v_workspace_id, p_owner_id, 'owner');
  RETURN v_workspace_id;
END;
$$;
