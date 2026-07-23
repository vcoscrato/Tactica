# Tactica

A desktop chess training application built with Rust and [iced](https://github.com/iced-rs/iced).

## Features

- **Analysis & Study**: Create and review opening studies with branching variations, notes, and engine evaluation.
- **Game Review**: Analyze your games with move quality classification (Brilliant, Best, Blunder, etc.) powered by Stockfish.
- **Trivia Mode**: Test your opening knowledge with interactive guessing based on your repertoire.
- **Chessle**: Wordle-style opening guessing game.

## User Guide

### Controls

**Global Hotkeys**

| Shortcut | Action |
|---|---|
| `Ctrl+S` | Save current Study/Review |
| `Ctrl+N` | New game/session (mode-dependent) |
| `Ctrl+I` | Import PGN |
| `Ctrl+B` | Toggle sidebar |
| `Home` | Go to start |
| `End` | Go to end |
| `F` | Flip board |
| `Esc` | Close panel |

**Mode-Specific Controls**

*   **Quick Board / Study / Game Review**: Use `Left`/`Right` arrow keys to navigate moves.
*   **Trivia**: `H` (Hint), `S` (Skip), `N` (Next).
*   **Chessle**: `Enter` to submit, `Left` to undo, `A` for debug autofill.

### Game Review

The Game Review mode analyzes your game to identify key moments:
- **Brilliant (!!) / Great (!)**: High-quality moves that find unique advantages or sacrifices.
- **Best / Excellent / Good**: Solid moves maintaining the position.
- **Inaccuracy / Mistake / Blunder**: Moves that lose advantage or material.

Run a review by clicking "Run Review" in the Game Review header.

## For Contributors

### Architecture

This project follows an Elm-inspired Model-View-Update (MVU) architecture with a strict separation of concerns:

-   **`src/core/`**: Pure Rust domain logic and state management. No UI dependencies.
-   **`src/iced/`**: UI implementation using the `iced` library. Handles rendering and user input, mapping them to core updates.

### Project Structure

*   `src/core/`: Application state, rules, parsing, and analysis logic.
*   `src/iced/`: Widgets, pages, layout, and styling.
*   `assets/`: Icons, chess pieces, and opening names.
*   `legal/`: Third-party notices and license-generation configuration.

### Tech Stack

*   **Language**: Rust (2024 edition)
*   **GUI**: [iced](https://github.com/iced-rs/iced) (0.14)
*   **Chess Rules**: [shakmaty](https://github.com/niklasf/shakmaty)
*   **Engine Protocol**: [vampirc-uci](https://github.com/vampirc/vampirc-uci)
*   **PGN Parsing**: pgn-reader

### Development Setup

**Prerequisites:**
1.  **Rust**: Latest stable or 2024 edition compatible.
2.  **Stockfish**: The application requires the Stockfish chess engine binary to be present at build time.
    *   Run `just stockfish` to download and verify the supported Stockfish build.
    *   The resulting `assets/stockfish` file is ignored by git.

**Commands:**

```bash
# Run the application
cargo run

# Run tests
cargo test --locked

# Check for errors
cargo check --locked

# Build for release
cargo build --release --locked
```

### Linux Distribution

Every push to `main` produces a `tactica-linux-x86_64` artifact. Download and
extract it from the latest successful
[`main` build](https://github.com/vcoscrato/Tactica/actions/workflows/build.yml?query=branch%3Amain),
then install it:

```bash
sha256sum --check tactica-linux-x86_64.sha256
install -Dm755 tactica-linux-x86_64 ~/.local/bin/tactica
install -Dm644 tactica.desktop ~/.local/share/applications/tactica.desktop
install -Dm644 tactica.svg ~/.local/share/icons/hicolor/scalable/apps/tactica.svg
```

The workflow tests and builds directly on Ubuntu 22.04, then packages
the binary, checksum, desktop entry, application icon, project license,
third-party notices, and generated Rust dependency licenses. Each artifact is
attached to the workflow run for its exact `main` commit. The version shown in
Settings comes from `Cargo.toml`.


**Contribution Guidelines:**
*   **Architecture**: Keep logic in `src/core` and UI in `src/iced`.
*   **State**: The `src/core` state is the source of truth.
*   **Dependencies**: Avoid adding heavy dependencies unless necessary.

## License

Tactica is licensed under the
[GNU General Public License v3.0 or later](LICENSE). Bundled third-party
components retain their upstream licenses; see
[legal/THIRD_PARTY_NOTICES.md](legal/THIRD_PARTY_NOTICES.md).

Copyright © 2026 Victor Coscrato.
