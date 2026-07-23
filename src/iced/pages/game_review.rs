//! Game Review mode — linear review of an imported game.
//!
//! Displays quality badges on moves, review summary,
//! and allows ephemeral exploration off the main line.

use iced::Theme;
use iced::widget::{Space, button, column, row, stack, text};
use iced::{Element, Length, Subscription, Task, keyboard};
use shakmaty::{Chess, Move, Position, Role, Square};
use std::collections::HashMap;
use std::sync::Arc;

use crate::core::config::AppSettings;
use crate::core::game_review::GameReview;
use crate::core::review::{self, MoveQuality, MoveReview};
use crate::iced::pages::{AnalysisMessage, GameMode, analysis_subscription};
use crate::iced::panels::GameLayout;
use crate::iced::style::{Palette, buttons};
use crate::iced::widgets::board::{Board, BoardEvent, BoardMessage};
use crate::iced::widgets::common::{game_result_banner, promotion_modal};
use crate::iced::widgets::engine_ui::{self, EngineState};
use crate::iced::widgets::move_ribbon;
use crate::iced::widgets::review_assets;
use crate::iced::widgets::sidebar;

use crate::core::openings::OpeningNames;

pub struct GameReviewMode {
    pub data: GameReview,
    pub(crate) board: Board,
    /// Current depth in the main line (0 = root position)
    current_depth: usize,
    es: EngineState,
    pub settings: AppSettings,
    review_running: bool,
    review_error: Option<String>,
    pending_promotion: Option<(Square, Square, Vec<Move>)>,
    openings: Arc<OpeningNames>,
}

#[derive(Debug, Clone)]
pub enum GameReviewMessage {
    Board(BoardMessage),
    ToggleAnalysis,
    OpenEngineSettings,
    PollEngine,
    PlayLine(usize),
    GoToDepth(usize),
    StepBackward,
    StepForward,
    Tick,
    PromoteTo(Role),
    CancelPromotion,
    KeyPressed(keyboard::Key, keyboard::Modifiers),
    RunReview,
    ReviewFinished(Result<Vec<MoveReview>, String>),
    Save,
    OpenInStudy,
    None,
}

impl GameReviewMode {
    pub fn new(data: GameReview, settings: AppSettings, openings: Arc<OpeningNames>) -> Self {
        let analyzing = settings.engine.enabled;
        let es = EngineState::new(analyzing);
        Self {
            data,
            board: {
                let mut b = Board::new();
                b.set_animation_speed(settings.animation_speed);
                b.set_theme(settings.board_theme);
                b
            },
            current_depth: 0,
            es,
            settings,
            review_running: false,
            review_error: None,
            pending_promotion: None,
            openings,
        }
    }

    pub fn shutdown(&mut self) {
        self.es.shutdown();
    }

    /// Get the current position at the current depth
    fn current_position(&self) -> Chess {
        let mut pos = self.data.tree.root().position().clone();
        let node = self.data.tree.root();
        let mut current = node;
        for _i in 0..self.current_depth {
            if current.children().is_empty() {
                break;
            }
            if let Some(mv) = current.children()[0].mv()
                && let Ok(next) = pos.clone().play(*mv)
            {
                pos = next;
            }
            current = &current.children()[0];
        }
        pos
    }

    /// Get the last move played to reach the current depth
    fn last_move_at_depth(&self) -> Option<&Move> {
        if self.current_depth == 0 {
            return None;
        }
        let mut current = self.data.tree.root();
        for _ in 0..(self.current_depth - 1) {
            if current.children().is_empty() {
                return None;
            }
            current = &current.children()[0];
        }
        if current.children().is_empty() {
            return None;
        }
        current.children()[0].mv()
    }

    fn total_moves(&self) -> usize {
        let mut count = 0;
        let mut node = self.data.tree.root();
        while !node.children().is_empty() {
            count += 1;
            node = &node.children()[0];
        }
        count
    }

    /// Get the next move in the reviewed main line from the current depth.
    fn next_main_line_move(&self) -> Option<&Move> {
        let mut node = self.data.tree.root();
        for _ in 0..self.current_depth {
            node = node.children().first()?;
        }
        node.children().first()?.mv()
    }

    fn go_to_depth(&mut self, depth: usize) {
        let max = self.total_moves();
        let new_depth = depth.min(max);
        let old_depth = self.current_depth;
        self.current_depth = new_depth;

        // Animate to new position
        if new_depth != old_depth {
            self.board.deselect();
            if self.es.analyzing {
                let pos = self.current_position();
                self.es.start_with_settings(&self.settings.engine, &pos);
            }
        }
    }

