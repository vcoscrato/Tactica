use crate::core::config::AppSettings;
use crate::core::openings::{Opening, OpeningNames};
use shakmaty::{Chess, Color, Move, Position};

#[derive(Debug, Clone, PartialEq)]
pub enum TriviaGameStatus {
    Idle,
    AwaitingUserMove,
    ShowingCorrect,
    LineComplete,
}

#[derive(Debug, Clone)]
pub struct TriviaState {
    pub position: Chess,
    pub playing_as: Color,
    pub opening_names: OpeningNames,
    pub target: Option<Opening>,
    pub move_index: usize,
    pub status: TriviaGameStatus,
    pub last_feedback: String,
    pub settings: AppSettings,
    pub show_confirm_new: bool,
    pub shake_trigger: bool,
}

impl TriviaState {
    pub fn new(settings: AppSettings) -> Self {
        let opening_names = OpeningNames::new();
        let loaded = opening_names.is_loaded();

        let mut trivia = Self {
            position: Chess::default(),
            playing_as: Color::White,
            opening_names,
            target: None,
            move_index: 0,
            status: TriviaGameStatus::Idle,
            last_feedback: if loaded {
                "Welcome to Opening Trivia! Press 'Start Trivia' to begin.".to_string()
            } else {
                "Warning: Opening Names are missing. Open the Assets menu to download them."
                    .to_string()
            },
            settings,
            show_confirm_new: false,
            shake_trigger: false,
        };

        if loaded {
            trivia.start_new_trivia();
        }

        trivia
    }

    pub fn set_settings(&mut self, settings: AppSettings) {
        self.settings = settings;
    }

    pub fn start_new_trivia(&mut self) {
        if let Some(opening) = self.opening_names.get_random_opening() {
            self.target = Some(opening.clone());
            self.position = Chess::default();
            self.move_index = 0;
            self.status = TriviaGameStatus::AwaitingUserMove;

            let target_len = opening.moves.len();
            if target_len > 0 {
                self.playing_as = if target_len % 2 != 0 {
                    Color::White
                } else {
                    Color::Black
                };
            } else {
                self.playing_as = Color::White;
            }

            self.last_feedback = String::new();

            if self.playing_as == Color::Black {
                self.make_opponent_move();
            }
        }
    }

    pub fn update(&mut self, message: TriviaMessage) -> Option<TriviaEffect> {
        self.shake_trigger = false;

        match message {
            TriviaMessage::UserMoved(mv) => self.handle_user_move(mv),
            TriviaMessage::ShowHint => {
                if let Some(target) = &self.target
                    && self.move_index < target.moves.len()
                {
                    let expected = &target.moves[self.move_index];
                    Some(TriviaEffect::ShowHint(expected.from()))
                } else {
                    None
                }
            }
            TriviaMessage::PlayCorrectMove => {
                if let Some(target) = &self.target
                    && self.move_index < target.moves.len()
                {
                    let expected_move = target.moves[self.move_index];
                    if self.position.is_legal(expected_move) {
                        self.position = self
                            .position
                            .clone()
                            .play(expected_move)
                            .expect("Valid move");
                        // Pass to handle_user_move to check completion logic, but skip legality/matching check since we know it's correct
                        self.move_index += 1;

                        if self.move_index >= target.moves.len() {
                            self.status = TriviaGameStatus::LineComplete;
                            self.last_feedback = format!("Success! You completed {}.", target.name);
                        } else {
                            self.status = TriviaGameStatus::ShowingCorrect;
                            self.make_opponent_move();
                        }
                        Some(TriviaEffect::SyncBoard(self.position.clone()))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            TriviaMessage::RequestNewTrivia => {
                if self.status == TriviaGameStatus::AwaitingUserMove
                    || self.status == TriviaGameStatus::ShowingCorrect
                {
                    self.show_confirm_new = true;
                    None
                } else {
                    self.start_new_trivia();
                    Some(TriviaEffect::SyncBoard(self.position.clone()))
                }
            }
            TriviaMessage::ConfirmNewTrivia => {
                self.show_confirm_new = false;
                self.start_new_trivia();
                Some(TriviaEffect::SyncBoard(self.position.clone()))
            }
            TriviaMessage::CancelNewTrivia => {
                self.show_confirm_new = false;
                None
            }
        }
    }

    fn handle_user_move(&mut self, mv: Move) -> Option<TriviaEffect> {
        let Some(target) = &self.target else {
            return None;
        };

        if self.move_index >= target.moves.len() {
            self.status = TriviaGameStatus::LineComplete;
            return Some(TriviaEffect::SyncBoard(self.position.clone()));
        }

        let expected_move = &target.moves[self.move_index];

        if &mv == expected_move {
            if self.position.is_legal(mv) {
                self.position = self.position.clone().play(mv).expect("Legal");
            }

            self.move_index += 1;

            if self.move_index >= target.moves.len() {
                self.status = TriviaGameStatus::LineComplete;
                self.last_feedback = format!("Success! You completed {}.", target.name);
            } else {
                self.status = TriviaGameStatus::ShowingCorrect;
                self.make_opponent_move();
            }

            // Sync board to show opponent move or just ensure consistency
            Some(TriviaEffect::SyncBoard(self.position.clone()))
        } else {
            // Wrong move
            self.shake_trigger = true;
            // The user moved locally (in UI), but it was wrong.
            // We need to revert the UI board to self.position (which hasn't changed).
            Some(TriviaEffect::SyncBoard(self.position.clone()))
        }
    }

    fn make_opponent_move(&mut self) {
        let Some(target) = &self.target else { return };

        if self.move_index >= target.moves.len() {
            self.status = TriviaGameStatus::LineComplete;
            self.last_feedback = format!("Done! You completed {}.", target.name);
            return;
        }

        let opponent_move = &target.moves[self.move_index];

        if self.position.is_legal(*opponent_move) {
            self.position = self
                .position
                .clone()
                .play(*opponent_move)
                .expect("Invalid opponent move");
            self.move_index += 1;

            if self.move_index >= target.moves.len() {
                self.status = TriviaGameStatus::LineComplete;
                self.last_feedback = format!("Success! You completed {}.", target.name);
            } else {
                self.status = TriviaGameStatus::AwaitingUserMove;
                self.last_feedback = String::new();
            }
        } else {
            self.last_feedback = "Error: Invalid move in opening sequence.".to_string();
        }
    }

    pub fn is_completed(&self) -> bool {
        self.status == TriviaGameStatus::LineComplete
    }
}

#[derive(Debug, Clone)]
pub enum TriviaMessage {
    UserMoved(Move),
    ShowHint,
    PlayCorrectMove,
    RequestNewTrivia,
    ConfirmNewTrivia,
    CancelNewTrivia,
}

#[derive(Debug, Clone)]
pub enum TriviaEffect {
    ShowHint(Option<shakmaty::Square>),
    SyncBoard(Chess),
}
