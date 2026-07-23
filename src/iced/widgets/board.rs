//! Chess board rendering with dynamic sizing

use crate::core::config::{AnimationSpeed, BoardTheme};
use crate::core::review::MoveQuality;
use crate::iced::assets;
use crate::iced::style::Palette;
use crate::iced::widgets::review_assets;
use iced::mouse;
use iced::widget::Action;
use iced::widget::canvas::{self, Canvas, Frame, Geometry, LineCap, Path, Stroke};
use iced::widget::image::Handle;
use iced::{Color, Element, Length, Point, Rectangle, Size, Theme};
use shakmaty::{CastlingSide, Chess, File, Move, Piece, Position, Rank, Role, Square};
use std::collections::HashMap;
use std::sync::LazyLock;

// Re-export core types for compatibility
pub use crate::core::board::{ChessTree, NodeAnnotation, PositionNode};

#[derive(Debug, Clone)]
pub struct DragState {
    pub piece: Piece,
    pub start: Square,
    pub position: Point,
}

#[derive(Default)]
pub struct BoardState {
    drag: Option<DragState>,
    arrows: Vec<(Square, Square)>,
    highlighted: Vec<Square>,
    right_click_start: Option<Square>,
    cursor_position: Point,
}

// Piece sizes for different board size ranges
const PIECE_SIZES: [u32; 3] = [60, 100, 150]; // Support boards up to 1200px

fn get_piece_size_tier(square_size: f32) -> u32 {
    let needed = (square_size * 0.9) as u32;
    for &size in PIECE_SIZES.iter() {
        if size >= needed {
            return size;
        }
    }
    PIECE_SIZES[PIECE_SIZES.len() - 1]
}

struct PieceCache {
    images: std::sync::Mutex<HashMap<(char, u32), Handle>>,
}

impl PieceCache {
    fn new() -> Self {
        Self {
            images: std::sync::Mutex::new(HashMap::new()),
        }
    }

    fn get(&self, piece_char: char, size: u32) -> Option<Handle> {
        let mut cache = self.images.lock().ok()?;
        if let Some(handle) = cache.get(&(piece_char, size)) {
            return Some(handle.clone());
        }
        let svg_data = assets::piece_svg(piece_char)?;
        let handle = rasterize_svg(svg_data, size)?;
        cache.insert((piece_char, size), handle.clone());
        Some(handle)
    }
}

static PIECE_CACHE: LazyLock<PieceCache> = LazyLock::new(PieceCache::new);

struct ReviewIconCache {
    images: std::sync::Mutex<HashMap<(MoveQuality, u32), Handle>>,
}

impl ReviewIconCache {
    fn new() -> Self {
        Self {
            images: std::sync::Mutex::new(HashMap::new()),
        }
    }

    fn get(&self, quality: MoveQuality, size: u32) -> Option<Handle> {
        let mut cache = self.images.lock().ok()?;
        if let Some(handle) = cache.get(&(quality, size)) {
            return Some(handle.clone());
        }

        let svg_template = review_icon_svg(quality);
        let svg_data = svg_template.replace("currentColor", "#ffffff");
        let handle = rasterize_svg(&svg_data, size)?;
        cache.insert((quality, size), handle.clone());
        Some(handle)
    }
}

static REVIEW_ICON_CACHE: LazyLock<ReviewIconCache> = LazyLock::new(ReviewIconCache::new);

fn rasterize_svg(svg_data: &str, size: u32) -> Option<Handle> {
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(svg_data, &options).ok()?;
    let original_size = tree.size();
    let scale = size as f32 / original_size.width().max(original_size.height());
    let pixmap_size = resvg::tiny_skia::IntSize::from_wh(size, size)?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height())?;
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let rgba_data: Vec<u8> = pixmap
        .pixels()
        .iter()
        .flat_map(|p| [p.red(), p.green(), p.blue(), p.alpha()])
        .collect();
    Some(Handle::from_rgba(size, size, rgba_data))
}

