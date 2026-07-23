//! Game outcome detection and user-facing result notation.

use shakmaty::{Chess, Color, KnownOutcome, Position};

/// A terminal result for a standard chess position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameResult {
    WhiteWins,
    BlackWins,
    Draw,
}

impl GameResult {
    /// Detect a terminal result from `position`.
    ///
    /// This covers checkmate, stalemate, and insufficient material through
    /// `shakmaty`'s standard chess outcome rules.
    pub fn from_position(position: &Chess) -> Option<Self> {
        match position.outcome().known()? {
            KnownOutcome::Decisive {
                winner: Color::White,
            } => Some(Self::WhiteWins),
            KnownOutcome::Decisive {
                winner: Color::Black,
            } => Some(Self::BlackWins),
            KnownOutcome::Draw => Some(Self::Draw),
        }
    }

    /// Result notation displayed in the app.
    pub const fn notation(self) -> &'static str {
        match self {
            Self::WhiteWins => "1-0",
            Self::BlackWins => "0-1",
            Self::Draw => "0.5-0.5",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakmaty::san::San;

    fn play_sans(sans: &[&str]) -> Chess {
        let mut position = Chess::default();
        for san in sans {
            let mv = san
                .parse::<San>()
                .expect("valid SAN")
                .to_move(&position)
                .expect("legal move");
            position = position.play(mv).expect("legal position");
        }
        position
    }

    #[test]
    fn reports_white_checkmate_win() {
        let position = play_sans(&["e4", "e5", "Qh5", "Nc6", "Bc4", "Nf6", "Qxf7#"]);

        let result = GameResult::from_position(&position).expect("terminal result");
        assert_eq!(result, GameResult::WhiteWins);
        assert_eq!(result.notation(), "1-0");
    }

    #[test]
    fn reports_black_checkmate_win() {
        let position = play_sans(&["f3", "e5", "g4", "Qh4#"]);

        let result = GameResult::from_position(&position).expect("terminal result");
        assert_eq!(result, GameResult::BlackWins);
        assert_eq!(result.notation(), "0-1");
    }

    #[test]
    fn reports_insufficient_material_draw() {
        let position = crate::core::pgn::parse_fen("8/8/8/8/8/8/2k5/K7 w - - 0 1")
            .expect("valid king-only position");

        let result = GameResult::from_position(&position).expect("terminal result");
        assert_eq!(result, GameResult::Draw);
        assert_eq!(result.notation(), "0.5-0.5");
    }

    #[test]
    fn reports_stalemate_draw() {
        let position = crate::core::pgn::parse_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1")
            .expect("valid stalemate position");

        let result = GameResult::from_position(&position).expect("terminal result");
        assert_eq!(result, GameResult::Draw);
        assert_eq!(result.notation(), "0.5-0.5");
    }

    #[test]
    fn ongoing_position_has_no_result() {
        assert_eq!(GameResult::from_position(&Chess::default()), None);
    }
}
