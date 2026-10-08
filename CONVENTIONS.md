# Conventions

temor follows quire's `CONVENTIONS.md` and `CONSUMING.md` (the four coherence rules) without
local exceptions. The plan is `PLAN.md`; decisions there win over anything older.

Additions for temor:
- Only `temor-vt` names libghostty; only `temor-desktop` names zbus or docket (`PLAN.md` §2).
- `cargo check --workspace --no-default-features` must pass (portable core, design/36).
- Build output goes to `/mnt/data/cargo/<lane>-temor`, never under `~`.
- Commits: author and committer `Po Hsuan Lai <pohsuanlai0208@gmail.com>` through `GIT_*` env vars,
  with no session, co-author or URL trailers.
- Tests never touch the real system: private bus, scratch `HOME`, no real `~/.config`.
