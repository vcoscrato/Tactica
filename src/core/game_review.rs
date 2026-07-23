//! Game review core data model.
//!
//! A lightweight data model for reviewing a completed game:
//! - Stores the game tree and review results
//! - Persistence via PGN + review sidecar JSON
//! - No annotation/branching support (read-focused)

use shakmaty::{Chess, Move};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::board::ChessTree;
use crate::core::config;
use crate::core::pgn;
use crate::core::review::MoveReview;
use crate::storage::{sanitize_filename, write_atomic};

/// A game review session — wraps a PGN game with review data
#[derive(Debug, Clone)]
pub struct GameReview {
    pub name: String,
    pub file_path: Option<PathBuf>,
    pub tree: ChessTree,
    pub review_results: HashMap<usize, MoveReview>,
    pub review_visible: bool,
    pub favorite: bool,
    pub dirty: bool,
}

impl GameReview {
    /// Create a new empty review
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            file_path: None,
            tree: ChessTree::new(),
            review_results: HashMap::new(),
            review_visible: true,
            favorite: false,
            dirty: false,
        }
    }

    /// Create an empty review (for mode picker entry)
    pub fn empty() -> Self {
        Self::new("Untitled Review")
    }

    /// Create a review from a PGN string
    pub fn from_pgn(pgn_str: &str) -> Result<Self, String> {
        let (name, tree) = pgn::parse_pgn(pgn_str)?;
        Ok(Self {
            name,
            file_path: None,
            tree,
            review_results: HashMap::new(),
            review_visible: true,
            favorite: false,
            dirty: true,
        })
    }

    /// Create a review from an existing tree (e.g., from Study "Review this line")
    pub fn from_tree(name: &str, tree: ChessTree) -> Self {
        Self {
            name: name.to_string(),
            file_path: None,
            tree,
            review_results: HashMap::new(),
            review_visible: true,
            favorite: false,
            dirty: true,
        }
    }

    /// Create a review from a sequence of moves
    pub fn from_moves(name: &str, moves: Vec<Move>) -> Self {
        let mut tree = ChessTree::new();
        for mv in moves {
            let _ = tree.play_move(mv);
        }
        tree.go_to_start();

        Self {
            name: name.to_string(),
            file_path: None,
            tree,
            review_results: HashMap::new(),
            review_visible: true,
            favorite: false,
            dirty: true,
        }
    }

    /// Create a review from a starting position
    pub fn from_position(name: &str, position: Chess) -> Self {
        Self {
            name: name.to_string(),
            file_path: None,
            tree: ChessTree::with_position(position),
            review_results: HashMap::new(),
            review_visible: true,
            favorite: false,
            dirty: true,
        }
    }

    /// Get the current position
    pub fn position(&self) -> &Chess {
        self.tree.position()
    }

    /// Go back one move
    pub fn go_back(&mut self) -> bool {
        self.tree.go_back()
    }

    /// Go forward one move
    pub fn go_forward(&mut self) -> bool {
        self.tree.go_forward()
    }

    /// Go to the starting position
    pub fn go_to_start(&mut self) {
        self.tree.go_to_start();
    }

    /// Go to the end of the current line
    pub fn go_to_end(&mut self) {
        self.tree.go_to_end();
    }

    /// Go to a specific path
    pub fn go_to_path(&mut self, path: &[usize]) -> bool {
        self.tree.go_to_path(path)
    }

    /// Get the last move played
    pub fn last_move(&self) -> Option<&Move> {
        self.tree.last_move()
    }

    /// Play a move (for ephemeral exploration)
    pub fn make_move(&mut self, mv: Move) -> Result<usize, String> {
        self.tree.play_move(mv)
    }

    // --- Review ---

    pub fn set_review_results(&mut self, review_results: HashMap<usize, MoveReview>) {
        self.review_results = review_results;
        self.review_visible = true;
        self.dirty = true;
    }

    pub fn clear_review_results(&mut self) {
        if !self.review_results.is_empty() {
            self.review_results.clear();
            self.review_visible = true;
            self.dirty = true;
        }
    }

    pub fn has_review_results(&self) -> bool {
        !self.review_results.is_empty()
    }

    pub fn toggle_review_visibility(&mut self) {
        if self.has_review_results() {
            self.review_visible = !self.review_visible;
            self.dirty = true;
        }
    }

    // --- Persistence ---

    /// Save the review (PGN + sidecar)
    pub fn save(&mut self) -> Result<(), String> {
        // Don't save empty untitled reviews
        if self.file_path.is_none()
            && self.tree.root().children().is_empty()
            && self.tree.root().position() == &Chess::default()
        {
            return Ok(());
        }

        if self.file_path.is_none() {
            let dir = config::reviews_dir();
            let filename = sanitize_filename(&self.name);
            self.file_path = Some(dir.join(format!("{}.pgn", filename)));
        }

        if let Some(ref path) = self.file_path {
            let pgn_content = pgn::to_pgn(&self.name, &self.tree);
            write_atomic(path, pgn_content.as_bytes())
                .map_err(|e| format!("Failed to save review: {e}"))?;
            self.save_review_sidecar(path)?;
            self.dirty = false;
        }
        Ok(())
    }

    /// Load a review from a PGN file (with optional sidecar)
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        let content =
            fs::read_to_string(path).map_err(|e| format!("Failed to read review file: {e}"))?;
        let (name, tree) = pgn::parse_pgn(&content)?;

        let (review_results, review_visible) = load_review_sidecar(path);

        Ok(Self {
            name,
            file_path: Some(path.to_path_buf()),
            tree,
            review_results,
            review_visible,
            favorite: false,
            dirty: false,
        })
    }

    fn save_review_sidecar(&self, pgn_path: &Path) -> Result<(), String> {
        let sidecar_path = review_sidecar_path(pgn_path);

        if self.review_results.is_empty() {
            if sidecar_path.exists() {
                fs::remove_file(&sidecar_path)
                    .map_err(|e| format!("Failed to remove review sidecar: {e}"))?;
            }
            return Ok(());
        }

        let data = ReviewSidecarData {
            review_visible: self.review_visible,
            reviews: self.review_results.values().cloned().collect(),
        };
        let json = serde_json::to_string_pretty(&data)
            .map_err(|e| format!("Failed to serialize review data: {e}"))?;
        write_atomic(&sidecar_path, json.as_bytes())
            .map_err(|e| format!("Failed to write review sidecar: {e}"))
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ReviewSidecarData {
    review_visible: bool,
    reviews: Vec<MoveReview>,
}

fn review_sidecar_path(pgn_path: &Path) -> PathBuf {
    let stem = pgn_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("review");
    let file_name = format!("{}.review.json", stem);
    pgn_path.with_file_name(file_name)
}

fn load_review_sidecar(path: &Path) -> (HashMap<usize, MoveReview>, bool) {
    let sidecar_path = review_sidecar_path(path);
    let Ok(content) = fs::read_to_string(sidecar_path) else {
        return (HashMap::new(), true);
    };
    let Ok(data) = serde_json::from_str::<ReviewSidecarData>(&content) else {
        return (HashMap::new(), true);
    };

    let map = data.reviews.into_iter().map(|r| (r.ply, r)).collect();
    (map, data.review_visible)
}
