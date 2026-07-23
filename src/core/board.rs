use shakmaty::fen::Fen;
use shakmaty::{Chess, Move, Position, san::San};

/// Convert a position to its FEN string representation.
pub fn position_to_fen(pos: &Chess) -> String {
    Fen::from_position(pos, shakmaty::EnPassantMode::Legal).to_string()
}

#[derive(Clone, Debug, Default)]
pub struct NodeAnnotation {
    pub title: String,
    pub note: String,
}

impl NodeAnnotation {
    pub fn is_empty(&self) -> bool {
        self.title.trim().is_empty() && self.note.trim().is_empty()
    }
}

#[derive(Clone, Debug)]
pub struct PositionNode {
    position: Chess,
    mv: Option<Move>,    // The move that led here (None for root)
    san: Option<String>, // SAN notation of the move
    uci: Option<String>, // UCI notation of the move
    children: Vec<PositionNode>,
    pub annotation: NodeAnnotation,
}

impl PositionNode {
    pub fn new_root(position: Chess) -> Self {
        Self {
            position,
            mv: None,
            san: None,
            uci: None,
            children: Vec::new(),
            annotation: NodeAnnotation::default(),
        }
    }

    pub fn from_move(parent_position: &Chess, mv: &Move) -> Result<Self, String> {
        let san = San::from_move(parent_position, *mv).to_string();
        let uci = format!(
            "{}{}",
            mv.from().map(|s| s.to_string()).unwrap_or_default(),
            mv.to()
        );
        let position = parent_position
            .clone()
            .play(*mv)
            .map_err(|_| "Illegal move".to_string())?;
        Ok(Self {
            position,
            mv: Some(*mv),
            san: Some(san),
            uci: Some(uci),
            children: Vec::new(),
            annotation: NodeAnnotation::default(),
        })
    }

    pub fn from_san(parent_position: &Chess, san_str: &str) -> Option<Self> {
        let san: San = san_str.parse().ok()?;
        let mv = san.to_move(parent_position).ok()?;
        Self::from_move(parent_position, &mv).ok()
    }

    pub fn add_move(&mut self, mv: &Move) -> Result<usize, String> {
        for (i, child) in self.children.iter().enumerate() {
            if child.mv.as_ref() == Some(mv) {
                return Ok(i);
            }
        }
        self.children
            .push(PositionNode::from_move(&self.position, mv)?);
        Ok(self.children.len() - 1)
    }

    pub fn add_san(&mut self, san_str: &str) -> Option<usize> {
        let san: San = san_str.parse().ok()?;
        let mv = san.to_move(&self.position).ok()?;
        self.add_move(&mv).ok()
    }

    pub fn position(&self) -> &Chess {
        &self.position
    }
    pub fn mv(&self) -> Option<&Move> {
        self.mv.as_ref()
    }
    pub fn san(&self) -> Option<&str> {
        self.san.as_deref()
    }
    pub fn uci(&self) -> Option<&str> {
        self.uci.as_deref()
    }
    pub fn children(&self) -> &[PositionNode] {
        &self.children
    }
    pub fn children_mut(&mut self) -> &mut Vec<PositionNode> {
        &mut self.children
    }
    pub fn add_child(&mut self, node: PositionNode) {
        self.children.push(node);
    }
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct ChessTree {
    root: PositionNode,
    current_path: Vec<usize>,
}

impl Default for ChessTree {
    fn default() -> Self {
        Self::new()
    }
}

impl ChessTree {
    pub fn new() -> Self {
        Self {
            root: PositionNode::new_root(Chess::default()),
            current_path: Vec::new(),
        }
    }

    fn node_at_path(&self, path: &[usize]) -> Option<&PositionNode> {
        let mut node = &self.root;
        for &idx in path {
            node = node.children.get(idx)?;
        }
        Some(node)
    }

    fn node_at_path_mut(&mut self, path: &[usize]) -> Option<&mut PositionNode> {
        let mut node = &mut self.root;
        for &idx in path {
            let child = node.children.get_mut(idx)?;
            node = child;
        }
        Some(node)
    }

    pub fn with_position(position: Chess) -> Self {
        Self {
            root: PositionNode::new_root(position),
            current_path: Vec::new(),
        }
    }

