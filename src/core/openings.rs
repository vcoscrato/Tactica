//! Openings module
//!
//! Handles opening names lookup for trivia and display.

use shakmaty::{Chess, Move, Position};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use std::str::FromStr;

// --- Opening Names ---

// Snapshot of lichess-org/chess-openings at 17ee660257de02870636f36248e919f2e01d8e85.
const BUNDLED_OPENINGS: &str = include_str!("../../assets/openings.tsv");

#[derive(Debug, Clone)]
pub struct Opening {
    pub eco: String,
    pub name: String,
    pub moves: Vec<Move>,
}

#[derive(Debug, Clone)]
pub struct OpeningNames {
    // Map from Zobrist hash of position to opening name
    names: HashMap<u64, String>,
    // Map of all known opening-prefix positions to an opening name
    book_positions: HashMap<u64, String>,
    // List of all openings for trivia
    openings: Vec<Opening>,
}

impl Default for OpeningNames {
    fn default() -> Self {
        Self::new()
    }
}

impl OpeningNames {
    pub fn new() -> Self {
        Self::from_tsv(BUNDLED_OPENINGS)
    }

    pub fn from_path(path: &Path) -> Self {
        let Ok(file) = File::open(path) else {
            return Self::from_tsv("");
        };
        Self::from_lines(io::BufReader::new(file).lines().map_while(Result::ok))
    }

    fn from_tsv(tsv: &str) -> Self {
        Self::from_lines(tsv.lines().map(str::to_owned))
    }

    fn from_lines(lines: impl IntoIterator<Item = String>) -> Self {
        let mut names = HashMap::new();
        let mut book_positions = HashMap::new();
        let mut openings = Vec::new();

        for line in lines {
            if let Some((eco, name, moves_str)) = parse_line_full(&line)
                && let Some((hash, moves)) = parse_pgn_full(moves_str)
            {
                names.insert(hash, name.clone());
                index_opening_prefixes(&mut book_positions, &moves, &name);
                openings.push(Opening { eco, name, moves });
            }
        }

        Self {
            names,
            book_positions,
            openings,
        }
    }

    pub fn lookup(&self, position: &Chess) -> Option<&String> {
        let hash = position_hash(position);
        self.names.get(&hash)
    }

    pub fn lookup_book(&self, position: &Chess) -> Option<&String> {
        let hash = position_hash(position);
        self.book_positions.get(&hash)
    }

    pub fn is_book_position(&self, position: &Chess) -> bool {
        let hash = position_hash(position);
        self.book_positions.contains_key(&hash)
    }

    pub fn get_random_opening(&self) -> Option<&Opening> {
        if self.openings.is_empty() {
            return None;
        }

        let index = fastrand::usize(..self.openings.len());
        self.openings.get(index)
    }

    /// Get a random opening with at least `min_moves` moves (for Chessle)
    pub fn get_random_opening_min_moves(&self, min_moves: usize) -> Option<&Opening> {
        let valid: Vec<_> = self
            .openings
            .iter()
            .filter(|o| o.moves.len() >= min_moves)
            .collect();

        if valid.is_empty() {
            return None;
        }

        let index = fastrand::usize(..valid.len());
        valid.get(index).copied()
    }

    pub fn is_loaded(&self) -> bool {
        !self.openings.is_empty()
    }
}

fn parse_line_full(line: &str) -> Option<(String, String, &str)> {
    // Format: ECO \t Name \t PGN
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() >= 3 {
        let eco = parts[0].to_string();
        let name = parts[1].to_string();
        let pgn = parts[2];
        Some((eco, name, pgn))
    } else {
        None
    }
}

fn parse_pgn_full(pgn: &str) -> Option<(u64, Vec<Move>)> {
    use shakmaty::san::San;

    let mut pos = Chess::default();
    let mut moves = Vec::new();

    for token in pgn.split_whitespace() {
        if token.ends_with('.') {
            continue;
        }

        // Parse SAN
        if let Ok(san) = San::from_str(token) {
            if let Ok(m) = san.to_move(&pos) {
                pos = pos.play(m).ok()?;
                moves.push(m);
            } else {
                return None;
            }
        } else {
            return None;
        }
    }

    Some((position_hash(&pos), moves))
}

fn position_hash(pos: &Chess) -> u64 {
    use shakmaty::EnPassantMode;
    use shakmaty::zobrist::Zobrist64;
    let hash: Zobrist64 = pos.zobrist_hash(EnPassantMode::Legal);
    hash.0
}

fn index_opening_prefixes(book_positions: &mut HashMap<u64, String>, moves: &[Move], name: &str) {
    let mut pos = Chess::default();
    for mv in moves {
        if let Ok(next) = pos.clone().play(*mv) {
            pos = next;
            let hash = position_hash(&pos);
            book_positions
                .entry(hash)
                .or_insert_with(|| name.to_string());
        } else {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_openings_are_available_without_runtime_assets() {
        let openings = OpeningNames::new();
        assert!(openings.is_loaded());
        assert!(openings.openings.len() > 3_000);
    }
}
