#!/usr/bin/env bash
# Ralph autonomous coding loop for pdf-stamper.
# Pipes prompt.md into `claude --dangerously-skip-permissions` repeatedly,
# fresh context each iteration, until either all stories in prd.json show
# `passes: true` (Claude prints `<promise>COMPLETE</promise>`) or the
# iteration cap is hit.
#
# Usage:  ./scripts/ralph/ralph.sh [MAX_ITERATIONS]
#         ./scripts/ralph/ralph.sh 5    # try 5 iterations max
#
# Memory persists across iterations via:
#   - git history (Ralph commits each story)
#   - prd.json (passes flags)
#   - progress.txt (Codebase Patterns + per-story learnings)
#
# Run inside the devcontainer (.devcontainer/), NOT on host. Verify the
# three feedback loops before launching:
#   cd src-tauri && cargo test
#   npx tsc --noEmit
#   npx vitest run

set -e

MAX_ITERATIONS=${1:-10}
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

cd "$REPO_ROOT"

echo "Starting Ralph"
echo "  repo:   $REPO_ROOT"
echo "  branch: $(git rev-parse --abbrev-ref HEAD)"
echo "  max iterations: $MAX_ITERATIONS"
echo

for i in $(seq 1 "$MAX_ITERATIONS"); do
  echo "================ iteration $i / $MAX_ITERATIONS ================"

  OUTPUT=$(cat "$SCRIPT_DIR/prompt.md" \
    | claude --dangerously-skip-permissions -p 2>&1 \
    | tee /dev/stderr) || true

  if echo "$OUTPUT" | grep -q "<promise>COMPLETE</promise>"; then
    echo
    echo "All stories complete. Final state:"
    jq '.userStories[] | {id, passes, title}' "$SCRIPT_DIR/prd.json"
    exit 0
  fi

  sleep 2
done

echo
echo "Max iterations reached. Story status:"
jq '.userStories[] | {id, passes, title}' "$SCRIPT_DIR/prd.json"
exit 1
