# Third-Party Notices

Tactica is licensed under `GPL-3.0-or-later`; see `LICENSE`. The table below
identifies material bundled into the application. Required visual-asset license
texts are in [ASSET_LICENSES.md](ASSET_LICENSES.md). Binary releases also
include the generated `RUST_DEPENDENCY_LICENSES.html` for linked Rust crates.

| Component | Included material | License | Source |
|---|---|---|---|
| Cburnett chess pieces | `assets/pieces/*.svg`, plus the knight artwork in `assets/tactica.svg` and `assets/tactica.png` | BSD-3-Clause | [Wikimedia Commons](https://commons.wikimedia.org/wiki/Category:SVG_chess_pieces) |
| Lucide Icons | `arrow-left.svg`, `branch.svg`, `close.svg`, `edit.svg`, `help.svg`, `menu.svg`, `moon.svg`, `save.svg`, `settings.svg`, `sun.svg`, and `trash.svg` under `assets/icons/` | ISC; some designs also MIT via Feather Icons | [Lucide](https://lucide.dev/) |
| Heroicons | `assets/icons/star-outline.svg`, `assets/icons/star-solid.svg` | MIT | [Heroicons](https://heroicons.com/) |
| Tabler Icons | `assets/review_icons/*.svg` | MIT | [Tabler Icons](https://github.com/tabler/tabler-icons) |
| Lichess chess openings | `assets/openings.tsv` | CC0-1.0 | [Pinned source revision](https://github.com/lichess-org/chess-openings/tree/17ee660257de02870636f36248e919f2e01d8e85) |
| Stockfish 18 | Unmodified `stockfish-ubuntu-x86-64`, embedded and run as a separate process | GPL-3.0-only | [Release and corresponding source](https://github.com/official-stockfish/Stockfish/releases/tag/sf_18) |

The Stockfish archive used by `download-stockfish.sh` has SHA-256
`5c6f38b02a4da5f3ffe763f27da6c3e743eebefd92b50cb3661623b96696adff`.
Each Tactica binary release links to its exact source commit in the release
notes; `Cargo.lock` identifies the corresponding Rust dependency versions.
