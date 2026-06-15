-- 0001_initial_down.sql — SQLx down migration
-- Drops all tables, types, and functions created in the up migration
-- Run this to roll back: sqlx migrate revert

-- Drop helper functions
DROP FUNCTION IF EXISTS create_workspace_with_owner(TEXT, TEXT, UUID);
DROP FUNCTION IF EXISTS check_workspace_access(UUID, UUID, TEXT);
DROP FUNCTION IF EXISTS get_user_workspaces(UUID);

-- Drop tables (order matters for FK constraints)
DROP TABLE IF EXISTS kb_chunks;
DROP TABLE IF EXISTS kb_documents;
DROP TABLE IF EXISTS knowledge_bases;
DROP TABLE IF EXISTS run_steps;
DROP TABLE IF EXISTS runs;
DROP TABLE IF EXISTS agent_versions;
DROP TABLE IF EXISTS agents;
DROP TABLE IF EXISTS api_keys;
DROP TABLE IF EXISTS workspace_members;
DROP TABLE IF EXISTS workspaces;

-- Drop types
DROP TYPE IF EXISTS step_type;
DROP TYPE IF EXISTS run_status;
DROP TYPE IF EXISTS member_role;

-- Drop triggers
DROP TRIGGER IF EXISTS trg_workspaces_updated_at ON workspaces;
DROP TRIGGER IF EXISTS trg_agents_updated_at ON agents;
DROP TRIGGER IF EXISTS trg_runs_updated_at ON runs;

-- Drop function
DROP FUNCTION IF EXISTS update_updated_at_column();

-- Extensions are preserved (removing them could affect other databases)
-- Only drop if you're certain: DROP EXTENSION IF EXISTS "vector"; DROP EXTENSION IF EXISTS "pgcrypto";
