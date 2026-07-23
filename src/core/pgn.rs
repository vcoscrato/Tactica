//! PGN (Portable Game Notation) parsing and serialization.
//!
//! This module handles reading and writing PGN files for the analysis feature.
//! It supports variations, comments, and a custom `[title]` prefix in comments
//! to store move titles separately from general notes.

use pgn_reader::{RawComment, RawTag, Reader, SanPlus, Skip, Visitor};
use shakmaty::{Chess, Color, Position, fen::Fen};
use std::ops::ControlFlow;

use crate::core::board::{ChessTree, NodeAnnotation, PositionNode};

#[derive(Debug, Clone)]
pub struct PgnParseResult {
    pub name: String,
    pub tree: ChessTree,
    pub warnings: Vec<String>,
}

impl PgnParseResult {
    pub fn warning_summary(&self) -> Option<String> {
        if self.warnings.is_empty() {
            return None;
        }

        let shown = self
            .warnings
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join("; ");
        let hidden = self.warnings.len().saturating_sub(3);

        if hidden == 0 {
            Some(format!("PGN imported with warnings: {shown}"))
        } else {
            Some(format!(
                "PGN imported with {} warnings: {}; and {} more",
                self.warnings.len(),
                shown,
                hidden
            ))
        }
    }
}

/// Parse a PGN string into a name and ChessTree
pub fn parse_pgn(pgn: &str) -> Result<(String, ChessTree), String> {
    let result = parse_pgn_detailed(pgn)?;
    Ok((result.name, result.tree))
}

/// Parse a PGN string and keep recoverable parse diagnostics.
pub fn parse_pgn_detailed(pgn: &str) -> Result<PgnParseResult, String> {
    let mut reader = Reader::new(pgn.as_bytes());
    let mut visitor = PgnVisitor::new();

    reader
        .read_game(&mut visitor)
        .map_err(|e| format!("PGN parse error: {e}"))?;

    let name = visitor
        .event
        .clone()
        .unwrap_or_else(|| "Untitled".to_string());
    let warnings = visitor.warnings.clone();
    let tree = visitor.into_tree();

    Ok(PgnParseResult {
        name,
        tree,
        warnings,
    })
}

/// Parse a FEN string into a starting position
pub fn parse_fen(fen: &str) -> Result<Chess, String> {
    let parsed: Fen = fen.parse().map_err(|e| format!("Invalid FEN: {e:?}"))?;
    parsed
        .into_position(shakmaty::CastlingMode::Standard)
        .map_err(|e| format!("Invalid position: {e:?}"))
}

/// Serialize a ChessTree to PGN format
pub fn to_pgn(name: &str, tree: &ChessTree) -> String {
    let mut output = String::new();

    // Headers
    output.push_str(&format!("[Event \"{}\"]\n", escape_pgn_string(name)));
    output.push_str(&format!("[Site \"{}\"]\n", crate::metadata::APP_NAME));
    output.push_str(&format!("[Date \"{}.??.??\"]\n", chrono_year()));
    output.push_str("[Round \"?\"]\n");
    output.push_str("[White \"?\"]\n");
    output.push_str("[Black \"?\"]\n");
    output.push_str("[Result \"*\"]\n");

    // Check if we have a custom starting position
    let start_fen =
        shakmaty::fen::Fen::from_position(tree.root().position(), shakmaty::EnPassantMode::Legal);
    let default_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    if start_fen.to_string() != default_fen {
        output.push_str(&format!("[FEN \"{}\"]\n", start_fen));
        output.push_str("[SetUp \"1\"]\n");
    }

    output.push('\n');

    // Moves
    let root_pos = tree.root().position();
    let turn = root_pos.turn();
    let fullmoves = root_pos.fullmoves().get() as usize;

    let (start_move, start_is_white) = if turn == Color::White {
        (fullmoves.saturating_sub(1), false)
    } else {
        (fullmoves, true)
    };

    serialize_node(tree.root(), &mut output, start_move, start_is_white, true);

    // Result
    output.push_str(" *\n");

    output
}

/// Check if a string looks like a FEN
pub fn looks_like_fen(s: &str) -> bool {
    let trimmed = s.trim();
    // FEN has 6 parts separated by spaces and contains '/'
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    !parts.is_empty()
        && parts[0].contains('/')
        && parts[0].chars().filter(|c| *c == '/').count() == 7
}

// --- Internal helpers ---

fn chrono_year() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Approximate year calculation (seconds since 1970)
    let year = 1970 + (secs / 31_536_000);
    year.to_string()
}

