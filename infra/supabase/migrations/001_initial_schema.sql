-- 001_initial_schema.sql
-- AgenticOS Platform — Supabase-compatible PostgreSQL schema
-- Works with any PostgreSQL 15+

-- ============================================================
-- EXTENSIONS
-- ============================================================
CREATE EXTENSION IF NOT EXISTS "pgcrypto";
CREATE EXTENSION IF NOT EXISTS "vector";

-- ============================================================
-- HELPER: updated_at trigger function
-- ============================================================
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = now();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- ============================================================
-- WORKSPACES
-- ============================================================
CREATE TABLE workspaces (
  id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name       TEXT NOT NULL,
  slug       TEXT NOT NULL UNIQUE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER trg_workspaces_updated_at
  BEFORE UPDATE ON workspaces
  FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ============================================================
-- WORKSPACE MEMBERS
-- ============================================================
CREATE TYPE member_role AS ENUM ('owner', 'admin', 'member', 'viewer');

CREATE TABLE workspace_members (
  id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  user_id      UUID NOT NULL,
  role         member_role NOT NULL DEFAULT 'member',
  created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE(workspace_id, user_id)
);

-- ============================================================
-- API KEYS
-- ============================================================
CREATE TABLE api_keys (
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

-- ============================================================
-- AGENTS
-- ============================================================
CREATE TABLE agents (
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

CREATE INDEX idx_agents_workspace_id ON agents(workspace_id);
CREATE INDEX idx_agents_is_active ON agents(is_active) WHERE deleted_at IS NULL;

CREATE TRIGGER trg_agents_updated_at
  BEFORE UPDATE ON agents
  FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ============================================================
-- AGENT VERSIONS
-- ============================================================
CREATE TABLE agent_versions (
  id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  agent_id        UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
  version_number  INTEGER NOT NULL,
  config_snapshot JSONB NOT NULL,
  created_by      UUID,
  created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE(agent_id, version_number)
);

CREATE INDEX idx_agent_versions_agent_id ON agent_versions(agent_id);

-- ============================================================
-- RUNS
-- ============================================================
CREATE TYPE run_status AS ENUM ('pending', 'running', 'completed', 'failed', 'cancelled');

CREATE TABLE runs (
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

CREATE INDEX idx_runs_workspace_id ON runs(workspace_id);
CREATE INDEX idx_runs_agent_id ON runs(agent_id);
CREATE INDEX idx_runs_status ON runs(status);
CREATE INDEX idx_runs_created_at ON runs(created_at DESC);

CREATE TRIGGER trg_runs_updated_at
  BEFORE UPDATE ON runs
  FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ============================================================
-- RUN STEPS
-- ============================================================
CREATE TYPE step_type AS ENUM ('plan', 'tool', 'evaluate', 'guard', 'message', 'other');

CREATE TABLE run_steps (
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

CREATE INDEX idx_run_steps_run_id ON run_steps(run_id);

-- ============================================================
-- KNOWLEDGE BASES
-- ============================================================
CREATE TABLE knowledge_bases (
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

CREATE INDEX idx_knowledge_bases_workspace_id ON knowledge_bases(workspace_id);

-- ============================================================
-- KB DOCUMENTS
-- ============================================================
CREATE TABLE kb_documents (
  id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  knowledge_base_id UUID NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
  filename          TEXT NOT NULL,
  content_type      TEXT,
  content           TEXT,
  metadata          JSONB,
  created_by        UUID,
  created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_kb_documents_kb_id ON kb_documents(knowledge_base_id);

-- ============================================================
-- KB CHUNKS (with vector embedding)
-- ============================================================
CREATE TABLE kb_chunks (
  id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  document_id UUID NOT NULL REFERENCES kb_documents(id) ON DELETE CASCADE,
  chunk_index INTEGER NOT NULL,
  content     TEXT NOT NULL,
  embedding   VECTOR(1536),
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_kb_chunks_document_id ON kb_chunks(document_id);
CREATE INDEX idx_kb_chunks_embedding ON kb_chunks USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

-- ============================================================
-- ROW LEVEL SECURITY
-- ============================================================
ALTER TABLE workspaces ENABLE ROW LEVEL SECURITY;
ALTER TABLE workspace_members ENABLE ROW LEVEL SECURITY;
ALTER TABLE api_keys ENABLE ROW LEVEL SECURITY;
ALTER TABLE agents ENABLE ROW LEVEL SECURITY;
ALTER TABLE agent_versions ENABLE ROW LEVEL SECURITY;
ALTER TABLE runs ENABLE ROW LEVEL SECURITY;
ALTER TABLE run_steps ENABLE ROW LEVEL SECURITY;
ALTER TABLE knowledge_bases ENABLE ROW LEVEL SECURITY;
ALTER TABLE kb_documents ENABLE ROW LEVEL SECURITY;
ALTER TABLE kb_chunks ENABLE ROW LEVEL SECURITY;

-- ============================================================
-- RLS POLICIES — Workspaces
-- ============================================================
CREATE POLICY "workspaces_select_member" ON workspaces
  FOR SELECT USING (
    id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid())
  );

CREATE POLICY "workspaces_insert_admin" ON workspaces
  FOR INSERT WITH CHECK (
    auth.uid() IS NOT NULL
  );

CREATE POLICY "workspaces_update_owner" ON workspaces
  FOR UPDATE USING (
    id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role = 'owner')
  );

CREATE POLICY "workspaces_delete_owner" ON workspaces
  FOR DELETE USING (
    id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role = 'owner')
  );

-- ============================================================
-- RLS POLICIES — Workspace Members
-- ============================================================
CREATE POLICY "members_select_workspace_members" ON workspace_members
  FOR SELECT USING (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid())
  );

CREATE POLICY "members_insert_admin" ON workspace_members
  FOR INSERT WITH CHECK (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin'))
  );

CREATE POLICY "members_update_admin" ON workspace_members
  FOR UPDATE USING (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin'))
  );

CREATE POLICY "members_delete_admin" ON workspace_members
  FOR DELETE USING (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin'))
  );

-- ============================================================
-- RLS POLICIES — API Keys
-- ============================================================
CREATE POLICY "api_keys_select" ON api_keys
  FOR SELECT USING (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid())
  );

CREATE POLICY "api_keys_insert_admin" ON api_keys
  FOR INSERT WITH CHECK (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin'))
  );

CREATE POLICY "api_keys_update_admin" ON api_keys
  FOR UPDATE USING (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin'))
  );

CREATE POLICY "api_keys_delete_admin" ON api_keys
  FOR DELETE USING (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin'))
  );

-- ============================================================
-- RLS POLICIES — Agents
-- ============================================================
CREATE POLICY "agents_select" ON agents
  FOR SELECT USING (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid())
  );

CREATE POLICY "agents_insert_write" ON agents
  FOR INSERT WITH CHECK (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin', 'member'))
  );

CREATE POLICY "agents_update_write" ON agents
  FOR UPDATE USING (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin', 'member'))
  );

CREATE POLICY "agents_delete_admin" ON agents
  FOR DELETE USING (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin'))
  );

-- ============================================================
-- RLS POLICIES — Agent Versions
-- ============================================================
CREATE POLICY "agent_versions_select" ON agent_versions
  FOR SELECT USING (
    agent_id IN (SELECT id FROM agents WHERE workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid()))
  );

CREATE POLICY "agent_versions_insert" ON agent_versions
  FOR INSERT WITH CHECK (
    agent_id IN (SELECT id FROM agents WHERE workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin', 'member')))
  );

-- ============================================================
-- RLS POLICIES — Runs
-- ============================================================
CREATE POLICY "runs_select" ON runs
  FOR SELECT USING (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid())
  );

CREATE POLICY "runs_insert" ON runs
  FOR INSERT WITH CHECK (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin', 'member'))
  );

CREATE POLICY "runs_update" ON runs
  FOR UPDATE USING (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin', 'member'))
  );

-- ============================================================
-- RLS POLICIES — Run Steps
-- ============================================================
CREATE POLICY "run_steps_select" ON run_steps
  FOR SELECT USING (
    run_id IN (SELECT id FROM runs WHERE workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid()))
  );