fn draw_arrow(
    frame: &mut Frame,
    start: Point,
    end: Point,
    width: f32,
    head_size: f32,
    color: Color,
) {
    let diff_x = end.x - start.x;
    let diff_y = end.y - start.y;
    let length = (diff_x * diff_x + diff_y * diff_y).sqrt();

    if length < 1.0 {
        return;
    }

    let angle = diff_y.atan2(diff_x);

    // Shorten body so the stroke doesn't stick out of the head
    // The head is drawn at 'end' pointing back towards 'start'
    // The triangle height is roughly head_size * cos(30).
    // Let's shorten the body by slightly less than the full head length to ensure overlap without poke-through
    let shorten_len = head_size * 0.8;
    let body_end_x = end.x - shorten_len * angle.cos();
    let body_end_y = end.y - shorten_len * angle.sin();
    let body_end = Point::new(body_end_x, body_end_y);

    let stroke = Stroke {
        width,
        style: canvas::Style::Solid(color),
        line_cap: LineCap::Round,
        ..Stroke::default()
    };

    // Draw body
    let body = Path::line(start, body_end);
    frame.stroke(&body, stroke);

    // Draw head (triangle)
    let head_path = Path::new(|b| {
        let tip = end;
        let wing_angle = std::f32::consts::PI / 6.0; // 30 degrees

        let left_wing = Point::new(
            tip.x + head_size * (angle - std::f32::consts::PI + wing_angle).cos(),
            tip.y + head_size * (angle - std::f32::consts::PI + wing_angle).sin(),
        );
        let right_wing = Point::new(
            tip.x + head_size * (angle - std::f32::consts::PI - wing_angle).cos(),
            tip.y + head_size * (angle - std::f32::consts::PI - wing_angle).sin(),
        );

        b.move_to(tip);
        b.line_to(left_wing);
        b.line_to(right_wing);
        b.close();
    });
    frame.fill(&head_path, color);
}

pub fn get_piece_image(piece_char: char, size: u32) -> Option<Handle> {
    PIECE_CACHE.get(piece_char, size)
}

#[derive(Debug, Clone)]
pub enum BoardMessage {
    SquareClicked(Square),
    PieceDropped(Square, Square),
    DragStarted(Square),
}

pub fn move_matches_user_input(mv: &Move, from: Square, to: Square) -> bool {
    if mv.from() != Some(from) {
        return false;
    }
    if mv.to() == to {
        return true;
    }
    if let Move::Castle { king, rook: _ } = mv {
        let king_dest = match mv.castling_side() {
            Some(CastlingSide::KingSide) => Square::from_coords(File::G, king.rank()),
            Some(CastlingSide::QueenSide) => Square::from_coords(File::C, king.rank()),
            None => return false,
        };
        return to == king_dest;
    }
    false
}

#[derive(Clone, Debug)]
struct MoveTarget {
    square: Square,
    is_capture: bool,
}

#[derive(Clone, Debug)]
pub struct AnimatedPiece {
    pub piece: Piece,
    pub start: Square,
    pub end: Square,
    pub progress: f32,
}

#[derive(Debug, Clone)]
pub enum BoardEvent {
    /// A legal move was made. The bool indicates if it was via drag-and-drop.
    MoveMade(Move, bool),
    MoveAttempted(Square, Square),
    SelectionChanged(Option<Square>),
    PromotionRequired(Square, Square, Vec<Move>),
    NavigationChanged,
}

#[derive(Debug, Clone)]
pub struct Board {
    selected: Option<Square>,
    pub flipped: bool,
    animated_piece: Option<AnimatedPiece>,
    animation_speed: AnimationSpeed,
    interactive: bool,
    hint_square: Option<Square>,
    theme: BoardTheme,
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

impl Board {
    pub fn new() -> Self {
        Self {
            selected: None,
            flipped: false,
            animated_piece: None,
            animation_speed: AnimationSpeed::Normal,
            interactive: true,
            hint_square: None,
            theme: BoardTheme::default(),
        }
    }

    pub fn set_theme(&mut self, theme: BoardTheme) {
        self.theme = theme;
    }

    pub fn set_flipped(&mut self, flipped: bool) {
        self.flipped = flipped;
    }

