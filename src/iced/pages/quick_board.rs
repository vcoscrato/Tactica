//! Quick Board mode — a scratchpad for exploring positions.
//!
//! No persistence, no notes, no branching. Just a board and an engine.

use iced::Theme;
use iced::widget::{button, column, row, stack, svg, text, text_input};
use iced::{Element, Length, Subscription, Task, keyboard};
use shakmaty::{Chess, Move, Position, Role, Square};

use crate::core::config::AppSettings;
use crate::core::pgn;
use crate::core::review::MoveQuality;
use crate::iced::assets;
use crate::iced::pages::{AnalysisMessage, GameMode, analysis_subscription};
use crate::iced::panels::GameLayout;
use crate::iced::style::{Palette, buttons};
use crate::iced::widgets::board::{Board, BoardEvent, BoardMessage};
use crate::iced::widgets::common::{game_result_banner, promotion_modal};
use crate::iced::widgets::engine_ui::{self, EngineState};
use crate::iced::widgets::move_ribbon;
use crate::iced::widgets::sidebar;

/// Quick Board — ephemeral scratchpad with engine analysis.
pub struct QuickBoardMode {
    position: Chess,
    pub(crate) board: Board,
    history: Vec<(Chess, Move)>, // (position_before, move)
    es: EngineState,
    pub settings: AppSettings,
    pending_promotion: Option<(Square, Square, Vec<Move>)>,
    fen_input: String,
    fen_error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum QuickBoardMessage {
    Board(BoardMessage),
    ToggleAnalysis,
    PollEngine,
    PlayLine(usize),
    StepBackward,
    StepForward,
    GoToPly(usize),
    SaveAsStudy(Vec<Move>),
    ReviewLine(Vec<Move>),
    OpenEngineSettings,
    ResetBoard,
    FenInputChanged(String),
    ApplyFen,
    Tick,
    PromoteTo(Role),
    CancelPromotion,
    KeyPressed(keyboard::Key, keyboard::Modifiers),
    None,
}

impl QuickBoardMode {
    pub fn new(settings: AppSettings) -> Self {
        Self::from_position(Chess::default(), settings)
    }

    pub fn from_position(position: Chess, settings: AppSettings) -> Self {
        let analyzing = settings.engine.enabled;
        let es = EngineState::new(analyzing);
        Self {
            position,
            board: {
                let mut b = Board::new();
                b.set_animation_speed(settings.animation_speed);
                b.set_theme(settings.board_theme);
                b
            },
            history: Vec::new(),
            es,
            settings,
            pending_promotion: None,
            fen_input: String::new(),
            fen_error: None,
        }
    }

    pub fn shutdown(&mut self) {
        self.es.shutdown();
    }

    fn make_move(&mut self, mv: Move, animate: bool) {
        if let Ok(next) = self.position.clone().play(mv) {
            self.history.push((self.position.clone(), mv));
            self.position = next;
            if animate {
                self.board.animate_move(&mv, &self.position, false);
            }
            self.board.deselect();
            if self.es.analyzing {
                self.es
                    .start_with_settings(&self.settings.engine, &self.position);
            }
        }
    }

    fn go_back(&mut self) {
        if let Some((prev_pos, mv)) = self.history.pop() {
            self.position = prev_pos;
            self.board.animate_move(&mv, &self.position, true);
            self.board.deselect();
            if self.es.analyzing {
                self.es
                    .start_with_settings(&self.settings.engine, &self.position);
            }
        }
    }

    fn go_to_ply(&mut self, ply: usize) {
        let target = ply.min(self.history.len());
        while self.history.len() > target {
            self.go_back();
        }
    }
}

impl GameMode for QuickBoardMode {
    type Message = QuickBoardMessage;

    fn set_settings(&mut self, settings: AppSettings) {
        self.board.set_animation_speed(settings.animation_speed);
        self.board.set_theme(settings.board_theme);
        self.es.apply_settings(&settings, Some(&self.position));
        self.settings = settings;
    }

