---
description: "Fix build or type errors with minimal changes"
---

Use the build-error-resolver agent to fix the current build errors.

Context: $ARGUMENTS

Requirements:
1. Run the failing build command and capture all errors
2. Fix errors one at a time with minimal diffs
3. Do not refactor or change architecture
4. Verify the build passes before finishing
