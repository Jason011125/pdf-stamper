# Ralph Iteration — pdf-stamper

You are running ONE iteration of an autonomous coding loop. Your output goes back into a fresh-context iteration; you cannot rely on memory between iterations except via files (prd.json, progress.txt, git history).

## Environment

- Linux devcontainer (Debian bookworm). Repo is your cwd. You are user `node`.
- Available: node 20, npm, Rust stable (cargo, rustc, clippy, rustfmt), pdfium Linux .so installed at `src-tauri/libs/pdfium/lib/libpdfium.so`.
- **NOT available**: Tauri GUI (no webkit2gtk). Do NOT run `npm run tauri dev` or `tauri build` — they will fail. UI smoke testing happens later on the macOS host, not here.

## Steps for this iteration

1. **Read state**:
   - `cat scripts/ralph/prd.json`
   - `cat scripts/ralph/progress.txt` — read the "Codebase Patterns" section *first*; it encodes hard-won project knowledge.
   - `cat .claude/CLAUDE.md` if you haven't internalized it yet (covers tech stack, the cm matrix recipe, q/Q wrapping rule, coordinate spaces).

2. **Confirm branch**:
   - `git rev-parse --abbrev-ref HEAD`
   - If you are on `main`, switch: `git checkout -B "$(jq -r .branchName scripts/ralph/prd.json)"`. Never edit on `main`.

3. **Pick the highest-priority unfinished story** (lowest `priority` number where `passes: false`). If all stories pass, output exactly `<promise>COMPLETE</promise>` and stop.

4. **Implement that ONE story.** Read the acceptance criteria. Edit only the files needed. Do not bundle multiple stories.

5. **Run the relevant feedback loops** before committing:
   - Touched Rust (`src-tauri/`)? Run `cd src-tauri && cargo test`. Must pass.
   - Touched TypeScript? Run `npx tsc --noEmit`. Must pass.
   - Touched React/Zustand/components? Also run `npx vitest run`. Must pass.
   If a loop fails, fix the failure before committing. Never commit broken code. If you cannot fix it within this iteration, leave the story `passes: false`, append a learnings note about what blocked you, and stop — the next iteration's fresh context can take another swing.

6. **Update `.claude/CLAUDE.md`** *only* if you discovered a reusable pattern worth preserving (gotcha, convention, dependency). Don't add story-specific noise.

7. **Commit**: `git add -A && git commit -m "<type>(<scope>): [<US-ID>] <title>"`. Match the repo's existing style — see `git log --oneline -10`. For UI stories (US-002c, US-R2, US-P2), append a `Manual verify:` line in the commit body with the recipe (e.g., "load 3 PDFs, drag stamp on #1, expect …").

8. **Mark the story done**:
   - Edit `scripts/ralph/prd.json`: set the story's `passes: true`. Add a one-line `notes` if there's something the next story should know (cross-story dependency, leftover TODO, etc.).
   - Append to `scripts/ralph/progress.txt`:
     ```
     ## YYYY-MM-DD — <US-ID>
     - What you implemented
     - Files changed
     - **Learnings**: pattern discovered, gotcha, etc.
     ---
     ```

9. **Commit the bookkeeping** as a follow-up commit (`chore(ralph): mark <US-ID> done`) or amend onto the implementation commit. Either way is fine; keep history clean.

10. Stop. The bash wrapper starts the next iteration with fresh context.

## Hard rules

- **Never `git push`**. Never delete or force-update `main`.
- **Never run `tauri dev`/`tauri build`** in this container — no webkit, will fail and waste minutes.
- **One story per iteration.** If you finish early, stop — do not speculatively pick a second story.
- **No mocking the database / external services** for tests in this repo. The Rust tests use real `lopdf` parsing and pdfium rendering; that is the whole point of the property-based test pattern (see CLAUDE.md "Why pure-arithmetic unit tests aren't enough").
- **Match existing conventions** (CLAUDE.md "Conventions" section). Rust = snake_case, TS = camelCase / PascalCase components, files = kebab-case, no `console.log` in production code.

## Stop condition (re-stated)

If after step 1 every story already has `passes: true`, output exactly:

<promise>COMPLETE</promise>

Then stop.