    fn collect_main_line(&self) -> (Chess, Vec<Move>) {
        let mut moves = Vec::new();
        let mut node = self.data.tree.root();
        while !node.children().is_empty() {
            if let Some(mv) = node.children()[0].mv() {
                moves.push(*mv);
            }
            node = &node.children()[0];
        }
        (self.data.tree.root().position().clone(), moves)
    }

    fn apply_book_tags(&self, mut reviews: Vec<MoveReview>) -> HashMap<usize, MoveReview> {
        let (mut pos, moves) = self.collect_main_line();
        for (i, mv) in moves.iter().enumerate() {
            if let Ok(next) = pos.clone().play(*mv) {
                pos = next;
                if self.openings.is_book_position(&pos)
                    && let Some(review) = reviews.get_mut(i)
                {
                    review.quality = MoveQuality::Book;
                    review.reason = "Book move from opening database".to_string();
                }
            } else {
                break;
            }
        }
        reviews.into_iter().map(|r| (r.ply, r)).collect()
    }
}

impl GameMode for GameReviewMode {
    type Message = GameReviewMessage;

    fn set_settings(&mut self, settings: AppSettings) {
        self.board.set_animation_speed(settings.animation_speed);
        self.board.set_theme(settings.board_theme);
        let pos = self.current_position();
        self.es.apply_settings(&settings, Some(&pos));
        self.settings = settings;
    }

    fn update(&mut self, message: GameReviewMessage) -> Task<GameReviewMessage> {
        match message {
            GameReviewMessage::Board(msg) => {
                if let Some(event) = self.board.update(&self.current_position(), msg) {
                    match event {
                        BoardEvent::MoveMade(mv, _) => {
                            // In review, only follow the next move from the main line.
                            if self.next_main_line_move() == Some(&mv) {
                                let next_depth = self.current_depth + 1;
                                self.go_to_depth(next_depth);
                            }
                        }
                        BoardEvent::PromotionRequired(from, to, moves)
                            if self
                                .next_main_line_move()
                                .is_some_and(|expected| moves.iter().any(|m| m == expected)) =>
                        {
                            self.pending_promotion = Some((from, to, moves));
                        }
                        _ => {}
                    }
                }
            }
            GameReviewMessage::ToggleAnalysis => {
                let pos = self.current_position();
                self.es.toggle(&mut self.settings, &pos);
            }
            GameReviewMessage::OpenEngineSettings => {}
            GameReviewMessage::PollEngine => {
                self.es.poll();
            }
            GameReviewMessage::PlayLine(i) => {
                if let Some(uci) = self.es.analysis.lines.get(i).and_then(|l| l.pv.first()) {
                    let pos = self.current_position();
                    if let Some(pv_move) = engine_ui::parse_uci_move(&pos, uci)
                        && self.next_main_line_move() == Some(&pv_move)
                    {
                        let next_depth = self.current_depth + 1;
                        self.go_to_depth(next_depth);
                    }
                }
            }
            GameReviewMessage::GoToDepth(depth) => {
                self.go_to_depth(depth);
            }
            GameReviewMessage::StepBackward => {
                if self.current_depth > 0 {
                    self.go_to_depth(self.current_depth - 1);
                }
            }
            GameReviewMessage::StepForward => {
                self.go_to_depth(self.current_depth + 1);
            }
            GameReviewMessage::Tick => {
                let pos = self.current_position();
                self.es.ensure_running(&self.settings.engine, &pos);
                self.board.tick();
                self.es.tick_eval_bar(self.settings.show_eval_bar);
            }
            GameReviewMessage::PromoteTo(role) => {
                if let Some((_from, _to, moves)) = self.pending_promotion.take()
                    && self.next_main_line_move().is_some_and(|expected| {
                        expected.promotion() == Some(role) && moves.iter().any(|m| m == expected)
                    })
                {
                    let next_depth = self.current_depth + 1;
                    self.go_to_depth(next_depth);
                }
            }
            GameReviewMessage::CancelPromotion => {
                self.pending_promotion = None;
            }
            GameReviewMessage::KeyPressed(key, modifiers) => {
                if modifiers.control() {
                    return Task::none();
                }
                match key.as_ref() {
                    keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                        if self.current_depth > 0 {
                            self.go_to_depth(self.current_depth - 1);
                        }
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                        self.go_to_depth(self.current_depth + 1);
                    }
                    _ => {}
                }
            }
            GameReviewMessage::RunReview => {
                self.review_running = true;
                self.review_error = None;

                let (start_pos, moves) = self.collect_main_line();
                let settings = self.settings.engine.clone();

                return Task::perform(
                    async move { review::review_line(start_pos, moves, &settings) },
                    GameReviewMessage::ReviewFinished,
                );
            }
            GameReviewMessage::ReviewFinished(result) => {
                self.review_running = false;
                match result {
                    Ok(reviews) => {
                        self.data.set_review_results(self.apply_book_tags(reviews));
                    }
                    Err(e) => {
                        self.review_error = Some(e);
                    }
                }
            }
            GameReviewMessage::Save => {}
            GameReviewMessage::OpenInStudy => {}
            GameReviewMessage::None => {}
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<GameReviewMessage> {
        let mut subs = vec![iced::event::listen_with(
            |event, _status, _window| match event {
                iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                    Some(GameReviewMessage::KeyPressed(key, modifiers))
                }
                _ => None,
            },
        )];
        subs.push(
            analysis_subscription(
                self.es.analyzing,
                self.es.engine.is_some(),
                self.settings.show_eval_bar,
                self.board.is_animating(),
            )
            .map(|message| match message {
                AnalysisMessage::PollEngine => GameReviewMessage::PollEngine,
                AnalysisMessage::Tick => GameReviewMessage::Tick,
            }),
        );
        Subscription::batch(subs)
    }