fn escape_pgn_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn serialize_node(
    node: &PositionNode,
    output: &mut String,
    move_num: usize,
    is_white: bool,
    is_main_line: bool,
) {
    // Write move number if needed
    if node.san().is_some() {
        if is_white {
            output.push_str(&format!("{}. ", move_num));
        } else if !is_main_line {
            // After a variation, black moves need the "..." notation
            output.push_str(&format!("{}... ", move_num));
        }

        // Write the move
        if let Some(san) = node.san() {
            output.push_str(san);
        }

        // Write annotation as comment
        let comment = format_annotation(&node.annotation);
        if !comment.is_empty() {
            output.push_str(&format!(" {{{}}}", comment));
        }

        output.push(' ');
    }

    // Process children
    if !node.children().is_empty() {
        // Main line is first child
        let main_child = &node.children()[0];
        let next_move_num = if is_white { move_num } else { move_num + 1 };
        let next_is_white = !is_white;

        serialize_node(main_child, output, next_move_num, next_is_white, true);

        // Variations are subsequent children
        for var_child in node.children().iter().skip(1) {
            output.push('(');
            serialize_node(var_child, output, next_move_num, next_is_white, false);
            output.push_str(") ");
        }
    }
}

fn format_annotation(annotation: &NodeAnnotation) -> String {
    let title = annotation.title.trim();
    let note = annotation.note.trim();

    if title.is_empty() && note.is_empty() {
        return String::new();
    }

    if title.is_empty() {
        return note.replace(['{', '}'], ""); // Escape braces
    }

    if note.is_empty() {
        return format!("[{}]", title.replace(['{', '}'], ""));
    }

    format!(
        "[{}] {}",
        title.replace(['{', '}'], ""),
        note.replace(['{', '}'], "")
    )
}

fn parse_annotation(comment: &str) -> NodeAnnotation {
    let trimmed = comment.trim();

    // Check for [title] prefix
    if let Some(stripped) = trimmed.strip_prefix('[')
        && let Some(end_bracket) = stripped.find(']')
    {
        let title = stripped[..end_bracket].trim().to_string();
        let note = stripped[end_bracket + 1..].trim().to_string();
        return NodeAnnotation { title, note };
    }

    // No title prefix, entire comment is a note
    NodeAnnotation {
        title: String::new(),
        note: trimmed.to_string(),
    }
}

// --- PGN Visitor for parsing ---

struct PgnVisitor {
    event: Option<String>,
    fen: Option<String>,
    warnings: Vec<String>,

    // Stack of (position, parent_node_path)
    stack: Vec<(Chess, Vec<usize>)>,
    root: PositionNode,
    current_path: Vec<usize>,
    pending_comment: String,
    move_count: usize,
}

impl PgnVisitor {
    fn new() -> Self {
        Self {
            event: None,
            fen: None,
            warnings: Vec::new(),
            stack: Vec::new(),
            root: PositionNode::new_root(Chess::default()),
            current_path: Vec::new(),
            pending_comment: String::new(),
            move_count: 0,
        }
    }

    fn into_tree(self) -> ChessTree {
        let mut tree = ChessTree::new();
        tree.set_root(self.root);
        tree
    }

    fn current_node(&self) -> &PositionNode {
        let mut node = &self.root;
        for &idx in &self.current_path {
            node = &node.children()[idx];
        }
        node
    }

    fn current_node_mut(&mut self) -> &mut PositionNode {
        let mut node = &mut self.root;
        for &idx in &self.current_path {
            node = &mut node.children_mut()[idx];
        }
        node
    }

    fn current_position(&self) -> &Chess {
        self.current_node().position()
    }
}

impl Visitor for PgnVisitor {
    type Tags = ();
    type Movetext = ();
    type Output = ();

    fn begin_tags(&mut self) -> ControlFlow<Self::Output, Self::Tags> {
        self.event = None;
        self.fen = None;
        self.warnings.clear();
        self.stack.clear();
        self.root = PositionNode::new_root(Chess::default());
        self.current_path.clear();
        self.pending_comment.clear();
        self.move_count = 0;
        ControlFlow::Continue(())
    }

    fn tag(
        &mut self,
        _tags: &mut Self::Tags,
        key: &[u8],
        value: RawTag<'_>,
    ) -> ControlFlow<Self::Output> {
        let key_str = String::from_utf8_lossy(key);
        let value_str = value.decode_utf8_lossy();

        match key_str.as_ref() {
            "Event" => self.event = Some(value_str.to_string()),
            "FEN" => self.fen = Some(value_str.to_string()),
            _ => {}
        }
        ControlFlow::Continue(())
    }

    fn begin_movetext(&mut self, _tags: Self::Tags) -> ControlFlow<Self::Output, Self::Movetext> {
        // Set up starting position from FEN if provided
        if let Some(ref fen_str) = self.fen {
            match parse_fen(fen_str) {
                Ok(pos) => {
                    self.root = PositionNode::new_root(pos);
                }
                Err(e) => {
                    self.warnings
                        .push(format!("Invalid FEN header ignored: {e}"));
                }
            }
        }
        ControlFlow::Continue(())
    }