CREATE POLICY "run_steps_insert" ON run_steps
  FOR INSERT WITH CHECK (
    run_id IN (SELECT id FROM runs WHERE workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin', 'member')))
  );

-- ============================================================
-- RLS POLICIES — Knowledge Bases
-- ============================================================
CREATE POLICY "kb_select" ON knowledge_bases
  FOR SELECT USING (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid())
  );

CREATE POLICY "kb_insert_write" ON knowledge_bases
  FOR INSERT WITH CHECK (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin', 'member'))
  );

CREATE POLICY "kb_update_write" ON knowledge_bases
  FOR UPDATE USING (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin', 'member'))
  );

CREATE POLICY "kb_delete_admin" ON knowledge_bases
  FOR DELETE USING (
    workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin'))
  );

-- ============================================================
-- RLS POLICIES — KB Documents
-- ============================================================
CREATE POLICY "kb_docs_select" ON kb_documents
  FOR SELECT USING (
    knowledge_base_id IN (SELECT id FROM knowledge_bases WHERE workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid()))
  );

CREATE POLICY "kb_docs_insert" ON kb_documents
  FOR INSERT WITH CHECK (
    knowledge_base_id IN (SELECT id FROM knowledge_bases WHERE workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin', 'member')))
  );

CREATE POLICY "kb_docs_delete" ON kb_documents
  FOR DELETE USING (
    knowledge_base_id IN (SELECT id FROM knowledge_bases WHERE workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin')))
  );

-- ============================================================
-- RLS POLICIES — KB Chunks
-- ============================================================
CREATE POLICY "kb_chunks_select" ON kb_chunks
  FOR SELECT USING (
    document_id IN (SELECT id FROM kb_documents WHERE knowledge_base_id IN (SELECT id FROM knowledge_bases WHERE workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid())))
  );

CREATE POLICY "kb_chunks_insert" ON kb_chunks
  FOR INSERT WITH CHECK (
    document_id IN (SELECT id FROM kb_documents WHERE knowledge_base_id IN (SELECT id FROM knowledge_bases WHERE workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = auth.uid() AND role IN ('owner', 'admin', 'member')))
  );

-- ============================================================
-- HELPER: get_user_workspaces
-- ============================================================
CREATE OR REPLACE FUNCTION get_user_workspaces(p_user_id UUID DEFAULT auth.uid())
RETURNS TABLE(
  workspace_id    UUID,
  workspace_name  TEXT,
  workspace_slug  TEXT,
  member_role     member_role,
  member_since    TIMESTAMPTZ
)
LANGUAGE plpgsql
SECURITY DEFINER
AS $$
BEGIN
  RETURN QUERY
  SELECT
    w.id,
    w.name,
    w.slug,
    wm.role,
    wm.created_at
  FROM workspaces w
  JOIN workspace_members wm ON wm.workspace_id = w.id
  WHERE wm.user_id = p_user_id
  ORDER BY wm.created_at DESC;
END;
$$;

-- ============================================================
-- HELPER: check_workspace_access
-- ============================================================
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
  role_rank INT[];
  min_rank INT;
  user_rank INT;
BEGIN
  SELECT role INTO v_role
  FROM workspace_members
  WHERE workspace_id = p_workspace_id AND user_id = p_user_id;

  IF v_role IS NULL THEN
    RETURN FALSE;
  END IF;

  role_rank := ARRAY['viewer', 'member', 'admin', 'owner'];
  min_rank := array_position(role_rank, p_min_role::TEXT);
  user_rank := array_position(role_rank, v_role::TEXT);

  RETURN user_rank >= min_rank;
END;
$$;

-- ============================================================
-- HELPER: create_workspace_with_owner
-- ============================================================
CREATE OR REPLACE FUNCTION create_workspace_with_owner(
  p_name       TEXT,
  p_slug       TEXT,
  p_owner_id   UUID DEFAULT auth.uid()
)
RETURNS UUID
LANGUAGE plpgsql
SECURITY DEFINER
AS $$
DECLARE
  v_workspace_id UUID;
BEGIN
  INSERT INTO workspaces (name, slug)
  VALUES (p_name, p_slug)
  RETURNING id INTO v_workspace_id;

  INSERT INTO workspace_members (workspace_id, user_id, role)
  VALUES (v_workspace_id, p_owner_id, 'owner');

  RETURN v_workspace_id;
END;
$$;
