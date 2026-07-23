use iced::Theme;
use iced::widget::{Space, button, column, container, stack, text};
use iced::{Element, Length, Subscription, Task, keyboard, time};
use shakmaty::Color as ChessColor;
use std::time::Duration as StdDuration;

use crate::core::config::AppSettings;
use crate::iced::pages::GameMode;
use crate::iced::panels::GameLayout;
use crate::iced::style::{self, Palette, buttons};
use crate::iced::widgets::board::{Board, BoardEvent, BoardMessage};
use crate::iced::widgets::common::{confirm_cancel_row, modal};
use crate::iced::widgets::move_ribbon;
use crate::iced::widgets::shake::Shake;
use crate::iced::widgets::sidebar;

// Import Core types
pub use crate::core::modes::trivia::TriviaMessage as CoreTriviaMessage;
use crate::core::modes::trivia::{TriviaEffect, TriviaGameStatus, TriviaState};
use iced::Border;
use std::time::Instant;

pub struct TriviaMode {
    pub state: TriviaState,
    pub(crate) board: Board,
    shake: Shake, // Shake animation state
    last_tick: Instant,
}

#[derive(Debug, Clone)]
pub enum TriviaMessage {
    Board(BoardMessage),
    Tick(Instant),
    AnimationTick,
    // Delegates to Core
    ShowHint,
    PlayCorrectMove,
    ShowSolution,
    RequestNewTrivia,
    ConfirmNewTrivia,
    CancelNewTrivia,
    KeyPressed(keyboard::Key, keyboard::Modifiers),
    AnalyzeOpening,
}

impl TriviaMode {
    pub fn new(settings: AppSettings) -> Self {
        let state = TriviaState::new(settings.clone());

        let mut board = Board::new();
        board.flipped = state.playing_as == ChessColor::Black;
        board.set_animation_speed(settings.animation_speed);
        board.set_theme(settings.board_theme);

        Self {
            state,
            board,
            shake: Shake::new(),
            last_tick: Instant::now(),
        }
    }

    pub fn is_completed(&self) -> bool {
        self.state.is_completed()
    }
}

impl GameMode for TriviaMode {
    type Message = TriviaMessage;

    fn set_settings(&mut self, settings: AppSettings) {
        self.board.set_animation_speed(settings.animation_speed);
        self.board.set_theme(settings.board_theme);
        self.state.set_settings(settings);
    }

