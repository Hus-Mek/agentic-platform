#!/bin/bash
cd /c/Users/USER/agentic-platform
PROMPT=$(cat .opencode_prompt_graphify_chunk01.txt)
opencode run --format json --timeout 600 "$PROMPT" 2>&1 | tee graphify-out/opencode_chunk01.log
