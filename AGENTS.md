# AGENTS.md — Operating Rules for AgenticOS Platform

## MANDATORY: Agent Skills (addyosmani/agent-skills)

Before ANY coding task, determine which skill(s) from `.agent-skills/skills/` apply. If a skill matches, load it and follow it.

### All 24 Skills by Phase

**DEFINE:**
- `interview-me` — One-question-at-a-time interview to extract what user wants
- `idea-refine` — Divergent/convergent thinking to turn vague concepts into proposals
- `spec-driven-development` — Write PRD before code

**PLAN:**
- `planning-and-task-breakdown` — Decompose specs into verifiable tasks with ACs

**BUILD:**
- `incremental-implementation` — Thin vertical slices, test-verify-commit
- `test-driven-development` — Red-Green-Refactor, test pyramid
- `context-engineering` — Feed agents right info at right time
- `source-driven-development` — Ground decisions in official docs
- `doubt-driven-development` — Adversarial fresh-context review of decisions
- `frontend-ui-engineering` — Components, design systems, a11y
- `api-and-interface-design` — Contract-first, Hyrum's Law

**VERIFY:**
- `browser-testing-with-devtools` — DOM inspection, console, network
- `debugging-and-error-recovery` — Five-step triage

**REVIEW:**
- `code-review-and-quality` — Five-axis review, change sizing
- `code-simplification` — Chesterton's Fence, Rule of 500
- `security-and-hardening` — OWASP Top 10, auth, secrets
- `performance-optimization` — Measure-first, Core Web Vitals

**SHIP:**
- `git-workflow-and-versioning` — Trunk-based, atomic commits
- `ci-cd-and-automation` — Shift Left, feature flags
- `deprecation-and-migration` — Code-as-liability
- `documentation-and-adrs` — ADRs, document the why
- `observability-and-instrumentation` — RED metrics, tracing
- `shipping-and-launch` — Pre-launch checklists, staged rollouts

**META:**
- `using-agent-skills` — Maps incoming work to correct skill

## Execution Rules

1. **All coding via subagents.** Use the `task` tool with `subagent_type: "general"` for every code change. Never write code directly.
2. **Skill-first.** If a task matches a skill (even 1% chance), load the skill's SKILL.md first via the `skill` tool.
3. **Follow the skill exactly.** Do not skip steps, do not partially apply.
4. **Verification is non-negotiable.** Every change must compile/validate before marking done.

## Project Context

- Rust workspace at `backend/` (5 crates: core, agent-runtime, api-server, guards, memory)
- TypeScript frontend at `frontend/`
- Infra at `infra/` (docker-compose.yml, migrations)
- Skills cloned at `.agent-skills/`
- Workspace compiles with `cargo check --workspace` from `backend/`