    fn san(&mut self, _movetext: &mut Self::Movetext, san: SanPlus) -> ControlFlow<Self::Output> {
        let position = self.current_position().clone();
        let san_text = san.san.to_string();

        match san.san.to_move(&position) {
            Ok(mv) => {
                if let Ok(child) = PositionNode::from_move(&position, &mv) {
                    let node = self.current_node_mut();
                    node.add_child(child);
                    let new_idx = node.children().len() - 1;
                    self.current_path.push(new_idx);

                    // Apply pending comment if any
                    if !self.pending_comment.is_empty() {
                        let annotation = parse_annotation(&self.pending_comment);
                        self.current_node_mut().annotation = annotation;
                        self.pending_comment.clear();
                    }

                    self.move_count += 1;
                } else {
                    self.warnings.push(format!(
                        "Ignored move at ply {} ({san_text}): illegal in current position",
                        self.move_count + 1
                    ));
                }
            }
            Err(e) => {
                self.warnings.push(format!(
                    "Ignored move at ply {} ({san_text}): {e:?}",
                    self.move_count + 1
                ));
            }
        }
        ControlFlow::Continue(())
    }

    fn comment(
        &mut self,
        _movetext: &mut Self::Movetext,
        comment: RawComment<'_>,
    ) -> ControlFlow<Self::Output> {
        let comment_str = String::from_utf8_lossy(comment.as_bytes()).to_string();

        // If we're at a position with a move, apply to current node
        if !self.current_path.is_empty() {
            let annotation = parse_annotation(&comment_str);
            self.current_node_mut().annotation = annotation;
        } else {
            // Comment before any moves - store for later or apply to root
            self.pending_comment = comment_str;
        }
        ControlFlow::Continue(())
    }

    fn begin_variation(
        &mut self,
        _movetext: &mut Self::Movetext,
    ) -> ControlFlow<Self::Output, Skip> {
        // Save current state
        let pos = self.current_position().clone();
        self.stack.push((pos, self.current_path.clone()));

        // Go back one move to the parent
        if !self.current_path.is_empty() {
            self.current_path.pop();
        }

        ControlFlow::Continue(Skip(false))
    }

    fn end_variation(&mut self, _movetext: &mut Self::Movetext) -> ControlFlow<Self::Output> {
        // Restore saved state
        if let Some((_, path)) = self.stack.pop() {
            self.current_path = path;
        }
        ControlFlow::Continue(())
    }

