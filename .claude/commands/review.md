---
description: "Review code changes for bugs, security issues, and quality"
---

Use the code-reviewer agent to review the current changes.

Scope: $ARGUMENTS

If no scope is provided, review all uncommitted changes via `git diff`.

Requirements:
1. Check for critical issues (secrets, path traversal, unhandled errors)
2. Check for quality issues (long functions, missing types, magic numbers)
3. Provide a clear verdict: Approve / Warning / Block
