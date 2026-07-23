//! Study mode core logic.
//!
//! Handles the data model for chess studies, including:
//! - Move tree management with branching
//! - Position navigation
//! - Annotations (titles and notes)
//! - PGN file persistence

use shakmaty::{Chess, Move};
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::board::{ChessTree, NodeAnnotation, PositionNode};
use crate::core::config;
use crate::core::pgn;
use crate::storage::{sanitize_filename, write_atomic};

/// A study session with a move tree, annotations, and branches
#[derive(Debug, Clone)]
pub struct Study {
    pub name: String,
    pub file_path: Option<PathBuf>,
    pub tree: ChessTree,
    pub dirty: bool,
}

impl Default for Study {
    fn default() -> Self {
        Self {
            name: "Untitled".to_string(),
            file_path: None,
            tree: ChessTree::new(),
            dirty: false,
        }
    }
}

impl Study {
    /// Create a new study with the given name
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            file_path: None,
            tree: ChessTree::new(),
            dirty: false,
        }
    }

    /// Create a study from an existing tree.
    pub fn from_tree(name: &str, tree: ChessTree) -> Self {
        Self {
            name: name.to_string(),
            file_path: None,
            tree,
            dirty: true,
        }
    }

    /// Create a study from a starting position (e.g., from FEN)
    pub fn from_position(name: &str, position: Chess) -> Self {
        Self {
            name: name.to_string(),
            file_path: None,
            tree: ChessTree::with_position(position),
            dirty: true,
        }
    }

    /// Create a study from a sequence of moves (e.g., from an opening)
    pub fn from_moves(name: &str, moves: Vec<Move>) -> Self {
        let mut tree = ChessTree::new();
        for mv in moves {
            let _ = tree.play_move(mv);
        }
        // Go back to start for viewing
        tree.go_to_start();

        Self {
            name: name.to_string(),
            file_path: None,
            tree,
            dirty: true,
        }
    }

    /// Create a study from a PGN string
    pub fn from_pgn(pgn_str: &str) -> Result<Self, String> {
        let (name, tree) = pgn::parse_pgn(pgn_str)?;
        Ok(Self {
            name,
            file_path: None,
            tree,
            dirty: true,
        })
    }

    /// Get the current position
    pub fn position(&self) -> &Chess {
        self.tree.position()
    }

    /// Get the current note
    pub fn current_note(&self) -> &str {
        &self.tree.annotation().note
    }

    /// Set the current note
    pub fn set_current_note(&mut self, note: String) {
        if self.tree.annotation().note != note {
            self.tree.set_note(note);
            self.dirty = true;
        }
    }

    /// Get the current note title
    pub fn current_note_title(&self) -> &str {
        &self.tree.annotation().title
    }

    /// Set the current note title
    pub fn set_current_note_title(&mut self, title: String) {
        if self.tree.annotation().title != title {
            self.tree.set_note_title(title);
            self.dirty = true;
        }
    }

    /// Play a move
    pub fn make_move(&mut self, mv: Move) -> Result<usize, String> {
        let idx = self.tree.play_move(mv)?;
        self.dirty = true;
        Ok(idx)
    }

    /// Go back one move
    pub fn go_back(&mut self) -> bool {
        self.tree.go_back()
    }

    /// Go forward one move (follow first variation)
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

    /// Go to a specific path in the tree
    pub fn go_to_path(&mut self, path: &[usize]) -> bool {
        self.tree.go_to_path(path)
    }

    /// Get the last move played
    pub fn last_move(&self) -> Option<&Move> {
        self.tree.last_move()
    }

    /// Get the current node
    pub fn current_node(&self) -> &PositionNode {
        let mut node = self.tree.root();
        for &idx in self.tree.current_path() {
            if idx < node.children().len() {
                node = &node.children()[idx];
            }
        }
        node
    }

    // --- File I/O ---

    /// Save the study to its file path
    pub fn save(&mut self) -> Result<(), String> {
        // Don't save empty "Untitled" studies
        if self.file_path.is_none()
            && self.tree.root().children().is_empty()
            && self.tree.root().annotation.is_empty()
            && self.tree.root().position() == &Chess::default()
        {
            return Ok(());
        }

        // Ensure we have a file path
        if self.file_path.is_none() {
            let dir = config::studies_dir();
            let filename = sanitize_filename(&self.name);
            self.file_path = Some(dir.join(format!("{}.pgn", filename)));
        }

        if let Some(ref path) = self.file_path {
            let pgn_content = pgn::to_pgn(&self.name, &self.tree);
            write_atomic(path, pgn_content.as_bytes())
                .map_err(|e| format!("Failed to save study: {e}"))?;
            self.dirty = false;
        }
        Ok(())
    }

    /// Autosave (only if dirty)
    pub fn autosave(&mut self) -> Result<(), String> {
        if !self.dirty {
            return Ok(());
        }
        self.save()
    }

    /// Load a study from a PGN file
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        let content =
            fs::read_to_string(path).map_err(|e| format!("Failed to read study file: {e}"))?;
        let (name, tree) = pgn::parse_pgn(&content)?;

        Ok(Self {
            name,
            file_path: Some(path.to_path_buf()),
            tree,
            dirty: false,
        })
    }

    /// Rename the study
    pub fn rename(&mut self, new_name: String) {
        if self.name != new_name {
            self.name = new_name.clone();

            // Update file path
            if let Some(ref path) = self.file_path {
                let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
                let filename = sanitize_filename(&new_name);
                self.file_path = Some(dir.join(format!("{}.pgn", filename)));
            }

            self.dirty = true;
        }
    }

    /// Set the note title at a specific path
    pub fn set_note_title_at_path(&mut self, path: &[usize], title: String) {
        if let Some(ann) = self.tree.annotation_at_path(path) {
            let new_ann = NodeAnnotation {
                title,
                note: ann.note.clone(),
            };
            if self.tree.set_annotation_at_path(path, new_ann) {
                self.dirty = true;
            }
        }
    }

    /// Clear the annotation at a specific path
    pub fn clear_note_at_path(&mut self, path: &[usize]) {
        if self
            .tree
            .set_annotation_at_path(path, NodeAnnotation::default())
        {
            self.dirty = true;
        }
    }

    /// Delete a branch from the tree
    pub fn delete_branch(&mut self, parent_path: &[usize], child_idx: usize) {
        self.tree.remove_branch(parent_path, child_idx);
        self.dirty = true;
    }

    /// Get the studies directory
    pub fn studies_dir() -> PathBuf {
        config::studies_dir()
    }
}

/// Initialization options for creating a Study
#[derive(Debug, Clone)]
pub enum StudyInit {
    Empty,
    FromPosition(Chess),
    FromMoves(Vec<Move>),
    FromPgn(String),
}

impl StudyInit {
    pub fn into_study(self, default_name: &str) -> Result<Study, String> {
        match self {
            StudyInit::Empty => Ok(Study::new(default_name)),
            StudyInit::FromPosition(pos) => Ok(Study::from_position(default_name, pos)),
            StudyInit::FromMoves(moves) => Ok(Study::from_moves(default_name, moves)),
            StudyInit::FromPgn(pgn) => Study::from_pgn(&pgn),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_study_is_clean() {
        let study = Study::new("Test");
        assert!(!study.dirty);
    }

    #[test]
    fn test_save_empty_study_does_not_create_file() {
        let mut study = Study::new("TestEmpty");
        study.dirty = true;

        let result = study.save();
        assert!(result.is_ok());

        // It should NOT have assigned a file path, because it returned early.
        assert!(study.file_path.is_none());
    }
}
