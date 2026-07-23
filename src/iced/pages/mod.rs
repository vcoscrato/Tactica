pub mod chessle;
pub mod game_review;
pub mod quick_board;
pub mod study;
pub mod trivia;

use crate::core::config::AppSettings;
use crate::iced::style;
use crate::iced::widgets::board::Board;
use iced::{Element, Subscription, Task, Theme};
use std::time::Duration;

// Import mode pages
use chessle::ChessleMode;
use game_review::GameReviewMode;
use quick_board::QuickBoardMode;
use study::StudyMode;
use trivia::TriviaMode;

/// Trait for game modes with common functionality
pub trait GameMode {
    type Message: Clone + std::fmt::Debug;

    /// Handle a message and return any follow-up tasks
    fn update(&mut self, msg: Self::Message) -> Task<Self::Message>;

    /// Render the mode's UI
    fn view(&self, theme: &Theme) -> Element<'_, Self::Message>;

    /// Return any subscriptions (keyboard, timers, etc.)
    fn subscription(&self) -> Subscription<Self::Message> {
        Subscription::none()
    }

    /// Update settings
    fn set_settings(&mut self, settings: AppSettings);

    /// Navigate to start of game/analysis (Home key)
    fn navigate_home(&mut self) {}

    /// Navigate to end of game/analysis (End key)
    fn navigate_end(&mut self) {}

    /// Check if the mode has unsaved progress or active game state
    fn has_pending_action(&self) -> bool {
        false
    }

    /// Get help instructions for this mode
    fn instructions(&self) -> String {
        String::new()
    }

    /// Get active hotkeys for this mode (Key, Description)
    fn active_hotkeys(&self) -> Vec<(String, String)> {
        Vec::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AnalysisMessage {
    PollEngine,
    Tick,
}

pub fn analysis_subscription(
    analyzing: bool,
    engine_running: bool,
    show_eval_bar: bool,
    board_animating: bool,
) -> Subscription<AnalysisMessage> {
    let mut subscriptions = Vec::new();

    if analyzing && engine_running {
        subscriptions.push(
            iced::time::every(Duration::from_millis(style::ENGINE_POLL_MS))
                .map(|_| AnalysisMessage::PollEngine),
        );
    }

    if board_animating || (analyzing && (show_eval_bar || !engine_running)) {
        subscriptions.push(
            iced::time::every(Duration::from_millis(style::ANIMATION_TICK_MS))
                .map(|_| AnalysisMessage::Tick),
        );
    }

    Subscription::batch(subscriptions)
}

/// Application modes
pub enum Mode {
    QuickBoard(Box<QuickBoardMode>),
    Study(Box<StudyMode>),
    GameReview(Box<GameReviewMode>),
    // Unchanged
    Trivia(TriviaMode),
    Chessle(ChessleMode),
}

impl Mode {
    pub fn update_settings(&mut self, settings: AppSettings) {
        match self {
            Mode::QuickBoard(m) => m.set_settings(settings),
            Mode::Study(m) => m.set_settings(settings),
            Mode::GameReview(m) => m.set_settings(settings),
            Mode::Trivia(m) => m.set_settings(settings),
            Mode::Chessle(m) => m.set_settings(settings),
        }
    }

    /// Returns a mutable reference to the board if this mode has one
    pub fn board_mut(&mut self) -> Option<&mut Board> {
        match self {
            Mode::QuickBoard(m) => Some(&mut m.board),
            Mode::Study(m) => Some(&mut m.board),
            Mode::GameReview(m) => Some(&mut m.board),
            Mode::Trivia(m) => Some(&mut m.board),
            Mode::Chessle(m) => Some(&mut m.board),
        }
    }

    /// Whether this mode has a board that supports user-initiated flipping
    pub fn has_board(&self) -> bool {
        matches!(
            self,
            Mode::QuickBoard(_)
                | Mode::Study(_)
                | Mode::GameReview(_)
                | Mode::Trivia(_)
                | Mode::Chessle(_)
        )
    }

    pub fn navigate_home(&mut self) {
        match self {
            Mode::QuickBoard(m) => m.navigate_home(),
            Mode::Study(m) => m.navigate_home(),
            Mode::GameReview(m) => m.navigate_home(),
            Mode::Chessle(m) => m.navigate_home(),
            _ => {}
        }
    }

    pub fn navigate_end(&mut self) {
        match self {
            Mode::QuickBoard(m) => m.navigate_end(),
            Mode::Study(m) => m.navigate_end(),
            Mode::GameReview(m) => m.navigate_end(),
            Mode::Chessle(m) => m.navigate_end(),
            _ => {}
        }
    }

    pub fn has_pending_action(&self) -> bool {
        match self {
            Mode::Study(m) => m.study.dirty,
            Mode::QuickBoard(_) => false,
            Mode::GameReview(m) => m.data.dirty,
            Mode::Trivia(m) => !m.is_completed(),
            Mode::Chessle(m) => !m.is_completed(),
        }
    }

    pub fn instructions(&self) -> String {
        match self {
            Mode::QuickBoard(m) => m.instructions(),
            Mode::Study(m) => m.instructions(),
            Mode::GameReview(m) => m.instructions(),
            Mode::Trivia(m) => m.instructions(),
            Mode::Chessle(m) => m.instructions(),
        }
    }

    pub fn active_hotkeys(&self) -> Vec<(String, String)> {
        match self {
            Mode::QuickBoard(m) => m.active_hotkeys(),
            Mode::Study(m) => m.active_hotkeys(),
            Mode::GameReview(m) => m.active_hotkeys(),
            Mode::Trivia(m) => m.active_hotkeys(),
            Mode::Chessle(m) => m.active_hotkeys(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ModeMessage {
    QuickBoard(quick_board::QuickBoardMessage),
    Study(study::StudyMessage),
    GameReview(game_review::GameReviewMessage),
    Trivia(trivia::TriviaMessage),
    Chessle(chessle::ChessleMessage),
}
