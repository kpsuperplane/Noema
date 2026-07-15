# Noema Personal Agent

## Resources 
- Project context: `docs/project.md`
- Current project brief: `docs/context/current.md`

## Standards
- Work on main branch unless explicitly instructed
- Clean code and good architecture are of utmost priority. When implementing features or fixing bugs, take a bigger-picture look at the surrounding system before editing, and aggressively look for ways to deliver the change with the least new net code.
- Unless specifically asked for a narrow fix, always fix the general failure mode or deeper architectural issue behind a bug. Do not special-case the exact prompt, example, input, or tool call that exposed it.
- Consolidate existing logic whenever reasonable. Prefer improving or reusing the right abstraction over adding parallel paths, duplicate helpers, or narrowly scoped patches that leave the system more fragmented.
- The project is under active development, do not build backwards compatibility unless explicitly instructed
- Early stage active development, schema changes may rewrite tables/docs directly. Do not add migrations or compatibility layers unless explicitly requested.
- Try to keep code source files under 750 lines. It is not a hard rule, however any file exceeding that threshold should be inspected for refactor, split up, and cleanup opportunities
- Do not use direct text, prefix, or English phrase matching as the authority for semantic user intent. It is brittle and fails for multilingual users. Prefer explicit product state, structured model/tool interpretation with policy checks, or language-aware parsers/tests.

## Codex Workflow
- Start by checking `git status --short --branch`.
- Preserve unrelated dirty worktree changes.
- For architecture, memory, harness, frontend IA, or workflow work, read `docs/project.md`, `docs/context/current.md`, and the closest relevant docs first.
- For nontrivial work, make the task mode explicit before proceeding: explore only, plan only, implement, adversarial review, or ship.
- Split long work at milestone boundaries. After a major commit or completed phase, summarize durable context into `docs/context/current.md` before continuing.
- Make a commit after finishing each unit of work unless explicitly instructed not to.
- Treat raw `~/.codex/sessions` files as private source material. Read them only when asked, summarize durable decisions, and do not quote raw transcript unless explicitly requested.

## Review And Subagents
- Keep narrow changes inline when delegation overhead would exceed the implementation work.
- For implementation work, assign disjoint ownership by large areas (eg. frontend implementation, rust changes).
- For adversarial review, reviewers should inspect and report findings without editing files.
- The main agent owns final integration, validation, and the user-facing summary.

## Validation
- Never circumvent, disable, bypass, unset, or otherwise interfere with the `sccache` build cache.
- Never modify `CARGO_BUILD_RUSTC_WRAPPER` or attempt to work around the configured Rust compiler wrapper. Doing so invalidates shared cache state, causes 15min+ builds, and can break other agents building in parallel.
- Default Rust validation:
  - `cargo fmt --all --check`
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace --no-fail-fast`
- Run unit tests only. Do not run smoke tests or fixture tests unless explicitly requested.
- Do not write tests for UI/frontend work unless explicitly requested.
- For frontend or UI work, do not inspect with browser tools unless explicitly requested.

## Ship Checklist
- Before committing or pushing, run:
  - `git status --short --branch`
  - `git diff --check`
  - the relevant validation commands above
- Inspect staged changes with `git diff --cached --stat` and `git diff --cached --name-status`.
- Report remaining untracked or unstaged files.
- Commit after each finished unit of work. Push only when explicitly requested.