    fn update(&mut self, message: TriviaMessage) -> Task<TriviaMessage> {
        // Internal update logic that returns internal messages
        // Since GameMode trait expects Task<Self::Message>, we adapt:
        // But wait, the app calls update() which returns Task<Message> (global).
        // The trait definition I wrote: fn update(&mut self, msg: Self::Message) -> Task<Self::Message>;
        // This is generic. The App wrapper maps it to global.

        match message {
            TriviaMessage::Tick(now) => {
                self.last_tick = now;
                self.shake.tick();
                Task::none()
            }
            TriviaMessage::AnimationTick => {
                self.board.tick();
                Task::none()
            }
            TriviaMessage::Board(msg) => {
                if self.state.status != TriviaGameStatus::AwaitingUserMove {
                    return Task::none();
                }
                if let Some(event) = self.board.update(&self.state.position, msg) {
                    match event {
                        BoardEvent::MoveMade(mv, _was_dragged) => {
                            if let Some(effect) =
                                self.state.update(CoreTriviaMessage::UserMoved(mv))
                            {
                                self.handle_effect(effect);
                            }
                        }
                        BoardEvent::MoveAttempted(_, _) => {
                            self.shake.trigger();
                        }
                        BoardEvent::SelectionChanged(_) => {
                            self.board.set_hint(None);
                        }
                        BoardEvent::PromotionRequired(_from, _to, candidates) => {
                            // Auto-promote if matches target
                            if let Some(target) = &self.state.target
                                && self.state.move_index < target.moves.len()
                            {
                                let expected = &target.moves[self.state.move_index];
                                if let Some(matching) = candidates.iter().find(|m| m == &expected)
                                    && let Some(effect) =
                                        self.state.update(CoreTriviaMessage::UserMoved(*matching))
                                {
                                    self.handle_effect(effect);
                                }
                            } else {
                                self.shake.trigger();
                            }
                        }
                        BoardEvent::NavigationChanged => {}
                    }
                }
                Task::none()
            }
            TriviaMessage::ShowHint => {
                if let Some(effect) = self.state.update(CoreTriviaMessage::ShowHint) {
                    self.handle_effect(effect);
                }
                Task::none()
            }
            TriviaMessage::ShowSolution => {
                if let Some(effect) = self.state.update(CoreTriviaMessage::PlayCorrectMove) {
                    self.handle_effect(effect);
                }
                Task::none()
            }
            TriviaMessage::PlayCorrectMove => {
                if let Some(effect) = self.state.update(CoreTriviaMessage::PlayCorrectMove) {
                    self.handle_effect(effect);
                }
                Task::none()
            }
            TriviaMessage::RequestNewTrivia => {
                if let Some(effect) = self.state.update(CoreTriviaMessage::RequestNewTrivia) {
                    self.handle_effect(effect);
                }
                Task::none()
            }
            TriviaMessage::ConfirmNewTrivia => {
                if let Some(effect) = self.state.update(CoreTriviaMessage::ConfirmNewTrivia) {
                    self.handle_effect(effect);
                }
                Task::none()
            }
            TriviaMessage::CancelNewTrivia => {
                if let Some(effect) = self.state.update(CoreTriviaMessage::CancelNewTrivia) {
                    self.handle_effect(effect);
                }
                Task::none()
            }
            TriviaMessage::KeyPressed(key, modifiers) => {
                // Ignore if Ctrl is pressed (let global hotkeys handle it)
                if modifiers.control() {
                    return Task::none();
                }

                match key.as_ref() {
                    keyboard::Key::Character("h") | keyboard::Key::Character("H") => {
                        return self.update(TriviaMessage::ShowHint);
                    }
                    keyboard::Key::Character("s") | keyboard::Key::Character("S") => {
                        return self.update(TriviaMessage::ShowSolution);
                    }
                    keyboard::Key::Character("n") | keyboard::Key::Character("N") => {
                        return self.update(TriviaMessage::RequestNewTrivia);
                    }
                    _ => {}
                }
                Task::none()
            }
            TriviaMessage::AnalyzeOpening => {
                // This is handled by the parent app to switch modes
                Task::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<TriviaMessage> {
        let mut subs = Vec::new();
        if self.shake.is_shaking() {
            subs.push(iced::window::frames().map(TriviaMessage::Tick));
        }
        if self.board.is_animating() {
            subs.push(
                time::every(StdDuration::from_millis(style::ANIMATION_TICK_MS))
                    .map(|_| TriviaMessage::AnimationTick),
            );
        }
        subs.push(iced::event::listen_with(
            |event, _status, _window| match event {
                iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                    Some(TriviaMessage::KeyPressed(key, modifiers))
                }
                _ => None,
            },
        ));
        Subscription::batch(subs)
    }

    fn view(&self, theme: &Theme) -> Element<'_, TriviaMessage> {
        let s = self.state.settings.ui_scale;
        let t = |v: f32| v * s;
        let board = self
            .board
            .view(&self.state.position, None, None, Length::Fill)
            .map(TriviaMessage::Board);

        let shake_padding = self.shake.apply();
        let board_container = container(board)
            .padding(shake_padding)
            .width(Length::Fill)
            .height(Length::Fill);

        let title_text = if let Some(target) = &self.state.target {
            format!("Opening: {}", target.name)
        } else {
            "Opening Trivia".to_string()
        };

        let header = sidebar::panel_header(theme, title_text, None, s, None);

        let feedback: Element<'_, TriviaMessage> =
            if self.state.status == TriviaGameStatus::LineComplete {
                container(
                    text(&self.state.last_feedback)
                        .size(t(16.0))
                        .color(Palette::background(theme)),
                )
                .padding([10, 16])
                .width(Length::Fill)
                .style(|theme| container::Style {
                    background: Some(iced::Background::Color(Palette::success(theme))),
                    border: Border {
                        radius: 8.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .into()
            } else if self.state.last_feedback.starts_with("Error")
                || self.state.last_feedback.starts_with("Warning")
            {
                text(&self.state.last_feedback)
                    .size(t(14.0))
                    .color(Palette::warning(theme))
                    .into()
            } else {
                Space::new().height(0).into()
            };

        let progress_section = if let Some(target) = &self.state.target {
            text(format!(
                "Progress: {} / {} moves",
                self.state.move_index,
                target.moves.len()
            ))
            .size(t(14.0))
            .color(Palette::text_secondary(theme))
        } else {
            text("").size(t(14.0))
        };

        let move_list: Element<'_, TriviaMessage> = if let Some(target) = &self.state.target {
            let moves_played = &target.moves[0..self.state.move_index];
            move_ribbon::build_linear_ribbon(
                theme,
                moves_played,
                self.state.move_index,
                |_| TriviaMessage::AnimationTick, // No-op
                |_| TriviaMessage::AnimationTick, // No-op
            )
        } else {
            text("").into()
        };

        let info_panel =
            column![sidebar::section(theme, "Moves".to_string(), move_list, s),].spacing(20);

        let buttons: Element<'_, TriviaMessage> = match &self.state.status {
            TriviaGameStatus::LineComplete => sidebar::action_row(
                vec![
                    button(text("Analyze").size(t(14.0)))
                        .padding(10)
                        .width(Length::Fill)
                        .style(buttons::secondary)
                        .on_press(TriviaMessage::AnalyzeOpening)
                        .into(),
                    button(text("New Trivia").size(t(14.0)))
                        .padding(10)
                        .width(Length::Fill)
                        .style(buttons::primary)
                        .on_press(TriviaMessage::RequestNewTrivia)
                        .into(),
                ],
                s,
            ),
            _ => {
                let hint_btn = if self.board.hint().is_some() {
                    button(text("Show Move").size(t(14.0)))
                        .padding(10)
                        .width(Length::Fill)
                        .style(buttons::secondary)
                        .on_press(TriviaMessage::PlayCorrectMove)
                } else {
                    button(text("Hint").size(t(14.0)))
                        .padding(10)
                        .width(Length::Fill)
                        .style(buttons::secondary)
                        .on_press(TriviaMessage::ShowHint)
                };

                sidebar::action_row(
                    vec![
                        hint_btn.into(),
                        button(text("New Trivia").size(t(14.0)))
                            .padding(10)
                            .width(Length::Fill)
                            .style(buttons::secondary)
                            .on_press(TriviaMessage::RequestNewTrivia)
                            .into(),
                    ],
                    s,
                )
            }
        };

        let control_panel = column![
            header,
            progress_section,
            feedback,
            Space::new().height(Length::Fill),
            buttons
        ]
        .spacing(20);

        let layout = GameLayout::new(
            board_container.into(),
            control_panel.into(),
            &self.state.settings,
        )
        .with_info_panel(info_panel.into());

        let main_content = layout.view();

        if self.state.show_confirm_new {
            let overlay = modal(
                column![
                    text("Leave Trivia?")
                        .size(18)
                        .color(Palette::text_primary(theme)),
                    text("Current progress will be lost.")
                        .size(14)
                        .color(Palette::text_muted(theme)),
                    confirm_cancel_row(
                        TriviaMessage::CancelNewTrivia,
                        TriviaMessage::ConfirmNewTrivia,
                        "Leave",
                        s,
                    )
                ]
                .spacing(12)
                .align_x(iced::Alignment::Center),
            );

            stack![main_content, overlay]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            main_content
        }
    }

    fn instructions(&self) -> String {
        "Test your opening memory.\n\n\
         • Goal: Play the correct moves for the selected opening line.\n\
         • Feedback: The board shakes on incorrect moves.\n\
         • Assistance: Use 'Hint' to highlight the target piece/square, or 'Show Move' if stuck.\n\
         • Progress: Complete the line to advance."
            .to_string()
    }

    fn active_hotkeys(&self) -> Vec<(String, String)> {
        vec![
            ("H".to_string(), "Show Hint".to_string()),
            ("S".to_string(), "Show Solution".to_string()),
            ("N".to_string(), "New Trivia".to_string()),
        ]
    }
}

impl TriviaMode {
    fn handle_effect(&mut self, effect: TriviaEffect) {
        match effect {
            TriviaEffect::SyncBoard(_) => {
                self.board.flipped = self.state.playing_as == ChessColor::Black;
                self.board.set_hint(None);
            }
            TriviaEffect::ShowHint(sq) => {
                self.board.set_hint(sq);
            }
        }
        if self.state.shake_trigger {
            self.shake.trigger();
        }
    }

    /// Returns the moves for cross-mode analysis
    pub fn get_opening_moves(&self) -> Vec<shakmaty::Move> {
        self.state
            .target
            .as_ref()
            .map(|t| t.moves.clone())
            .unwrap_or_default()
    }
}
