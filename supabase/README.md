# Supabase Local Development — AgenticOS Platform

This directory configures [Supabase](https://supabase.com) for local development. Supabase provides a PostgREST API, Studio dashboard, and the `auth.uid()` function used by the project's Row-Level Security policies.

## Prerequisites

### Install Supabase CLI

```bash
# Windows (winget)
winget install supabase.cli

# Windows (scoop)
scoop bucket add supabase https://github.com/supabase/scoop-bucket.git
scoop install supabase

# macOS
brew install supabase/tap/supabase

# Linux (npm)
npm install -g supabase

# Verify
supabase --version
```

## Quick Start

### 1. Start Supabase stack

```bash
# From the project root (where supabase/config.toml lives)
supabase start
```

This launches:
- **PostgREST API** → http://localhost:54321
- **Supabase Studio** → http://localhost:54323
- **PostgreSQL** → localhost:54322 (managed by Supabase)
- **Kong API Gateway** → http://localhost:54321

> **Note:** Studio uses port 54323 (not 3000) because Langfuse occupies port 3000 in the project's docker-compose.

### 2. Apply migrations

Migrations in `supabase/migrations/` are applied automatically on `supabase start`. To re-apply:

```bash
supabase db reset
```

This drops the Supabase-managed database and re-runs all migrations + seed data.

### 3. Access Supabase Studio

Open http://localhost:54323 in your browser.

## Architecture: Supabase + Project PostgreSQL

The project uses **two PostgreSQL instances**:

| Database | Port | Purpose |
|----------|------|---------|
| **Supabase-managed DB** | 54322 | Supabase internal tables (auth, storage), Supabase migrations, Studio queries |
| **Project main DB** (`docker-compose`) | 5432 | Application data used by the Rust API server, Langfuse, etc. |

The project's **custom JWT auth** (Axum middleware in `backend/api-server/src/middleware/auth.rs`) operates on the main DB (port 5432). Supabase Auth is only used to provide the `auth.uid()` plumbing for RLS policies.

### Using only one PostgreSQL (optional)

If you want Supabase to **manage the project's database** instead of docker-compose:

1. Stop the `postgres` service in docker-compose
2. Set the project's `DATABASE_URL` to point to Supabase's PostgreSQL:
   ```
   DATABASE_URL=postgres://postgres:postgres@localhost:54322/agenticos
   ```
3. Apply SQLx migrations separately or re-import them into `supabase/migrations/`

## Migration Workflow

### Creating a new migration

```bash
supabase migration new <description>
# Creates: supabase/migrations/<timestamp>_<description>.sql
```

### Applying migrations

```bash
# Apply pending migrations
supabase db push

# Full reset (drop + re-migrate + seed)
supabase db reset
```

### Syncing with infra/supabase/migrations/

The canonical schema lives at `infra/supabase/migrations/` (used by docker-compose's initdb). The files in `supabase/migrations/` are copies for Supabase CLI management. When schema changes:

1. Update `infra/supabase/migrations/` first
2. Copy/port the changes to a new migration in `supabase/migrations/`

## Seed Data

`seed.sql` creates a development workspace, API key, and sample agent:

```sql
-- Dev API key plaintext: agicos_dev_key
-- Default workspace slug: default
```

## Useful Commands

```bash
supabase start          # Start all Supabase services
supabase stop           # Stop all Supabase services
supabase status         # Check service status
supabase db reset       # Drop + re-migrate + seed
supabase db push        # Apply pending migrations only
supabase migration list # Show migration status
supabase projects list  # List linked Supabase projects (production)
```

## Production

For production, use the Supabase cloud platform ([supabase.com](https://supabase.com)) or self-host the Supabase stack. Migrations are applied via `supabase db push` against the production database URL.
