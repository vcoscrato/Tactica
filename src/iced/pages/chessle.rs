//! Chessle - Wordle-style opening guessing game
//!
//! Guess the opening sequence with green/yellow/gray feedback.
//! Configurable difficulty (3-6 moves) and max guesses (3-10).

use iced::Theme;
use iced::widget::{Space, button, column, container, row, scrollable, slider, text};
use iced::{Background, Border, Element, Length, Subscription, Task, keyboard};
use shakmaty::{Chess, Move, Position, Role, san::San};

use crate::core::config::AppSettings;
use crate::iced::pages::GameMode;
use crate::iced::panels::GameLayout;
use crate::iced::style::{Palette, buttons, containers};
use crate::iced::widgets::board::{Board, BoardEvent, BoardMessage};
use crate::iced::widgets::common::icon_button;
use crate::iced::widgets::sidebar;
use crate::iced::widgets::toast::ToastType;
// Core imports
pub use crate::core::modes::chessle::{
    ChessleMessage as CoreChessleMessage, ChessleState, ChessleStatus, Guess, MoveFeedback,
};

pub struct ChessleMode {
    pub state: ChessleState,
    pub(crate) board: Board, // UI Board
    pub confirm_new_game: bool,
}

#[derive(Debug, Clone)]
pub enum ChessleMessage {
    Board(BoardMessage),
    SetMoveCount(usize),
    SetMaxGuesses(usize),
    StartGame,
    RequestNewGame,
    ConfirmNewGame,
    CancelNewGame,
    SubmitGuess,
    UndoMove,
    NewGame,
    AutoFill,
    KeyPressed(keyboard::Key, keyboard::Modifiers),
    AnalyzeOpening,
    Notify(ToastType, String),
}

impl ChessleMode {
    pub fn is_completed(&self) -> bool {
        self.state.status == ChessleStatus::Won || self.state.status == ChessleStatus::Lost
    }

    pub fn new(settings: AppSettings) -> Self {
        let state = ChessleState::new(settings.clone());
        let mut board = Board::new();
        board.set_animation_speed(settings.animation_speed);
        board.set_theme(settings.board_theme);

        Self {
            state,
            board,
            confirm_new_game: false,
        }
    }
}

impl GameMode for ChessleMode {
    type Message = ChessleMessage;

    fn set_settings(&mut self, settings: AppSettings) {
        self.board.set_animation_speed(settings.animation_speed);
        self.board.set_theme(settings.board_theme);
        self.state.settings = settings;
    }