    fn end_game(&mut self, _movetext: Self::Movetext) -> Self::Output {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_pgn() {
        let pgn = r#"[Event "Test Game"]

1. e4 e5 2. Nf3 *"#;

        let (name, tree) = parse_pgn(pgn).unwrap();
        assert_eq!(name, "Test Game");
        assert_eq!(tree.root().children().len(), 1);
    }

    #[test]
    fn test_looks_like_fen() {
        assert!(looks_like_fen(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
        ));
        assert!(!looks_like_fen("1. e4 e5"));
    }

    #[test]
    fn test_annotation_parsing() {
        let ann = parse_annotation("[Sicilian] This is a note");
        assert_eq!(ann.title, "Sicilian");
        assert_eq!(ann.note, "This is a note");

        let ann2 = parse_annotation("Just a comment");
        assert_eq!(ann2.title, "");
        assert_eq!(ann2.note, "Just a comment");
    }

    #[test]
    fn test_to_pgn_standard_start() {
        // Create a simple tree: 1. e4 e6
        let mut tree = ChessTree::new();
        // 1. e4
        let e4 = shakmaty::san::San::from_ascii(b"e4")
            .unwrap()
            .to_move(tree.position())
            .unwrap();
        tree.play_move(e4).unwrap();
        // 1... e6
        let e6 = shakmaty::san::San::from_ascii(b"e6")
            .unwrap()
            .to_move(tree.position())
            .unwrap();
        tree.play_move(e6).unwrap();

        let output = to_pgn("Test Game", &tree);

        // We expect "1. e4 e6"
        assert!(output.contains("1. e4"));
        assert!(output.contains("e6"));

        // Check for the specific wrong format
        assert!(!output.contains("e4 2. e6"));
    }

    #[test]
    fn test_parse_full_game_repro() {
        let pgn = r#"[Event "Live Chess"]
[Site "Chess.com"]
[Date "2026.02.08"]
[Round "?"]
[White "agnewas29"]
[Black "victorcoscrato"]
[Result "1-0"]
[TimeControl "600"]
[WhiteElo "1841"]
[BlackElo "1811"]
[Termination "agnewas29 won by resignation"]
[ECO "C02"]
[EndTime "2:35:10 GMT+0000"]
[Link "https://www.chess.com/game/live/164386392640?move=0"]

1. e4 e6 2. Nf3 d5 3. e5 c5 4. d4 Nc6 5. c3 Qb6 6. Qb3 cxd4 7. Qxb6 axb6 8. cxd4
Nge7 9. Nc3 Nf5 10. Be3 Nxe3 11. fxe3 Bb4 12. a3 Ba5 13. Bd3 Bxc3+ 14. bxc3 Bd7
15. O-O Na5 16. Rfb1 O-O 17. Rxb6 Rfc8 18. Ng5 Rxc3 19. Bxh7+ Kh8 20. Rf1 Be8
21. Bb1 Rc7 22. e4 Nc4 23. Rb3 Nd2 24. Rh3+ Kg8 25. Rd1 Nxb1 26. Rxb1 dxe4 27.
Nxe4 Bc6 28. Nd6 Rxa3 29. Rxa3 1-0"#;

        let (_, mut tree) = parse_pgn(pgn).unwrap();
        tree.go_to_end(); // Move to end to count moves
        let moves = tree.move_sequence();
        // 29 moves for white + 28 for black (game ended at 29. Rxa3) = 57 half-moves?
        // 29. Rxa3 is the last move.
        // 28... Rxa3 29. Rxa3.
        // So 29 full moves. 29 * 2 - 1 = 57?
        // Let's count explicitly.
        // 1. e4 e6 (2)
        // ...
        // 29. Rxa3 (1)
        // Total = 28 * 2 + 1 = 57.
        assert_eq!(moves.len(), 57, "Expected 57 moves, found {}", moves.len());
    }

    #[test]
    fn test_round_trip_full_game() {
        let pgn = r#"[Event "Live Chess"]
[Site "Chess.com"]
[Date "2026.02.08"]
[Round "?"]
[White "agnewas29"]
[Black "victorcoscrato"]
[Result "1-0"]

1. e4 e6 2. Nf3 d5 3. e5 c5 4. d4 Nc6 5. c3 Qb6 6. Qb3 cxd4 7. Qxb6 axb6 8. cxd4
Nge7 9. Nc3 Nf5 10. Be3 Nxe3 11. fxe3 Bb4 12. a3 Ba5 13. Bd3 Bxc3+ 14. bxc3 Bd7
15. O-O Na5 16. Rfb1 O-O 17. Rxb6 Rfc8 18. Ng5 Rxc3 19. Bxh7+ Kh8 20. Rf1 Be8
21. Bb1 Rc7 22. e4 Nc4 23. Rb3 Nd2 24. Rh3+ Kg8 25. Rd1 Nxb1 26. Rxb1 dxe4 27.
Nxe4 Bc6 28. Nd6 Rxa3 29. Rxa3 1-0"#;

        // Parse
        let (_, tree) = parse_pgn(pgn).unwrap();

        // Serialize back
        let serialized = to_pgn("Live Chess", &tree);

        // Re-parse the serialized PGN
        let (_, mut tree2) = parse_pgn(&serialized).expect("Failed to re-parse serialized PGN");
        tree2.go_to_end();
        let moves2 = tree2.move_sequence();
        assert_eq!(
            moves2.len(),
            57,
            "Round-trip lost moves: expected 57, got {}.\nSerialized PGN:\n{}",
            moves2.len(),
            serialized
        );
    }

    #[test]
    fn test_parse_game_missing_move_fails() {
        // Missing 8. cxd4
        // The game proceeds: 7... axb6 8. Nc3 (skipping cxd4)
        // This makes Nc3 illegal because c3 is occupied by a pawn.
        let pgn = r#"[Event "Broken Game"]
1. e4 e6 2. Nf3 d5 3. e5 c5 4. d4 Nc6 5. c3 Qb6 6. Qb3 cxd4 7. Qxb6 axb6 8. Nc3"#;

        // The parser should report recoverable warnings and return what it could parse.
        let result = parse_pgn_detailed(pgn).unwrap();
        assert!(
            result
                .warnings
                .iter()
                .any(|warning| warning.contains("Ignored move")),
            "Expected an ignored-move warning, got {:?}",
            result.warnings
        );
        let mut tree = result.tree;
        tree.go_to_end();
        let moves = tree.move_sequence();

        // Moves up to 7... axb6 should be parsed.
        // 1. e4 e6 (2)
        // 2. Nf3 d5 (2)
        // 3. e5 c5 (2)
        // 4. d4 Nc6 (2)
        // 5. c3 Qb6 (2)
        // 6. Qb3 cxd4 (2)
        // 7. Qxb6 axb6 (2)
        // Total 14 moves.
        assert_eq!(moves.len(), 14);

        // The last move should be axb6
        let last_move = tree.last_move().unwrap();
        // We can check the UCI or verify it's a capture on b6
        assert_eq!(last_move.to(), shakmaty::Square::B6);
    }
}
