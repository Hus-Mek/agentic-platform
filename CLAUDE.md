# Claude Code in agentic-platform

@AGENTS.md

AGENTS.md, imported above, holds the rules for every agent. This file adds what Claude Code needs on top.

## Done means it works for the user

Green CI doesn't make a feature, a fix, or any code change done. Before you call it done, use it
the way its user will meet it, and read what they would see:

- Start the stack from `infra/docker-compose.yml` and the frontend with `npm run dev` (port 3001). Open
  it in a browser (use the Claude in Chrome extension) and use the flow you changed.
- Read what the screen shows and what the API returns, not only that `cargo check --workspace` passes.
- Put what you ran and what the user saw in the pull request body. If you couldn't try a change
  this way, say so and why.