    fn update(&mut self, message: QuickBoardMessage) -> Task<QuickBoardMessage> {
        match message {
            QuickBoardMessage::Board(msg) => {
                if let Some(event) = self.board.update(&self.position, msg) {
                    match event {
                        BoardEvent::MoveMade(mv, was_dragged) => self.make_move(mv, !was_dragged),
                        BoardEvent::PromotionRequired(from, to, moves) => {
                            self.pending_promotion = Some((from, to, moves));
                        }
                        _ => {}
                    }
                }
            }
            QuickBoardMessage::ToggleAnalysis => {
                self.es.toggle(&mut self.settings, &self.position);
            }
            QuickBoardMessage::PollEngine => {
                self.es.poll();
            }
            QuickBoardMessage::PlayLine(i) => {
                if let Some(uci) = self.es.analysis.lines.get(i).and_then(|l| l.pv.first())
                    && let Some(mv) = engine_ui::parse_uci_move(&self.position, uci)
                {
                    self.make_move(mv, true);
                }
            }
            QuickBoardMessage::StepBackward => self.go_back(),
            QuickBoardMessage::StepForward => { /* no forward in quick board */ }
            QuickBoardMessage::GoToPly(ply) => self.go_to_ply(ply),
            QuickBoardMessage::SaveAsStudy(_) => {}
            QuickBoardMessage::ReviewLine(_) => {}
            QuickBoardMessage::OpenEngineSettings => {}
            QuickBoardMessage::ResetBoard => {
                self.position = Chess::default();
                self.history.clear();
                self.board.deselect();
                self.board.clear_animation();
                self.fen_error = None;
                if self.es.analyzing {
                    self.es
                        .start_with_settings(&self.settings.engine, &self.position);
                }
            }
            QuickBoardMessage::FenInputChanged(value) => {
                self.fen_input = value;
                self.fen_error = None;
            }
            QuickBoardMessage::ApplyFen => {
                let input = self.fen_input.trim();
                if pgn::looks_like_fen(input) {
                    match pgn::parse_fen(input) {
                        Ok(position) => {
                            self.position = position;
                            self.history.clear();
                            self.board.deselect();
                            self.board.clear_animation();
                            self.fen_error = None;
                            if self.es.analyzing {
                                self.es
                                    .start_with_settings(&self.settings.engine, &self.position);
                            }
                        }
                        Err(e) => self.fen_error = Some(e),
                    }
                } else {
                    match pgn::parse_pgn_detailed(input) {
                        Ok(parsed) => {
                            let warning = parsed.warning_summary();
                            let tree = parsed.tree;
                            let mut pos = tree.root().position().clone();
                            let mut history = Vec::new();
                            let mut node = tree.root();
                            while !node.children().is_empty() {
                                let child = &node.children()[0];
                                if let Some(mv) = child.mv() {
                                    history.push((pos.clone(), *mv));
                                    if let Ok(next) = pos.clone().play(*mv) {
                                        pos = next;
                                    }
                                }
                                node = child;
                            }
                            self.position = pos;
                            self.history = history;
                            self.board.deselect();
                            self.board.clear_animation();
                            self.fen_error = warning;
                            if self.es.analyzing {
                                self.es
                                    .start_with_settings(&self.settings.engine, &self.position);
                            }
                        }
                        Err(e) => self.fen_error = Some(e),
                    }
                }
            }
            QuickBoardMessage::Tick => {
                self.es
                    .ensure_running(&self.settings.engine, &self.position);
                self.board.tick();
                self.es.tick_eval_bar(self.settings.show_eval_bar);
            }
            QuickBoardMessage::PromoteTo(role) => {
                if let Some((_from, _to, moves)) = self.pending_promotion.take()
                    && let Some(mv) = moves.iter().find(|m| m.promotion() == Some(role)).cloned()
                {
                    self.make_move(mv, true);
                }
            }
            QuickBoardMessage::CancelPromotion => {
                self.pending_promotion = None;
            }
            QuickBoardMessage::KeyPressed(key, modifiers) => {
                if modifiers.control() {
                    return Task::none();
                }
                if let keyboard::Key::Named(keyboard::key::Named::ArrowLeft) = key.as_ref() {
                    self.go_back();
                }
            }
            QuickBoardMessage::None => {}
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<QuickBoardMessage> {
        let mut subs = vec![iced::event::listen_with(
            |event, _status, _window| match event {
                iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                    Some(QuickBoardMessage::KeyPressed(key, modifiers))
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
                AnalysisMessage::PollEngine => QuickBoardMessage::PollEngine,
                AnalysisMessage::Tick => QuickBoardMessage::Tick,
            }),
        );
        Subscription::batch(subs)
    }

    fn view(&self, theme: &Theme) -> Element<'_, QuickBoardMessage> {
        let s = self.settings.ui_scale;

        let board = self
            .board
            .view(
                &self.position,
                self.history.last().map(|(_, m)| m),
                None,
                Length::Fixed(self.settings.board_size),
            )
            .map(QuickBoardMessage::Board);

        let reset_btn = button(text("Reset").size(11.0 * s))
            .padding([4, 8])
            .style(buttons::secondary)
            .on_press(QuickBoardMessage::ResetBoard);

        let review_icon = svg(iced::widget::svg::Handle::from_memory(
            crate::iced::widgets::review_assets::icon_bytes(MoveQuality::Best),
        ))
        .width(Length::Fixed(12.0))
        .height(Length::Fixed(12.0))
        .style(|theme, _| iced::widget::svg::Style {
            color: Some(Palette::text_primary(theme)),
        });
        let review_btn = if self.history.is_empty() {
            button(
                row![review_icon, text("Review position").size(11.0 * s)]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
            )
            .padding([4, 8])
            .style(buttons::secondary)
        } else {
            let moves = self.history.iter().map(|(_, mv)| *mv).collect();
            button(
                row![review_icon, text("Review position").size(11.0 * s)]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
            )
            .padding([4, 8])
            .style(buttons::primary)
            .on_press(QuickBoardMessage::ReviewLine(moves))
        };

        let study_icon = svg(assets::icon("branch"))
            .width(Length::Fixed(12.0))
            .height(Length::Fixed(12.0))
            .style(|theme, _| iced::widget::svg::Style {
                color: Some(Palette::text_primary(theme)),
            });
        let study_btn = if self.history.is_empty() {
            button(
                row![study_icon, text("Study position").size(11.0 * s)]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
            )
            .padding([4, 8])
            .style(buttons::secondary)
        } else {
            let moves = self.history.iter().map(|(_, mv)| *mv).collect();
            button(
                row![study_icon, text("Study position").size(11.0 * s)]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
            )
            .padding([4, 8])
            .style(buttons::primary)
            .on_press(QuickBoardMessage::SaveAsStudy(moves))
        };

        let header = row![reset_btn, review_btn, study_btn]
            .spacing(6)
            .align_y(iced::Alignment::Center);

        let engine_controls = engine_ui::engine_controls_row(
            theme,
            engine_ui::EngineControlsState {
                ui_scale: s,
                analyzing: self.es.analyzing,
                current_depth: self.es.analysis.depth,
                max_depth: self.settings.engine.max_depth,
                error: self.es.error.as_deref(),
            },
            QuickBoardMessage::ToggleAnalysis,
            QuickBoardMessage::OpenEngineSettings,
        );

        let lines = engine_ui::build_engine_lines(
            theme,
            s,
            &self.position,
            &self.es.analysis,
            QuickBoardMessage::PlayLine,
        );

        let engine_content: Element<'_, QuickBoardMessage> = if self.es.analyzing {
            column![engine_controls, lines].spacing(8).into()
        } else {
            engine_controls
        };

        let moves: Vec<Move> = self.history.iter().map(|(_, mv)| *mv).collect();
        let ribbon = move_ribbon::build_linear_ribbon(
            theme,
            &moves,
            self.history.len(),
            QuickBoardMessage::GoToPly,
            |_| QuickBoardMessage::None,
        );

        let mut info_panel = column![header].spacing(10.0 * s);
        if let Some(result) = game_result_banner(theme, &self.position, s) {
            info_panel = info_panel.push(result);
        }
        info_panel = info_panel
            .push(iced::widget::rule::horizontal(1))
            .push(engine_content)
            .push(iced::widget::rule::horizontal(1))
            .push(sidebar::section(theme, "Moves".to_string(), ribbon, s));

        let control_panel = column![
            sidebar::panel_header(theme, "Quick Board", None, s, None),
            text_input("Paste FEN or PGN...", &self.fen_input)
                .size(11.0 * s)
                .padding(6)
                .on_input(QuickBoardMessage::FenInputChanged)
                .on_submit(QuickBoardMessage::ApplyFen),
            button(text("Set Position").size(11.0 * s))
                .padding([4, 8])
                .style(buttons::secondary)
                .on_press(QuickBoardMessage::ApplyFen),
            if let Some(err) = &self.fen_error {
                text(err).size(10.0 * s).color(Palette::error(theme))
            } else {
                text("")
            },
        ]
        .spacing(8);

        let board_area: Element<'_, QuickBoardMessage> = engine_ui::build_board_eval_area(
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
            let is_white = self.position.turn().is_white();
            let promo = promotion_modal(
                theme,
                is_white,
                QuickBoardMessage::PromoteTo,
                QuickBoardMessage::CancelPromotion,
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
        while !self.history.is_empty() {
            self.history.pop();
        }
        self.position = Chess::default();
        self.board.deselect();
        self.board.clear_animation();
        if self.es.analyzing {
            self.es
                .start_with_settings(&self.settings.engine, &self.position);
        }
    }

    fn navigate_end(&mut self) {
        // No-op for quick board
    }

    fn instructions(&self) -> String {
        "Quick Board — explore positions quickly.\n\n\
         • Play moves freely.\n\
         • Use the engine toggle to analyze positions.\n\
         • Click engine lines to play them."
            .to_string()
    }

    fn active_hotkeys(&self) -> Vec<(String, String)> {
        vec![("Left".to_string(), "Take back move".to_string())]
    }
}
