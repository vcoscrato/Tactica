# AGENTS.md

Guidance for coding agents working in this repository.

## Project Overview

- This is Tactica, a Rust 2024 desktop chess trainer built with `iced`.
- Treat `Cargo.toml` and `Cargo.lock` as the source of truth for dependencies and versions.
- The app uses an Elm-style Model-View-Update architecture:
  - `src/core/` contains chess/domain logic, persistence models, engine integration, PGN parsing, review logic, bundled-data parsing, and mode state.
  - `src/iced/` contains UI rendering, widgets, layout, styling, subscriptions, and message mapping.
  - `src/storage.rs` owns application data paths and atomic file writes.
  - `src/main.rs` only starts `tactica::iced::app::run()`.
- Keep core logic independent of `iced`. UI code may depend on core types, but `src/core/` should not depend on UI widgets, themes, or layout code.

## Setup Notes

- The engine is embedded at compile time with `include_bytes!("../../assets/stockfish")` in `src/core/engine.rs`.
- `assets/stockfish` is intentionally ignored by git because it is a large local binary. If it is missing, run:

  ```bash
  just stockfish
  ```

- `just stockfish` downloads a large binary from the network. Ask before running it when network access or large downloads are not already expected.
- The app stores settings under the platform config directory and data under the platform local data directory via `src/storage.rs`. Do not delete or rewrite user data directories unless explicitly asked.
- Opening names are embedded from `assets/`; user data remains under the platform data directory.
- Every push to `main` triggers `.github/workflows/build.yml`, which tests, builds, packages, and uploads the Linux x86_64 artifact on Ubuntu 22.04.

## Common Commands

Use Cargo directly for routine development checks:

```bash
cargo fmt --check
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo run
```

The minimal `Justfile` is reserved for the project-specific `just stockfish`
asset task.

Use focused commands while iterating when that is faster, for example:

```bash
cargo test --locked pgn
cargo check --locked
```

For Rust changes, run the narrowest relevant test first, then run the standard format, test, and clippy commands above before finishing when practical. For documentation-only changes, a readback or diff inspection is enough unless the docs describe commands or generated output that should be verified.

## Architecture Rules

- Put chess rules, move generation, PGN parsing, review heuristics, engine state, and persistent models in `src/core/`.
- Put screens in `src/iced/pages/` and shared UI pieces in `src/iced/widgets/`.
- New modes should follow the existing `GameMode` trait in `src/iced/pages/mod.rs` and be wired through `Mode`, `ModeMessage`, app update handling, subscriptions, hotkeys, and navigation helpers.
- Reuse `shakmaty` for chess legality, board state, move representation, and FEN/PGN-adjacent behavior. Do not hand-roll chess rules unless the existing libraries cannot cover the case.
- Keep long-running or blocking work out of the UI update path. Use `iced::Task`, subscriptions, async functions, or background threads following existing engine patterns.
- Preserve the existing settings flow: mutate `AppSettings`, call the save/apply helpers, and propagate settings into the active mode.
- Use `Storage`, `write_atomic`, `ensure_parent_dir`, and related helpers for persisted files instead of ad hoc path handling.

## UI Guidelines

- Follow the existing `iced` style helpers in `src/iced/style.rs` and shared widgets before adding new styling patterns.
- Keep board behavior centralized in `src/iced/widgets/board.rs`; avoid duplicating drag/drop, highlighting, animation, promotion, or orientation logic in page code.
- Page modules should translate user actions into mode messages and delegate reusable controls to widgets where possible.
- When adding controls, update mode instructions and active hotkeys if the new behavior is user-facing.

## Testing Expectations

- Add or update tests in the closest core module for changes to parsing, storage, mode state, review classification, bundled opening data, or chess behavior.
- Existing tests live inline under `#[cfg(test)]` in modules such as `src/core/pgn.rs`, `src/core/modes/study.rs`, `src/core/openings.rs`, `src/core/library.rs`, and `src/storage.rs`.
- For UI-only changes, at minimum run `cargo check --locked` when `assets/stockfish` is present. Prefer the full format, test, and clippy checks before larger handoffs.
- If a command cannot be run because `assets/stockfish` is missing, network is unavailable, or the environment lacks GUI support, state that clearly in the final response.

## Dependency and Asset Policy

- Avoid adding heavy dependencies unless the benefit is clear and localized.
- Keep `Cargo.lock` in sync when dependencies change.
- Do not commit local engine binaries, downloaded tablebases, downloaded books, or user study/review data.
- Use the existing SVG assets and review icon pipeline before introducing new asset formats.

## Final Response Checklist

- Summarize the files changed and the behavior affected.
- Report the verification commands that were run, or explain why they were not run.
- Call out any remaining risks, GUI-only behavior, or unverified engine behavior.
