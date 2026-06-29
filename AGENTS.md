# Noema Personal Agent

## Resources 
- Project context: `docs/project.md`
- Current project brief: `docs/context/current.md`

## Standards
- Work on main branch unless explicitly instructed
- The project is under active development, do not build backwards compatibility unless explicitly instructed
- Pre-V1 schema changes may rewrite tables/docs directly. Do not add migrations or compatibility layers unless explicitly requested.
- Try to keep code source files under 750 lines. It is not a hard rule, however any file exceeding that threshold should be inspected for refactor, split up, and cleanup opportunities
- Do not use direct text, prefix, or English phrase matching as the authority for semantic user intent. It is brittle and fails for multilingual users. Prefer explicit product state, structured model/tool interpretation with policy checks, or language-aware parsers/tests.

## Codex Workflow
- Start by checking `git status --short --branch`.
- Preserve unrelated dirty worktree changes.
- For architecture, memory, harness, frontend IA, or workflow work, read `docs/project.md`, `docs/context/current.md`, and the closest relevant docs first.
- For nontrivial work, make the task mode explicit before proceeding: explore only, plan only, implement, adversarial review, or ship.
- Split long work at milestone boundaries. After a major commit or completed phase, summarize durable context into `docs/context/current.md` before continuing.
- Make a commit after finishing each unit of work unless explicitly instructed not to.
- Prefer small scoped changes. Avoid unrelated refactors unless they are needed to finish safely.
- Treat raw `~/.codex/sessions` files as private source material. Read them only when asked, summarize durable decisions, and do not quote raw transcript unless explicitly requested.

## Review And Subagents
- Use subagents only for distinct, well-scoped work.
- Default coding subagents to `5.5-medium` and all other subagents to `5.5-high`.
- For implementation work, assign disjoint ownership by file/module area.
- For adversarial review, reviewers should inspect and report findings without editing files.
- The main agent owns final integration, validation, and the user-facing summary.

## Validation
- Default Rust validation:
  - `cargo fmt --all --check`
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace --no-fail-fast`
- Run unit tests only. Do not run smoke tests or fixture tests unless explicitly requested.
- Noema daemon/OpenAI provider tests may bind Unix/TCP sockets. If sandboxed tests fail with local socket `PermissionDenied`, rerun the same test command with socket permissions and report that distinction.
- For frontend or UI work, do not inspect with browser tools unless explicitly requested.

## Ship Checklist
- Before committing or pushing, run:
  - `git status --short --branch`
  - `git diff --check`
  - the relevant validation commands above
- Inspect staged changes with `git diff --cached --stat` and `git diff --cached --name-status`.
- Report remaining untracked or unstaged files.
- Commit after each finished unit of work. Push only when explicitly requested.