    fn view(&self, theme: &Theme) -> Element<'_, GameReviewMessage> {
        let s = self.settings.ui_scale;
        let pos = self.current_position();
        let last_mv = self.last_move_at_depth();

        let quality_at_depth = self
            .data
            .review_results
            .get(&self.current_depth)
            .map(|r| r.quality);

        let board = self
            .board
            .view(
                &pos,
                last_mv,
                quality_at_depth,
                Length::Fixed(self.settings.board_size),
            )
            .map(GameReviewMessage::Board);

        // Header
        let title = text(&self.data.name)
            .size(14.0 * s)
            .color(Palette::text_primary(theme));

        let review_btn: Option<Element<'_, GameReviewMessage>> = if self.review_running {
            Some(
                button(text("Reviewing...").size(11.0 * s))
                    .padding([4, 8])
                    .style(buttons::secondary)
                    .into(),
            )
        } else if self.data.review_results.is_empty() {
            Some(
                button(text("Review Line").size(11.0 * s))
                    .padding([4, 8])
                    .style(buttons::primary)
                    .on_press(GameReviewMessage::RunReview)
                    .into(),
            )
        } else {
            None
        };

        let save_btn = button(text("Save").size(11.0 * s))
            .padding([4, 8])
            .style(buttons::secondary)
            .on_press(GameReviewMessage::Save);

        let open_in_study_btn = button(text("Open in Study").size(11.0 * s))
            .padding([4, 8])
            .style(buttons::secondary)
            .on_press(GameReviewMessage::OpenInStudy);

        let mut actions = row![].spacing(6).align_y(iced::Alignment::Center);
        if let Some(review_btn) = review_btn {
            actions = actions.push(review_btn);
        }
        actions = actions.push(save_btn).push(open_in_study_btn);

        let header = row![title, Space::new().width(Length::Fill), actions]
            .spacing(6)
            .align_y(iced::Alignment::Center);

        // Engine lines
        let lines = engine_ui::build_engine_lines(
            theme,
            s,
            &pos,
            &self.es.analysis,
            GameReviewMessage::PlayLine,
        );

        let engine_controls = engine_ui::engine_controls_row(
            theme,
            engine_ui::EngineControlsState {
                ui_scale: s,
                analyzing: self.es.analyzing,
                current_depth: self.es.analysis.depth,
                max_depth: self.settings.engine.max_depth,
                error: self.es.error.as_deref(),
            },
            GameReviewMessage::ToggleAnalysis,
            GameReviewMessage::OpenEngineSettings,
        );

        let engine_content: Element<'_, GameReviewMessage> = if self.es.analyzing {
            column![engine_controls, lines].spacing(8).into()
        } else {
            engine_controls
        };

        // Move ribbon
        let ribbon = self.build_move_ribbon(theme);

        // Review summary
        let review_summary = self.build_review_summary(theme);

        let mut info_panel = column![header].spacing(10.0 * s);
        if let Some(result) = game_result_banner(theme, &pos, s) {
            info_panel = info_panel.push(result);
        }
        info_panel = info_panel
            .push(iced::widget::rule::horizontal(1))
            .push(engine_content)
            .push(iced::widget::rule::horizontal(1))
            .push(review_summary)
            .push(iced::widget::rule::horizontal(1))
            .push(sidebar::section(theme, "Moves".into(), ribbon, s));

        let control_panel = column![
            sidebar::panel_header(theme, "Game Review", None, s, None),
            text("Read-only review of an imported game.")
                .size(10.0 * s)
                .color(Palette::text_muted(theme)),
        ]
        .spacing(8);

        let board_area: Element<'_, GameReviewMessage> = engine_ui::build_board_eval_area(
            board,
            &self.es.analysis,
            self.es.current_eval_pct,
            s,
            self.settings.board_size,
            self.settings.show_eval_bar && self.es.analyzing,
        );

        let content = GameLayout::new(board_area, control_panel.into(), &self.settings)
            .with_info_panel(info_panel.into())
            .view();

        if self.pending_promotion.is_some() {
            let is_white = pos.turn().is_white();
            let promo = promotion_modal(
                theme,
                is_white,
                GameReviewMessage::PromoteTo,
                GameReviewMessage::CancelPromotion,
            );
            stack![content, promo]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            content
        }
    }

    fn navigate_home(&mut self) {
        self.go_to_depth(0);
        self.board.deselect();
        self.board.clear_animation();
    }

    fn navigate_end(&mut self) {
        let max = self.total_moves();
        self.go_to_depth(max);
        self.board.deselect();
        self.board.clear_animation();
    }

    fn instructions(&self) -> String {
        "Game Review — analyze an imported game.\n\n\
         • Navigate with arrow keys.\n\
         • Use Review Line to compute move quality badges.\n\
         • Toggle engine for live analysis."
            .to_string()
    }

    fn active_hotkeys(&self) -> Vec<(String, String)> {
        vec![("Left/Right".to_string(), "Navigate moves".to_string())]
    }
}