    fn update(&mut self, message: ChessleMessage) -> Task<ChessleMessage> {
        match message {
            ChessleMessage::Board(msg) => {
                if self.state.status != ChessleStatus::Playing {
                    return Task::none();
                }

                match self.board.update(&self.state.current_position, msg) {
                    Some(BoardEvent::MoveMade(mv, _was_dragged)) => {
                        self.state.update(CoreChessleMessage::UserMove(mv));
                    }
                    Some(BoardEvent::PromotionRequired(_, _, moves)) => {
                        // Auto-promote to Queen for simplicity in guessing game
                        if let Some(mv) = moves
                            .iter()
                            .find(|m: &&Move| m.promotion() == Some(Role::Queen))
                        {
                            self.state.update(CoreChessleMessage::UserMove(*mv));
                        }
                    }
                    _ => {}
                }
            }

            ChessleMessage::SetMoveCount(count) => {
                self.state.update(CoreChessleMessage::SetMoveCount(count));
            }

            ChessleMessage::SetMaxGuesses(count) => {
                self.state.update(CoreChessleMessage::SetMaxGuesses(count));
            }

            ChessleMessage::StartGame | ChessleMessage::ConfirmNewGame => {
                if !self.state.opening_names.is_loaded() {
                    return Task::done(ChessleMessage::Notify(
                        ToastType::Error,
                        "Opening Names are missing. Open the Assets menu to download them."
                            .to_string(),
                    ));
                }
                self.confirm_new_game = false;
                self.board.set_flipped(false);
                self.state.update(CoreChessleMessage::StartGame);

                if self.state.status != ChessleStatus::Playing {
                    // Failed to start (no openings found)
                    return Task::done(ChessleMessage::Notify(
                        ToastType::Error,
                        "No openings found for this difficulty.".to_string(),
                    ));
                }
            }

            ChessleMessage::RequestNewGame => {
                if self.state.status == ChessleStatus::Playing {
                    self.confirm_new_game = true;
                } else {
                    return Task::done(ChessleMessage::StartGame);
                }
            }

            ChessleMessage::CancelNewGame => {
                self.confirm_new_game = false;
            }

            ChessleMessage::NewGame => {
                return Task::done(ChessleMessage::RequestNewGame);
            }

            ChessleMessage::SubmitGuess => {
                let total_moves = self.state.move_count * 2;
                if self.state.current_guess_moves.len() != total_moves {
                    return Task::done(ChessleMessage::Notify(
                        ToastType::Info,
                        format!("Need {} moves to submit.", total_moves),
                    ));
                }

                self.state.update(CoreChessleMessage::Submit);

                // Check result for toast
                if self.state.status == ChessleStatus::Won {
                    return Task::done(ChessleMessage::Notify(
                        ToastType::Success,
                        format!(
                            "Correct! {}",
                            self.state
                                .target
                                .as_ref()
                                .map(|o| o.name.as_str())
                                .unwrap_or("")
                        ),
                    ));
                } else if self.state.status == ChessleStatus::Lost {
                    return Task::done(ChessleMessage::Notify(
                        ToastType::Error,
                        format!(
                            "Game Over: {}",
                            self.state
                                .target
                                .as_ref()
                                .map(|o| o.name.as_str())
                                .unwrap_or("")
                        ),
                    ));
                }
            }

            ChessleMessage::UndoMove => {
                if self.state.current_guess_moves.is_empty() {
                    return Task::done(ChessleMessage::Notify(
                        ToastType::Info,
                        "Nothing to undo.".to_string(),
                    ));
                }
                self.state.update(CoreChessleMessage::Undo);
            }

            ChessleMessage::AutoFill => {
                let prev_len = self.state.current_guess_moves.len();
                self.state.update(CoreChessleMessage::AutoFill);
                let new_len = self.state.current_guess_moves.len();

                if new_len > prev_len {
                    return Task::done(ChessleMessage::Notify(
                        ToastType::Success,
                        format!("Filled {} moves", new_len - prev_len),
                    ));
                } else {
                    return Task::done(ChessleMessage::Notify(
                        ToastType::Info,
                        "No known moves to fill".to_string(),
                    ));
                }
            }
            ChessleMessage::KeyPressed(key, modifiers) => {
                if modifiers.control() {
                    return Task::none();
                }
                match key.as_ref() {
                    keyboard::Key::Named(keyboard::key::Named::Enter) => {
                        return Task::done(ChessleMessage::SubmitGuess);
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                        return Task::done(ChessleMessage::UndoMove);
                    }
                    keyboard::Key::Character("a") | keyboard::Key::Character("A") => {
                        return Task::done(ChessleMessage::AutoFill);
                    }
                    _ => {}
                }
            }
            ChessleMessage::AnalyzeOpening => {
                // Handled by parent app
            }
            ChessleMessage::Notify(_, _) => {
                // Handled by parent app
            }
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<ChessleMessage> {
        iced::event::listen_with(|event, _status, _window| match event {
            iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                Some(ChessleMessage::KeyPressed(key, modifiers))
            }
            _ => None,
        })
    }

    fn view(&self, theme: &Theme) -> Element<'_, ChessleMessage> {
        self.view_game(theme)
    }

    fn navigate_home(&mut self) {
        if self.state.status == ChessleStatus::Playing {
            self.state.current_guess_moves.clear();
            self.state.current_position = Chess::default();
        }
    }

    fn navigate_end(&mut self) {}

    fn instructions(&self) -> String {
        "Wordle-style opening guessing game.\n\n\
         • Objective: Guess the exact opening sequence.\n\
         • Feedback:\n  - Green: Correct move in correct slot.\n  - Yellow: Correct move in wrong slot.\n  - Gray: Move not in sequence.\n\
         • Controls: Play moves on board, then click Submit. Use Undo to correct your guess before submitting."
            .to_string()
    }

