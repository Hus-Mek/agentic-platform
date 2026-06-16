-- ============================================================
-- supabase/seed.sql — AgenticOS Platform
-- Dev seed data for local Supabase environment.
-- Runs AFTER all migrations when `supabase db reset` or
-- `supabase db push` is executed (with seed.enabled = true).
-- ============================================================

-- Default workspace
INSERT INTO workspaces (id, name, slug)
VALUES (
  '00000000-0000-0000-0000-000000000001',
  'Default Workspace',
  'default'
)
ON CONFLICT (slug) DO NOTHING;

-- Ownership member record (owner UUID = all-zeros dev user)
INSERT INTO workspace_members (workspace_id, user_id, role)
VALUES (
  '00000000-0000-0000-0000-000000000001',
  '00000000-0000-0000-0000-000000000000',
  'owner'
)
ON CONFLICT (workspace_id, user_id) DO NOTHING;

-- Dev API key
-- Plaintext: agicos_dev_key
-- Hash: bcrypt($2a$ rounds=12) of "agicos_dev_key"
-- NOTE: bcrypt hashes are non-deterministic (random salt).
-- To verify: use supabase Studio or run a Rust test with
--   bcrypt::verify("agicos_dev_key", &hash)
INSERT INTO api_keys (workspace_id, name, key_hash, key_prefix, scope)
VALUES (
  '00000000-0000-0000-0000-000000000001',
  'Dev API Key',
  crypt('agicos_dev_key', gen_salt('bf', 12)),
  'agicos_',
  'write'
)
ON CONFLICT (name) DO NOTHING;

-- Sample agent
INSERT INTO agents (id, workspace_id, name, description, instructions, model, created_by)
VALUES (
  '00000000-0000-0000-0000-000000000010',
  '00000000-0000-0000-0000-000000000001',
  'Assistant Bot',
  'Default development agent for testing',
  'You are a helpful assistant for the AgenticOS platform.',
  'gpt-4o',
  '00000000-0000-0000-0000-000000000000'
)
ON CONFLICT (id) DO NOTHING;