    pub fn set_interactive(&mut self, interactive: bool) {
        self.interactive = interactive;
        if !interactive {
            self.selected = None;
        }
    }

    pub fn set_hint(&mut self, square: Option<Square>) {
        self.hint_square = square;
    }

    pub fn hint(&self) -> Option<Square> {
        self.hint_square
    }

    pub fn set_animation_speed(&mut self, speed: AnimationSpeed) {
        self.animation_speed = speed;
    }

    /// Start animating a piece move. Call this AFTER the position has been updated.
    /// `position` should be the new (post-move) position.
    /// If an animation is already running, it is instantly completed (snapped).
    pub fn animate_move(&mut self, mv: &Move, position: &Chess, reverse: bool) {
        if self.animation_speed.is_off() {
            return;
        }

        // Snap any in-progress animation
        self.animated_piece = None;

        let (start, end) = if reverse {
            (mv.to(), mv.from().unwrap_or(Square::A1))
        } else {
            (mv.from().unwrap_or(Square::A1), mv.to())
        };

        if let Some(piece) = position.board().piece_at(end) {
            self.animated_piece = Some(AnimatedPiece {
                piece,
                start,
                end,
                progress: 0.0,
            });
        }
    }

    /// Clear any in-progress animation (for instant jumps like navigate_home/end).
    pub fn clear_animation(&mut self) {
        self.animated_piece = None;
    }

    /// Advance the animation by one tick. Returns true if still animating.
    pub fn tick(&mut self) -> bool {
        if let Some(anim) = &mut self.animated_piece {
            anim.progress += self.animation_speed.progress_per_tick();
            if anim.progress >= 1.0 {
                self.animated_piece = None;
                return false;
            }
            true
        } else {
            false
        }
    }

    pub fn is_animating(&self) -> bool {
        self.animated_piece.is_some()
    }

    pub fn deselect(&mut self) {
        self.selected = None;
    }

    pub fn update(&mut self, position: &Chess, message: BoardMessage) -> Option<BoardEvent> {
        if !self.interactive {
            return None;
        }

        let legal_moves: Vec<Move> = position.legal_moves().iter().cloned().collect();

        match message {
            BoardMessage::DragStarted(sq) => {
                if let Some(piece) = position.board().piece_at(sq)
                    && piece.color == position.turn()
                {
                    let old_selected = self.selected;
                    self.selected = Some(sq);
                    if old_selected != self.selected {
                        return Some(BoardEvent::SelectionChanged(self.selected));
                    }
                }
                None
            }

            BoardMessage::SquareClicked(sq) => {
                if let Some(selected) = self.selected {
                    let matches: Vec<Move> = legal_moves
                        .iter()
                        .filter(|m| move_matches_user_input(m, selected, sq))
                        .cloned()
                        .collect();

                    if matches.len() > 1 {
                        self.selected = None;
                        return Some(BoardEvent::PromotionRequired(selected, sq, matches));
                    } else if let Some(mv) = matches.first().cloned() {
                        self.selected = None;
                        return Some(BoardEvent::MoveMade(mv, false));
                    }

                    if let Some(piece) = position.board().piece_at(sq)
                        && piece.color == position.turn()
                    {
                        self.selected = Some(sq);
                        return Some(BoardEvent::SelectionChanged(self.selected));
                    }

                    self.selected = None;
                    return Some(BoardEvent::MoveAttempted(selected, sq));
                } else if let Some(piece) = position.board().piece_at(sq)
                    && piece.color == position.turn()
                {
                    self.selected = Some(sq);
                    return Some(BoardEvent::SelectionChanged(self.selected));
                }
                None
            }

            BoardMessage::PieceDropped(from, to) => {
                if from == to {
                    return None;
                }

                let matches: Vec<Move> = legal_moves
                    .iter()
                    .filter(|m| move_matches_user_input(m, from, to))
                    .cloned()
                    .collect();

                if matches.len() > 1 {
                    self.selected = None;
                    return Some(BoardEvent::PromotionRequired(from, to, matches));
                } else if let Some(mv) = matches.first().cloned() {
                    self.selected = None;
                    return Some(BoardEvent::MoveMade(mv, true));
                }

                self.selected = None;
                Some(BoardEvent::MoveAttempted(from, to))
            }
        }
    }