    fn current_node(&self) -> &PositionNode {
        self.node_at_path(&self.current_path)
            .expect("current path should always be valid")
    }

    fn current_node_mut(&mut self) -> &mut PositionNode {
        let path = self.current_path.clone();
        self.node_at_path_mut(&path)
            .expect("current path should always be valid")
    }

    pub fn position(&self) -> &Chess {
        self.current_node().position()
    }

    pub fn current_path(&self) -> &[usize] {
        &self.current_path
    }

    pub fn depth(&self) -> usize {
        self.current_path.len()
    }

    pub fn last_move(&self) -> Option<&Move> {
        self.current_node().mv()
    }

    pub fn legal_moves(&self) -> Vec<Move> {
        self.position().legal_moves().iter().cloned().collect()
    }

    pub fn go_back(&mut self) -> bool {
        if self.current_path.is_empty() {
            false
        } else {
            self.current_path.pop();
            true
        }
    }

    pub fn go_forward(&mut self) -> bool {
        if self.current_node().children.is_empty() {
            false
        } else {
            self.current_path.push(0);
            true
        }
    }

    pub fn go_to_start(&mut self) {
        self.current_path.clear();
    }

    pub fn go_to_end(&mut self) {
        let mut path = self.current_path.clone();
        let mut curr = self.node_at_path(&path).unwrap_or(&self.root);
        while let Some(child) = curr.children.first() {
            path.push(0);
            curr = child;
        }
        self.current_path = path;
    }

    pub fn go_to_path(&mut self, path: &[usize]) -> bool {
        if self.node_at_path(path).is_some() {
            self.current_path = path.to_vec();
            true
        } else {
            false
        }
    }

    pub fn variations(&self) -> &[PositionNode] {
        self.current_node().children()
    }

    pub fn variation_sans(&self) -> Vec<String> {
        self.current_node()
            .children()
            .iter()
            .filter_map(|c| c.san().map(|s| s.to_string()))
            .collect()
    }

    pub fn move_sequence(&self) -> Vec<Move> {
        let mut moves = Vec::new();
        let mut node = &self.root;
        for &idx in &self.current_path {
            let child = &node.children[idx];
            if let Some(mv) = child.mv() {
                moves.push(*mv);
            }
            node = child;
        }
        moves
    }

    pub fn play_move(&mut self, mv: Move) -> Result<usize, String> {
        let child_idx = self.current_node_mut().add_move(&mv)?;
        self.current_path.push(child_idx);
        Ok(child_idx)
    }

    pub fn undo(&mut self) -> bool {
        self.go_back()
    }

    pub fn annotation(&self) -> &NodeAnnotation {
        &self.current_node().annotation
    }

    pub fn annotation_mut(&mut self) -> &mut NodeAnnotation {
        &mut self.current_node_mut().annotation
    }

    pub fn set_note(&mut self, note: String) {
        self.current_node_mut().annotation.note = note;
    }

    pub fn set_note_title(&mut self, title: String) {
        self.current_node_mut().annotation.title = title;
    }

    pub fn annotation_at_path(&self, path: &[usize]) -> Option<&NodeAnnotation> {
        self.node_at_path(path).map(|node| &node.annotation)
    }

    pub fn set_annotation_at_path(&mut self, path: &[usize], annotation: NodeAnnotation) -> bool {
        if let Some(node) = self.node_at_path_mut(path) {
            node.annotation = annotation;
            true
        } else {
            false
        }
    }

    pub fn remove_branch(&mut self, parent_path: &[usize], child_idx: usize) {
        let Some(node) = self.node_at_path_mut(parent_path) else {
            return;
        };
        if child_idx < node.children.len() {
            node.children.remove(child_idx);
        }
    }

    pub fn set_position(&mut self, position: Chess) {
        self.root = PositionNode::new_root(position);
        self.current_path.clear();
    }

    pub fn root(&self) -> &PositionNode {
        &self.root
    }

    pub fn set_root(&mut self, root: PositionNode) {
        self.root = root;
        self.current_path.clear();
    }

    pub fn root_mut(&mut self) -> &mut PositionNode {
        &mut self.root
    }
}
