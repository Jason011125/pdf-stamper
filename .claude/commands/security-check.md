---
description: "Security-focused review of code changes or specific files"
---

Use the code-reviewer agent to perform a security-focused review.

Scope: $ARGUMENTS

Requirements:
1. Focus exclusively on security concerns
2. Check for: hardcoded secrets, path traversal, unsafe file handling, XSS, injection
3. Verify user inputs are validated at system boundaries
4. Check that file I/O errors are handled (no silent failures)
5. Verdict: Safe / Needs attention / Blocking security issue