    pub fn view(
        &self,
        position: &Chess,
        last_move: Option<&Move>,
        last_move_quality: Option<MoveQuality>,
        length: Length,
    ) -> Element<'static, BoardMessage> {
        let legal_moves: Vec<Move> = position.legal_moves().iter().cloned().collect();

        let moves_for_selected: Vec<Move> = if let Some(sq) = self.selected {
            legal_moves
                .iter()
                .filter(|m| m.from() == Some(sq))
                .cloned()
                .collect()
        } else {
            Vec::new()
        };

        let move_targets: Vec<MoveTarget> = moves_for_selected
            .iter()
            .map(|m| {
                let target_square = if let Move::Castle { king, rook: _ } = m {
                    match m.castling_side() {
                        Some(CastlingSide::KingSide) => Square::from_coords(File::G, king.rank()),
                        Some(CastlingSide::QueenSide) => Square::from_coords(File::C, king.rank()),
                        None => m.to(),
                    }
                } else {
                    m.to()
                };
                MoveTarget {
                    square: target_square,
                    is_capture: m.is_capture(),
                }
            })
            .collect();

        Canvas::new(BoardCanvas {
            position: position.clone(),
            selected: self.selected,
            hint_highlight: self.hint_square,
            move_targets,
            last_move: last_move.cloned(),
            last_move_quality,
            flipped: self.flipped,
            animated_piece: self.animated_piece.clone(),
            theme: self.theme,
        })
        .width(length)
        .height(length)
        .into()
    }
}

fn piece_to_char(piece: Piece) -> char {
    match (piece.color.is_white(), piece.role) {
        (true, Role::King) => 'K',
        (true, Role::Queen) => 'Q',
        (true, Role::Rook) => 'R',
        (true, Role::Bishop) => 'B',
        (true, Role::Knight) => 'N',
        (true, Role::Pawn) => 'P',
        (false, Role::King) => 'k',
        (false, Role::Queen) => 'q',
        (false, Role::Rook) => 'r',
        (false, Role::Bishop) => 'b',
        (false, Role::Knight) => 'n',
        (false, Role::Pawn) => 'p',
    }
}

fn review_icon_svg(quality: MoveQuality) -> &'static str {
    review_assets::icon_svg_str(quality)
}

fn review_badge_color(quality: MoveQuality) -> Color {
    review_assets::quality_color_fixed(quality)
}

#[derive(Debug, Clone)]
struct BoardCanvas {
    position: Chess,
    selected: Option<Square>,
    hint_highlight: Option<Square>,
    move_targets: Vec<MoveTarget>,
    last_move: Option<Move>,
    last_move_quality: Option<MoveQuality>,
    flipped: bool,
    animated_piece: Option<AnimatedPiece>,
    theme: BoardTheme,
}

