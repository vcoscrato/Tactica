use crate::core::config::AppSettings;
use crate::core::openings::{Opening, OpeningNames};
use shakmaty::{Chess, Move, Position};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MoveFeedback {
    Correct,  // Green
    WrongPos, // Yellow
    Wrong,    // Gray
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChessleStatus {
    Setup,
    Playing,
    Won,
    Lost,
}

#[derive(Clone, Debug)]
pub struct Guess {
    pub moves: Vec<Move>,
    pub feedback: Vec<MoveFeedback>,
}

#[derive(Debug, Clone)]
pub struct ChessleState {
    pub settings: AppSettings,
    pub opening_names: OpeningNames,

    pub status: ChessleStatus,
    pub move_count: usize,
    pub max_guesses: usize,

    pub target: Option<Opening>,
    pub target_moves: Vec<Move>,
    pub guesses: Vec<Guess>,

    pub current_guess_moves: Vec<Move>,
    pub current_position: Chess,
}

impl ChessleState {
    pub fn new(settings: AppSettings) -> Self {
        let opening_names = OpeningNames::new();

        Self {
            settings,
            opening_names,
            status: ChessleStatus::Setup,
            move_count: 3,
            max_guesses: 5,
            target: None,
            target_moves: Vec::new(),
            guesses: Vec::new(),
            current_guess_moves: Vec::new(),
            current_position: Chess::default(),
        }
    }

    pub fn update(&mut self, message: ChessleMessage) {
        match message {
            ChessleMessage::StartGame => self.start_game(),
            ChessleMessage::SetMoveCount(c) => self.move_count = c.clamp(3, 6),
            ChessleMessage::SetMaxGuesses(c) => self.max_guesses = c.clamp(3, 10),
            ChessleMessage::UserMove(mv) => self.make_move(mv),
            ChessleMessage::Undo => self.undo(),
            ChessleMessage::Submit => self.submit(),
            ChessleMessage::AutoFill => self.auto_fill(),
        }
    }

    fn start_game(&mut self) {
        let total_moves = self.move_count * 2;
        if let Some(opening) = self.opening_names.get_random_opening_min_moves(total_moves) {
            self.target = Some(opening.clone());
            self.target_moves = opening.moves.iter().take(total_moves).cloned().collect();
            self.guesses.clear();
            self.current_guess_moves.clear();
            self.current_position = Chess::default();
            self.status = ChessleStatus::Playing;
        }
    }

    fn make_move(&mut self, mv: Move) {
        if self.status != ChessleStatus::Playing {
            return;
        }

        let total_moves = self.move_count * 2;
        if self.current_guess_moves.len() >= total_moves {
            return;
        }

        if let Ok(new_pos) = self.current_position.clone().play(mv) {
            self.current_position = new_pos;
            self.current_guess_moves.push(mv);
        }
    }

    fn undo(&mut self) {
        if self.status != ChessleStatus::Playing {
            return;
        }
        if !self.current_guess_moves.is_empty() {
            self.current_guess_moves.pop();
            // Replay
            let mut pos = Chess::default();
            for m in &self.current_guess_moves {
                pos = pos.clone().play(*m).unwrap_or(pos);
            }
            self.current_position = pos;
        }
    }

    fn submit(&mut self) {
        if self.status != ChessleStatus::Playing {
            return;
        }
        let total_moves = self.move_count * 2;
        if self.current_guess_moves.len() != total_moves {
            return;
        }

        let feedback: Vec<MoveFeedback> = self
            .current_guess_moves
            .iter()
            .enumerate()
            .map(|(i, mv)| {
                if i < self.target_moves.len() && mv == &self.target_moves[i] {
                    MoveFeedback::Correct
                } else if self.target_moves.contains(mv) {
                    MoveFeedback::WrongPos
                } else {
                    MoveFeedback::Wrong
                }
            })
            .collect();

        let all_correct = feedback.iter().all(|f| *f == MoveFeedback::Correct);

        self.guesses.push(Guess {
            moves: self.current_guess_moves.clone(),
            feedback,
        });

        if all_correct {
            self.status = ChessleStatus::Won;
        } else if self.guesses.len() >= self.max_guesses {
            self.status = ChessleStatus::Lost;
        } else {
            self.current_guess_moves.clear();
            self.current_position = Chess::default();
        }
    }

    fn auto_fill(&mut self) {
        if self.status != ChessleStatus::Playing {
            return;
        }
        let total_moves = self.move_count * 2;
        let mut known = vec![false; total_moves];

        for g in &self.guesses {
            for (i, f) in g.feedback.iter().enumerate() {
                if *f == MoveFeedback::Correct && i < total_moves {
                    known[i] = true;
                }
            }
        }

        while self.current_guess_moves.len() < total_moves {
            let depth = self.current_guess_moves.len();
            if !known[depth] {
                break;
            }
            if depth >= self.target_moves.len() {
                break;
            }

            let mv = self.target_moves[depth];
            if self.current_position.is_legal(mv) {
                self.make_move(mv); // Re-use logic
            } else {
                break;
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum ChessleMessage {
    StartGame,
    SetMoveCount(usize),
    SetMaxGuesses(usize),
    UserMove(Move),
    Undo,
    Submit,
    AutoFill,
}
