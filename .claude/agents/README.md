# PDF Stamper — Subagent Reference

This project uses specialized Claude Code subagents for different tasks. Each agent is a markdown file with YAML frontmatter defining its name, description, tools, and model.

## Agent Inventory

### Core Agents (General Purpose)

| Agent | Model | Tools | Use When |
|-------|-------|-------|----------|
| **planner** | opus | Read-only | Before starting non-trivial work. Creates implementation plans. |
| **coder** | sonnet | Full | Implementing features, bug fixes, refactoring. |
| **code-reviewer** | sonnet | Read-only + Bash | After code changes. Reviews for bugs, security, quality. |
| **build-error-resolver** | sonnet | Full | Any build failure (`cargo`, `tsc`, `tauri build`). |

### Domain-Specific Agents

| Agent | Model | Tools | Use When |
|-------|-------|-------|----------|
| **pdf-debugger** | opus | Read-only + Bash | Stamps render wrong — inspects raw PDF objects, cm matrices, /Rotate. |
| **coordinate-fixer** | opus | Full | Stamp position mismatch between preview and saved PDF. |
| **tauri-ipc-debugger** | sonnet | Read-only + Bash | Frontend↔backend communication failures. |
| **ui-tweaker** | sonnet | Full | React component changes, styling, interactions. |
| **test-writer** | sonnet | Full | Adding Rust (`cargo test`) or frontend (`vitest`) tests. |
| **rust-build-resolver** | sonnet | Full | Rust-specific build errors (lopdf, pdfium-render, Tauri macros). |
| **release-builder** | sonnet | Full | Production builds, CI/CD, platform bundling. |

## When to Use Which Agent

```
Bug: "stamp is in wrong position"
  → coordinate-fixer (if coordinate math issue)
  → pdf-debugger (if PDF structure issue)

Bug: "build fails"
  → build-error-resolver (general)
  → rust-build-resolver (Rust-specific, knows lopdf API)

Task: "add a new feature"
  → planner (first — create the plan)
  → coder (then — implement it)
  → code-reviewer (finally — review changes)

Bug: "invoke returns error"
  → tauri-ipc-debugger

Task: "change the UI"
  → ui-tweaker

Task: "add tests"
  → test-writer
```

## Agent Design Principles

1. **Single responsibility** — each agent does one thing well
2. **Least privilege** — read-only agents can't accidentally break code
3. **Model selection** — opus for complex reasoning (planning, PDF debugging), sonnet for implementation
4. **Project context** — every agent knows this project's tech stack and patterns
5. **Focused workflow** — each agent has a clear step-by-step process