    fn active_hotkeys(&self) -> Vec<(String, String)> {
        vec![
            ("Enter".to_string(), "Submit Guess".to_string()),
            ("Arrow Left".to_string(), "Undo Move".to_string()),
            ("A".to_string(), "Auto-fill".to_string()),
        ]
    }
}

impl ChessleMode {
    fn view_game(&self, theme: &Theme) -> Element<'_, ChessleMessage> {
        let s = self.state.settings.ui_scale;
        let t = |v: f32| v * s;
        let board = self
            .board
            .view(&self.state.current_position, None, None, Length::Fill)
            .map(ChessleMessage::Board);

        let board_container = container(board).width(Length::Fill).height(Length::Fill);

        // Guesses list (History)
        let mut guess_items: Vec<Element<'_, ChessleMessage>> = self
            .state
            .guesses
            .iter()
            .enumerate()
            .map(|(i, guess)| self.render_guess(theme, i + 1, guess))
            .collect();

        // Append current pending guess if playing and not empty
        if self.state.status == ChessleStatus::Playing && !self.state.current_guess_moves.is_empty()
        {
            guess_items.push(self.render_pending_guess(
                theme,
                self.state.guesses.len() + 1,
                &self.state.current_guess_moves,
            ));
        }

        let guess_history: Element<'_, ChessleMessage> = if guess_items.is_empty() {
            container(
                text("Make moves on the board to start guessing!")
                    .size(t(12.0))
                    .color(Palette::text_muted(theme)),
            )
            .into()
        } else {
            scrollable(column(guess_items).spacing(5))
                .height(Length::Fill)
                .into()
        };

        // Configuration Section (Always visible)
        let config_section = column![
            text("Difficulty")
                .size(t(12.0))
                .color(Palette::text_secondary(theme)),
            row![
                text("Moves").size(t(12.0)),
                slider(3..=6, self.state.move_count as i32, |v| {
                    ChessleMessage::SetMoveCount(v as usize)
                })
                .width(Length::Fill),
                text(format!("{}", self.state.move_count)).size(t(12.0)),
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
            row![
                text("Guesses").size(t(12.0)),
                slider(3..=10, self.state.max_guesses as i32, |v| {
                    ChessleMessage::SetMaxGuesses(v as usize)
                })
                .width(Length::Fill),
                text(format!("{}", self.state.max_guesses)).size(t(12.0)),
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(10);

        // Status Text
        let status_text = match self.state.status {
            ChessleStatus::Setup => "Ready to Start",
            ChessleStatus::Playing => "Playing...",
            ChessleStatus::Won => "You Won!",
            ChessleStatus::Lost => "Game Over",
        };
        let status_color = match self.state.status {
            ChessleStatus::Won => Palette::success(theme),
            ChessleStatus::Lost => Palette::warning(theme),
            _ => Palette::text_primary(theme),
        };
        let status_display = text(status_text)
            .size(t(16.0))
            .color(status_color)
            .width(Length::Fill)
            .align_x(iced::Alignment::Center);

        // Action Buttons
        let buttons: Element<'_, ChessleMessage> = if self.confirm_new_game {
            row![
                button(text("Cancel").size(t(12.0)))
                    .padding(8)
                    .width(Length::Fill)
                    .style(buttons::secondary)
                    .on_press(ChessleMessage::CancelNewGame),
                button(text("Confirm").size(t(12.0)))
                    .padding(8)
                    .width(Length::Fill)
                    .style(buttons::danger)
                    .on_press(ChessleMessage::ConfirmNewGame),
            ]
            .spacing(10)
            .into()
        } else {
            let mut col = column![].spacing(8);

            if self.state.status == ChessleStatus::Playing {
                col = col.push(
                    row![
                        button(text("Auto-fill").size(t(12.0)))
                            .padding(8)
                            .width(Length::Fill)
                            .style(buttons::secondary)
                            .on_press(ChessleMessage::AutoFill),
                        icon_button("arrow-left.svg", ChessleMessage::UndoMove).width(Length::Fill),
                    ]
                    .spacing(8),
                );
                col = col.push(
                    button(text("Submit Guess").size(t(12.0)))
                        .padding(8)
                        .width(Length::Fill)
                        .style(buttons::primary)
                        .on_press(ChessleMessage::SubmitGuess),
                );
            }

            if self.state.status == ChessleStatus::Won || self.state.status == ChessleStatus::Lost {
                col = col.push(
                    button(text("Analyze Game").size(t(12.0)))
                        .padding(8)
                        .width(Length::Fill)
                        .style(buttons::secondary)
                        .on_press(ChessleMessage::AnalyzeOpening),
                );
            }

            col = col.push(
                button(text("New Game").size(t(12.0)))
                    .padding(8)
                    .width(Length::Fill)
                    .style(if self.state.status == ChessleStatus::Playing {
                        buttons::secondary
                    } else {
                        buttons::primary
                    })
                    .on_press(ChessleMessage::NewGame),
            );

            col.into()
        };

        // Opening Info (Reveal on End)
        let opening_info: Element<'_, ChessleMessage> = match self.state.status {
            ChessleStatus::Won | ChessleStatus::Lost => {
                if let Some(ref target) = self.state.target {
                    let correct_moves_str = self.format_moves(&self.state.target_moves);
                    container(
                        column![
                            text(format!("Opening: {}", target.name))
                                .size(t(14.0))
                                .color(Palette::accent(theme)),
                            text(format!("Answer: {}", correct_moves_str))
                                .size(t(11.0))
                                .color(Palette::text_secondary(theme)),
                        ]
                        .spacing(4),
                    )
                    .padding([8, 12])
                    .style(containers::panel(6.0))
                    .into()
                } else {
                    Space::new().height(0).into()
                }
            }
            _ => Space::new().height(0).into(),
        };

        let right_sidebar = column![
            sidebar::panel_header(theme, "Chessle", None, s, None),
            config_section,
            Space::new().height(10),
            status_display,
            Space::new().height(10),
            opening_info,
            Space::new().height(20),
            buttons,
        ]
        .spacing(5);

        let info_panel = column![sidebar::section(
            theme,
            "Your Guesses".into(),
            guess_history,
            s
        )]
        .spacing(5);

        GameLayout::new(
            board_container.into(),
            right_sidebar.into(),
            &self.state.settings,
        )
        .with_info_panel(info_panel.into())
        .view()
    }

    fn render_pending_guess(
        &self,
        theme: &Theme,
        num: usize,
        moves: &[Move],
    ) -> Element<'_, ChessleMessage> {
        let s = self.state.settings.ui_scale;
        let t = |v: f32| v * s;
        let mut pair_groups: Vec<Element<'_, ChessleMessage>> = Vec::new();
        let mut i = 0;
        let mut move_num = 1;

        // Create a temporary guess object to reuse move_to_san logic
        let dummy_guess = Guess {
            moves: moves.to_vec(),
            feedback: vec![],
        };

        while i < moves.len() {
            let mut pair_items: Vec<Element<'_, ChessleMessage>> = Vec::new();

            pair_items.push(
                text(format!("{}.", move_num))
                    .size(t(9.0))
                    .color(Palette::text_muted(theme))
                    .into(),
            );

            if i < moves.len() {
                let san = self.move_to_san(&dummy_guess, i, &moves[i]);
                pair_items.push(
                    container(text(san).size(t(10.0)).color(Palette::text_primary(theme)))
                        .padding([2, 4])
                        .style(|theme| container::Style {
                            background: Some(Background::Color(Palette::panel(theme))),
                            border: Border {
                                radius: 3.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .into(),
                );
                i += 1;
            }

            if i < moves.len() {
                let san = self.move_to_san(&dummy_guess, i, &moves[i]);
                pair_items.push(
                    container(text(san).size(t(10.0)).color(Palette::text_primary(theme)))
                        .padding([2, 4])
                        .style(|theme| container::Style {
                            background: Some(Background::Color(Palette::panel(theme))),
                            border: Border {
                                radius: 3.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .into(),
                );
                i += 1;
            }

            pair_groups.push(
                container(row(pair_items).spacing(2))
                    .padding([2, 4])
                    .style(|theme| container::Style {
                        background: Some(Background::Color(Palette::background(theme))),
                        border: Border {
                            radius: 4.0.into(),
                            color: Palette::border(theme),
                            width: 1.0,
                        },
                        ..Default::default()
                    })
                    .into(),
            );
            move_num += 1;
        }

        row![
            text(format!("{}.", num))
                .size(t(10.0))
                .color(Palette::text_muted(theme))
                .width(Length::Fixed(20.0)),
            row(pair_groups).spacing(4).wrap(),
        ]
        .spacing(5)
        .into()
    }

    fn render_guess(
        &self,
        theme: &Theme,
        num: usize,
        guess: &Guess,
    ) -> Element<'_, ChessleMessage> {
        let s = self.state.settings.ui_scale;
        let t = |v: f32| v * s;
        let mut pair_groups: Vec<Element<'_, ChessleMessage>> = Vec::new();
        let mut i = 0;
        let mut move_num = 1;

        while i < guess.moves.len() {
            let mut pair_items: Vec<Element<'_, ChessleMessage>> = Vec::new();

            pair_items.push(
                text(format!("{}.", move_num))
                    .size(t(9.0))
                    .color(Palette::text_muted(theme))
                    .into(),
            );

            if i < guess.moves.len() {
                let san = self.move_to_san(guess, i, &guess.moves[i]);
                let (bg, text_color) = match guess.feedback[i] {
                    MoveFeedback::Correct => (Palette::success(theme), Palette::background(theme)),
                    MoveFeedback::WrongPos => (Palette::warning(theme), Palette::background(theme)),
                    MoveFeedback::Wrong => (Palette::surface(theme), Palette::text_muted(theme)),
                };
                pair_items.push(
                    container(text(san).size(t(10.0)).color(text_color))
                        .padding([2, 4])
                        .style(move |_| container::Style {
                            background: Some(Background::Color(bg)),
                            border: Border {
                                radius: 3.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .into(),
                );
                i += 1;
            }

            if i < guess.moves.len() {
                let san = self.move_to_san(guess, i, &guess.moves[i]);
                let (bg, text_color) = match guess.feedback[i] {
                    MoveFeedback::Correct => (Palette::success(theme), Palette::background(theme)),
                    MoveFeedback::WrongPos => (Palette::warning(theme), Palette::background(theme)),
                    MoveFeedback::Wrong => (Palette::surface(theme), Palette::text_muted(theme)),
                };
                pair_items.push(
                    container(text(san).size(t(10.0)).color(text_color))
                        .padding([2, 4])
                        .style(move |_| container::Style {
                            background: Some(Background::Color(bg)),
                            border: Border {
                                radius: 3.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .into(),
                );
                i += 1;
            }

            pair_groups.push(
                container(row(pair_items).spacing(2))
                    .padding([2, 4])
                    .style(|theme| container::Style {
                        background: Some(Background::Color(Palette::background(theme))),
                        border: Border {
                            radius: 4.0.into(),
                            color: Palette::border(theme),
                            width: 1.0,
                        },
                        ..Default::default()
                    })
                    .into(),
            );
            move_num += 1;
        }

        row![
            text(format!("{}.", num))
                .size(t(10.0))
                .color(Palette::text_muted(theme))
                .width(Length::Fixed(20.0)),
            row(pair_groups).spacing(4).wrap(),
        ]
        .spacing(5)
        .into()
    }
}

impl ChessleMode {
    fn format_moves(&self, moves: &[Move]) -> String {
        let mut result = String::new();
        let mut pos = Chess::default();

        for (i, mv) in moves.iter().enumerate() {
            if i % 2 == 0 {
                if i > 0 {
                    result.push(' ');
                }
                result.push_str(&format!("{}.", i / 2 + 1));
            }
            result.push(' ');
            result.push_str(&San::from_move(&pos, *mv).to_string());
            pos = pos.clone().play(*mv).expect("Format replay failed");
        }

        result
    }

    fn move_to_san(&self, guess: &Guess, index: usize, mv: &Move) -> String {
        let mut pos = Chess::default();
        for (i, guess_mv) in guess.moves.iter().enumerate() {
            if i == index {
                break;
            }
            pos = pos.clone().play(*guess_mv).ok().unwrap_or(pos);
        }
        San::from_move(&pos, *mv).to_string()
    }

    /// Returns the target moves for cross-mode analysis
    pub fn get_opening_moves(&self) -> Vec<Move> {
        self.state.target_moves.clone()
    }
}