impl canvas::Program<BoardMessage> for BoardCanvas {
    type State = BoardState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<BoardMessage>> {
        let cursor_position = cursor.position_in(bounds)?;

        state.cursor_position = cursor_position;

        let board_size = bounds.width.min(bounds.height);
        let square_size = board_size / 8.0;
        let offset_x = (bounds.width - board_size) / 2.0;
        let offset_y = (bounds.height - board_size) / 2.0;

        let adjusted_x = cursor_position.x - offset_x;
        let adjusted_y = cursor_position.y - offset_y;

        // Check if inside board area
        if adjusted_x < 0.0
            || adjusted_x >= board_size
            || adjusted_y < 0.0
            || adjusted_y >= board_size
        {
            // Clear highlights/arrows on click outside board (left or right)
            if let canvas::Event::Mouse(mouse::Event::ButtonPressed(_)) = event {
                state.highlighted.clear();
                state.arrows.clear();
                return Some(Action::request_redraw().and_capture());
            }
            return None;
        }

        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                // Clear highlights/arrows on left click
                state.highlighted.clear();
                state.arrows.clear();

                let file = (adjusted_x / square_size) as u32;
                let rank = 7 - (adjusted_y / square_size) as u32;
                let (file, rank) = if self.flipped {
                    (7 - file, 7 - rank)
                } else {
                    (file, rank)
                };

                if file < 8 && rank < 8 {
                    let square = Square::from_coords(File::new(file), Rank::new(rank));

                    if let Some(piece) = self.position.board().piece_at(square)
                        && piece.color == self.position.turn()
                    {
                        state.drag = Some(DragState {
                            piece,
                            start: square,
                            position: cursor_position, // Use absolute position for drag visual
                        });
                        return Some(
                            Action::publish(BoardMessage::DragStarted(square)).and_capture(),
                        );
                    }
                    return Some(
                        Action::publish(BoardMessage::SquareClicked(square)).and_capture(),
                    );
                }
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                let file = (adjusted_x / square_size) as u32;
                let rank = 7 - (adjusted_y / square_size) as u32;
                let (file, rank) = if self.flipped {
                    (7 - file, 7 - rank)
                } else {
                    (file, rank)
                };
                if file < 8 && rank < 8 {
                    let square = Square::from_coords(File::new(file), Rank::new(rank));
                    state.right_click_start = Some(square);
                    return Some(Action::request_redraw().and_capture());
                }
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Right)) => {
                if let Some(start) = state.right_click_start.take() {
                    let file = (adjusted_x / square_size) as u32;
                    let rank = 7 - (adjusted_y / square_size) as u32;
                    let (file, rank) = if self.flipped {
                        (7 - file, 7 - rank)
                    } else {
                        (file, rank)
                    };

                    if file < 8 && rank < 8 {
                        let end = Square::from_coords(File::new(file), Rank::new(rank));
                        if start == end {
                            // Toggle highlight
                            if let Some(idx) = state.highlighted.iter().position(|&s| s == start) {
                                state.highlighted.remove(idx);
                            } else {
                                state.highlighted.push(start);
                            }
                        } else {
                            // Toggle arrow
                            if let Some(idx) = state
                                .arrows
                                .iter()
                                .position(|&(s, e)| s == start && e == end)
                            {
                                state.arrows.remove(idx);
                            } else {
                                state.arrows.push((start, end));
                            }
                        }
                        return Some(Action::request_redraw().and_capture());
                    }
                }
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { position: _ }) => {
                if let Some(drag) = &mut state.drag {
                    drag.position = cursor_position; // Use absolute position
                    return Some(Action::request_redraw().and_capture());
                }
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if let Some(drag) = state.drag.take() {
                    let file = (adjusted_x / square_size) as u32;
                    let rank = 7 - (adjusted_y / square_size) as u32;
                    let (file, rank) = if self.flipped {
                        (7 - file, 7 - rank)
                    } else {
                        (file, rank)
                    };

                    if file < 8 && rank < 8 {
                        let end = Square::from_coords(File::new(file), Rank::new(rank));
                        if end == drag.start {
                            return Some(Action::request_redraw().and_capture());
                        } else {
                            return Some(
                                Action::publish(BoardMessage::PieceDropped(drag.start, end))
                                    .and_capture(),
                            );
                        }
                    }
                    return Some(Action::request_redraw().and_capture());
                }
            }
            _ => {}
        }
        None
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &iced::Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        let board_size = bounds.width.min(bounds.height);
        let square_size = board_size / 8.0;
        let offset_x = (bounds.width - board_size) / 2.0;
        let offset_y = (bounds.height - board_size) / 2.0;

        let piece_display_size = square_size * 0.85;

        // Theme colors
        let (light_square, dark_square) = Palette::board_theme_colors(self.theme);
        let last_move_color = Palette::board_highlight_move(theme);
        // let hint_color = Palette::board_highlight_check(theme); // Use accent for hint?
        let hint_color = Color::from_rgba(0.0, 0.85, 1.0, 0.7); // Keep original hint color for now
        let selected_color = Color::from_rgba(0.0, 0.6, 0.0, 0.5);
        let legal_move_color = Color::from_rgba(0.0, 0.0, 0.0, 0.2);
        let capture_color = Color::from_rgba(0.8, 0.2, 0.2, 0.6);

        for rank_idx in 0..8 {
            for file_idx in 0..8 {
                let is_light = (rank_idx + file_idx) % 2 != 0;
                let color = if is_light { light_square } else { dark_square };

                let (draw_file, draw_rank) = if self.flipped {
                    (7 - file_idx, 7 - rank_idx)
                } else {
                    (file_idx, rank_idx)
                };

                // Add offsets here
                let x = offset_x + draw_file as f32 * square_size;
                let y = offset_y + (7 - draw_rank) as f32 * square_size;

                frame.fill_rectangle(Point::new(x, y), Size::new(square_size, square_size), color);

                let square = Square::from_coords(File::new(file_idx), Rank::new(rank_idx));

                // Coordinates
                let coord_color = if is_light { dark_square } else { light_square };
                let font_size = (square_size * 0.18).max(10.0);

                let is_left_edge =
                    (self.flipped && file_idx == 7) || (!self.flipped && file_idx == 0);
                if is_left_edge {
                    let rank_char = char::from_digit(rank_idx + 1, 10).unwrap_or('?');
                    let text_pos = Point::new(x + font_size * 0.3, y + font_size * 0.2);
                    frame.fill_text(canvas::Text {
                        content: rank_char.to_string(),
                        position: text_pos,
                        color: coord_color,
                        size: font_size.into(),
                        align_x: iced::alignment::Horizontal::Left.into(),
                        align_y: iced::alignment::Vertical::Top,
                        ..Default::default()
                    });
                }

                let is_bottom_edge =
                    (self.flipped && rank_idx == 7) || (!self.flipped && rank_idx == 0);
                if is_bottom_edge {
                    let file_char = (b'a' + file_idx as u8) as char;
                    let text_pos = Point::new(
                        x + square_size - font_size * 0.4,
                        y + square_size - font_size * 0.2,
                    );
                    frame.fill_text(canvas::Text {
                        content: file_char.to_string(),
                        position: text_pos,
                        color: coord_color,
                        size: font_size.into(),
                        align_x: iced::alignment::Horizontal::Right.into(),
                        align_y: iced::alignment::Vertical::Bottom,
                        ..Default::default()
                    });
                }

                // Highlight last move
                if let Some(lm) = self.last_move.as_ref()
                    && (lm.from() == Some(square) || lm.to() == square)
                {
                    frame.fill_rectangle(
                        Point::new(x, y),
                        Size::new(square_size, square_size),
                        last_move_color,
                    );
                }

                // Highlight hint
                if Some(square) == self.hint_highlight {
                    frame.fill_rectangle(
                        Point::new(x, y),
                        Size::new(square_size, square_size),
                        hint_color,
                    );
                }

                // Highlight selected
                if Some(square) == self.selected {
                    frame.fill_rectangle(
                        Point::new(x, y),
                        Size::new(square_size, square_size),
                        selected_color,
                    );
                }

                // Custom Highlights (Right Click)
                if state.highlighted.contains(&square) {
                    frame.fill_rectangle(
                        Point::new(x, y),
                        Size::new(square_size, square_size),
                        Color::from_rgba(0.9, 0.2, 0.2, 0.6), // Red tint like chess.com
                    );
                }

                // Moves
                if let Some(target) = self.move_targets.iter().find(|t| t.square == square) {
                    let center = Point::new(x + square_size / 2.0, y + square_size / 2.0);
                    let radius = if target.is_capture {
                        square_size * 0.4
                    } else {
                        square_size * 0.15
                    };
                    let color = if target.is_capture {
                        capture_color
                    } else {
                        legal_move_color
                    };

                    if target.is_capture {
                        let stroke = canvas::Stroke::default()
                            .with_color(color)
                            .with_width(square_size * 0.1);
                        let circle = Path::circle(center, radius);
                        frame.stroke(&circle, stroke);
                    } else {
                        let circle = Path::circle(center, radius);
                        frame.fill(&circle, color);
                    }
                }

                // Draw Piece
                let mut drawn_piece = self.position.board().piece_at(square);

                if let Some(anim) = &self.animated_piece
                    && anim.end == square
                {
                    drawn_piece = None;
                }

                if let Some(drag) = &state.drag
                    && drag.start == square
                {
                    drawn_piece = None;
                }

                if let Some(piece) = drawn_piece {
                    let piece_size_tier = get_piece_size_tier(square_size);
                    if let Some(handle) = PIECE_CACHE.get(piece_to_char(piece), piece_size_tier) {
                        let piece_offset = (square_size - piece_display_size) / 2.0;
                        let piece_pos = Point::new(x + piece_offset, y + piece_offset);
                        let bounds = Rectangle::new(
                            piece_pos,
                            Size::new(piece_display_size, piece_display_size),
                        );
                        frame.draw_image(bounds, &handle);
                    }
                }
            }
        }

        if let (Some(lm), Some(quality)) = (self.last_move.as_ref(), self.last_move_quality) {
            let sq = lm.to();
            let file_idx = u32::from(sq.file());
            let rank_idx = u32::from(sq.rank());
            let (draw_file, draw_rank) = if self.flipped {
                (7 - file_idx, 7 - rank_idx)
            } else {
                (file_idx, rank_idx)
            };

            let x = offset_x + draw_file as f32 * square_size;
            let y = offset_y + (7 - draw_rank) as f32 * square_size;

            let badge_d = (square_size * 0.42).max(18.0);
            let mut center = Point::new(x + square_size - badge_d * 0.30, y + badge_d * 0.30);

            let radius = badge_d / 2.0;
            let min_x = offset_x + radius + 1.0;
            let min_y = offset_y + radius + 1.0;
            let max_x = offset_x + board_size - radius - 1.0;
            let max_y = offset_y + board_size - radius - 1.0;
            center.x = center.x.clamp(min_x, max_x);
            center.y = center.y.clamp(min_y, max_y);

            let shadow = Path::circle(Point::new(center.x + 1.0, center.y + 1.0), radius);
            frame.fill(&shadow, Color::from_rgba(0.0, 0.0, 0.0, 0.25));

            let badge = Path::circle(center, radius);
            frame.fill(&badge, review_badge_color(quality));
            frame.stroke(
                &badge,
                Stroke::default()
                    .with_color(Color::from_rgba(1.0, 1.0, 1.0, 0.95))
                    .with_width((square_size * 0.03).max(1.2)),
            );

            let icon_size = (badge_d * 0.62).max(12.0) as u32;
            if let Some(handle) = REVIEW_ICON_CACHE.get(quality, icon_size) {
                let icon_size_f = icon_size as f32;
                let icon_pos =
                    Point::new(center.x - icon_size_f / 2.0, center.y - icon_size_f / 2.0);
                let bounds = Rectangle::new(icon_pos, Size::new(icon_size_f, icon_size_f));
                frame.draw_image(bounds, &handle);
            }
        }

        // Animated Piece
        if let Some(anim) = &self.animated_piece {
            let start_file = u32::from(anim.start.file());
            let start_rank = u32::from(anim.start.rank());
            let end_file = u32::from(anim.end.file());
            let end_rank = u32::from(anim.end.rank());

            let (f1, r1) = if self.flipped {
                (7 - start_file, 7 - start_rank)
            } else {
                (start_file, start_rank)
            };
            let (f2, r2) = if self.flipped {
                (7 - end_file, 7 - end_rank)
            } else {
                (end_file, end_rank)
            };

            let x1 = offset_x + f1 as f32 * square_size;
            let y1 = offset_y + (7 - r1) as f32 * square_size;
            let x2 = offset_x + f2 as f32 * square_size;
            let y2 = offset_y + (7 - r2) as f32 * square_size;

            let x = x1 + (x2 - x1) * anim.progress;
            let y = y1 + (y2 - y1) * anim.progress;

            let piece_size_tier = get_piece_size_tier(square_size);
            if let Some(handle) = PIECE_CACHE.get(piece_to_char(anim.piece), piece_size_tier) {
                let piece_offset = (square_size - piece_display_size) / 2.0;
                let bounds = Rectangle::new(
                    Point::new(x + piece_offset, y + piece_offset),
                    Size::new(piece_display_size, piece_display_size),
                );
                frame.draw_image(bounds, canvas::Image::new(handle.clone()));
            }
        }

        // Draw Arrows
        let arrow_color = Color::from_rgba(1.0, 0.6, 0.0, 0.8); // Orange opaque
        let arrow_width = square_size * 0.15;
        let head_size = square_size * 0.4;

        for &(start, end) in &state.arrows {
            let start_file = u32::from(start.file());
            let start_rank = u32::from(start.rank());
            let end_file = u32::from(end.file());
            let end_rank = u32::from(end.rank());

            let (f1, r1) = if self.flipped {
                (7 - start_file, 7 - start_rank)
            } else {
                (start_file, start_rank)
            };
            let (f2, r2) = if self.flipped {
                (7 - end_file, 7 - end_rank)
            } else {
                (end_file, end_rank)
            };

            let x1 = offset_x + f1 as f32 * square_size + square_size / 2.0;
            let y1 = offset_y + (7 - r1) as f32 * square_size + square_size / 2.0;
            let x2 = offset_x + f2 as f32 * square_size + square_size / 2.0;
            let y2 = offset_y + (7 - r2) as f32 * square_size + square_size / 2.0;

            draw_arrow(
                &mut frame,
                Point::new(x1, y1),
                Point::new(x2, y2),
                arrow_width,
                head_size,
                arrow_color,
            );
        }

        // Draw Ghost Arrow (Right Click Drag)
        if let Some(start) = state.right_click_start {
            // Calculate end square
            let adjusted_x = state.cursor_position.x - offset_x;
            let adjusted_y = state.cursor_position.y - offset_y;

            let file = (adjusted_x / square_size) as i32;
            let rank = 7 - (adjusted_y / square_size) as i32;

            // Bounds check
            if (0..8).contains(&file) && (0..8).contains(&rank) {
                let (file, rank) = if self.flipped {
                    (7 - file, 7 - rank)
                } else {
                    (file, rank)
                };

                let end = Square::from_coords(File::new(file as u32), Rank::new(rank as u32));

                if start != end {
                    // Same logic as `draw_arrows` loop
                    let start_file = u32::from(start.file());
                    let start_rank = u32::from(start.rank());
                    let end_file = u32::from(end.file());
                    let end_rank = u32::from(end.rank());

                    let (f1, r1) = if self.flipped {
                        (7 - start_file, 7 - start_rank)
                    } else {
                        (start_file, start_rank)
                    };
                    let (f2, r2) = if self.flipped {
                        (7 - end_file, 7 - end_rank)
                    } else {
                        (end_file, end_rank)
                    };

                    let x1 = offset_x + f1 as f32 * square_size + square_size / 2.0;
                    let y1 = offset_y + (7 - r1) as f32 * square_size + square_size / 2.0;
                    let x2 = offset_x + f2 as f32 * square_size + square_size / 2.0;
                    let y2 = offset_y + (7 - r2) as f32 * square_size + square_size / 2.0;

                    // Semi-transparent for ghost arrow
                    let ghost_color = Color::from_rgba(1.0, 0.6, 0.0, 0.5);
                    draw_arrow(
                        &mut frame,
                        Point::new(x1, y1),
                        Point::new(x2, y2),
                        arrow_width,
                        head_size,
                        ghost_color,
                    );
                }
            }
        }

        // Dragged Piece
        if let Some(drag) = &state.drag {
            let piece_size_tier = get_piece_size_tier(square_size);
            if let Some(handle) = PIECE_CACHE.get(piece_to_char(drag.piece), piece_size_tier) {
                // center_offset relative to drag position is independent of board offset, but drag.position is absolute
                // and draw_image takes absolute coordinates.
                let center_offset = square_size / 2.0;
                let top_left = Point::new(
                    drag.position.x - center_offset,
                    drag.position.y - center_offset,
                );
                frame.draw_image(
                    Rectangle::new(top_left, Size::new(square_size, square_size)),
                    canvas::Image::new(handle.clone()),
                );
            }
        }

        vec![frame.into_geometry()]
    }
}