impl GameReviewMode {
    fn build_move_ribbon(&self, theme: &Theme) -> Element<'_, GameReviewMessage> {
        let mut ribbon_moves = Vec::new();
        let mut node = self.data.tree.root();
        let root_pos = node.position();
        let start_move_num = root_pos.fullmoves().get() as usize;
        let start_with_black = root_pos.turn().is_black();
        let mut is_white = !start_with_black;
        let mut depth = 0;

        while !node.children().is_empty() {
            depth += 1;
            let child = &node.children()[0];
            if let Some(san) = child.san() {
                let badge = self.data.review_results.get(&depth).map(|r| r.quality);

                ribbon_moves.push(move_ribbon::RibbonMove {
                    san: san.to_string(),
                    move_index: depth,
                    is_white,
                    has_note: false,
                    has_branch: false,
                    badge,
                });
                is_white = !is_white;
            }
            node = &node.children()[0];
        }

        let current_depth = self.current_depth;

        move_ribbon::build_ribbon(
            theme,
            ribbon_moves,
            current_depth,
            start_move_num,
            start_with_black,
            GameReviewMessage::GoToDepth,
            move |_| GameReviewMessage::None,
        )
    }

    fn build_review_summary(&self, theme: &Theme) -> Element<'_, GameReviewMessage> {
        let s = self.settings.ui_scale;

        if self.review_running {
            return sidebar::section(
                theme,
                "Review".to_string(),
                text("Analyzing moves with engine...")
                    .size(11.0 * s)
                    .color(Palette::text_muted(theme)),
                s,
            );
        }

        if let Some(err) = &self.review_error {
            return sidebar::section(
                theme,
                "Review".to_string(),
                text(err).size(11.0 * s).color(Palette::error(theme)),
                s,
            );
        }

        if let Some(r) = self.data.review_results.get(&self.current_depth) {
            let q_label = r.quality.label();
            let content = text(format!("{} ({:+.2})", q_label, -(r.loss_cp as f32) / 100.0))
                .size(12.0 * s)
                .color(review_assets::quality_color(theme, r.quality));
            return sidebar::section(theme, "Review".to_string(), content, s);
        }

        if self.data.review_results.is_empty() {
            sidebar::section(
                theme,
                "Review".to_string(),
                text("Run review to tag moves.")
                    .size(11.0 * s)
                    .color(Palette::text_muted(theme)),
                s,
            )
        } else {
            sidebar::section(
                theme,
                "Review".to_string(),
                text("Navigate to a reviewed move.")
                    .size(11.0 * s)
                    .color(Palette::text_muted(theme)),
                s,
            )
        }
    }
}
