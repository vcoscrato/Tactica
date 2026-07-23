use crate::core::config::EngineSettings;
use crate::core::engine::Engine;
use serde::{Deserialize, Serialize};
use shakmaty::{Chess, Color, File, Move, Position, Rank, Role, Square};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MoveQuality {
    Book,
    Brilliant,
    Great,
    Best,
    Excellent,
    Good,
    Inaccuracy,
    Mistake,
    Blunder,
    Missed,
}

impl MoveQuality {
    /// Human-readable label for this quality level.
    pub fn label(self) -> &'static str {
        match self {
            Self::Book => "Book",
            Self::Brilliant => "Brilliant",
            Self::Great => "Great",
            Self::Best => "Best",
            Self::Excellent => "Excellent",
            Self::Good => "Good",
            Self::Inaccuracy => "Inaccuracy",
            Self::Mistake => "Mistake",
            Self::Blunder => "Blunder",
            Self::Missed => "Missed",
        }
    }

    /// Short description of what this quality level means.
    pub fn description(self) -> &'static str {
        match self {
            Self::Book => "Opening database move",
            Self::Brilliant => "Best move with a sacrifice idea",
            Self::Great => "Only/critical best move",
            Self::Best => "Engine top choice",
            Self::Excellent => "Very small eval loss",
            Self::Good => "Playable with limited loss",
            Self::Inaccuracy => "Noticeable eval loss",
            Self::Mistake => "Major eval loss",
            Self::Blunder => "Severe losing move",
            Self::Missed => "Missed tactical opportunity",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveReview {
    pub ply: usize,
    pub quality: MoveQuality,
    pub loss_cp: i32,
    pub best_move_uci: Option<String>,
    pub played_move_uci: String,
    pub reason: String,
}

// -- Review classification thresholds (centipawns) --

/// Maximum loss (cp) to still be classified as Excellent.
const EXCELLENT_THRESHOLD: i32 = 25;
/// Maximum loss (cp) to still be classified as Good.
const GOOD_THRESHOLD: i32 = 60;
/// Maximum loss (cp) to still be classified as Inaccuracy.
const INACCURACY_THRESHOLD: i32 = 120;
/// Maximum loss (cp) to still be classified as Mistake.
const MISTAKE_THRESHOLD: i32 = 220;
/// Minimum loss (cp) to qualify as a Missed opportunity.
const MISSED_LOSS_THRESHOLD: i32 = 180;
/// Minimum expected eval (cp) for a Missed opportunity.
const MISSED_EXPECTED_THRESHOLD: i32 = 120;
/// Maximum actual eval (cp) for a Missed opportunity.
const MISSED_ACTUAL_THRESHOLD: i32 = -80;
/// Material sacrifice threshold (cp) for a Brilliant move.
const BRILLIANT_MATERIAL_DELTA: i32 = -200;
/// Maximum loss (cp) for a Brilliant move (must still be nearly best).
const BRILLIANT_LOSS_THRESHOLD: i32 = 30;
/// Minimum gap (cp) from second/third best to qualify as Great.
const GREAT_GAP_THRESHOLD: i32 = 120;
/// Default analysis timeout per move (ms).
const REVIEW_TIMEOUT_MS: u64 = 9000;

pub fn review_line(
    start: Chess,
    moves: Vec<Move>,
    settings: &EngineSettings,
) -> Result<Vec<MoveReview>, String> {
    if moves.is_empty() {
        return Ok(Vec::new());
    }

    let mut review_settings = settings.clone();
    review_settings.multi_pv = review_settings.multi_pv.max(3);

    let mut engine = Engine::new_with_settings(&review_settings)?;
    let review_depth = settings.max_depth.unwrap_or(18).max(16).clamp(16, 36);
    let timeout_ms = REVIEW_TIMEOUT_MS;

    let mut pos = start;
    let mut out = Vec::with_capacity(moves.len());

    for (idx, mv) in moves.into_iter().enumerate() {
        let fen_before = crate::core::board::position_to_fen(&pos);
        let before = engine.analyze_position_depth(
            &fen_before,
            pos.turn().is_white(),
            review_depth,
            timeout_ms,
        )?;

        let best_move_uci = before
            .best_move
            .clone()
            .or_else(|| before.lines.first().and_then(|l| l.pv.first()).cloned());

        let mover = pos.turn();
        let expected = mover_perspective_cp(
            before.best_line().and_then(|l| l.score_cp).unwrap_or(0),
            mover,
        );
        let second_best = before
            .lines
            .get(1)
            .and_then(|l| l.score_cp)
            .map(|cp| mover_perspective_cp(cp, mover));
        let third_best = before
            .lines
            .get(2)
            .and_then(|l| l.score_cp)
            .map(|cp| mover_perspective_cp(cp, mover));

        let before_material = material_balance_cp(&pos, mover);
        let played_uci = to_uci(&mv);

        let Ok(next_pos) = pos.clone().play(mv) else {
            return Err("Illegal move encountered during review".to_string());
        };

        let fen_after = crate::core::board::position_to_fen(&next_pos);
        let after = engine.analyze_position_depth(
            &fen_after,
            next_pos.turn().is_white(),
            review_depth,
            timeout_ms,
        )?;

        let actual = mover_perspective_cp(
            after.best_line().and_then(|l| l.score_cp).unwrap_or(0),
            mover,
        );
        let loss = (expected - actual).max(0);

        let is_best = best_move_uci.as_deref() == Some(played_uci.as_str());
        let after_material = material_balance_cp(&next_pos, mover);
        let material_delta = after_material - before_material;

        let quality = classify_move(
            loss,
            expected,
            actual,
            is_best,
            second_best,
            third_best,
            material_delta,
        );
        let reason = reason_text(quality, loss, expected, actual);

        out.push(MoveReview {
            ply: idx + 1,
            quality,
            loss_cp: loss,
            best_move_uci,
            played_move_uci: played_uci,
            reason,
        });

        pos = next_pos;
    }

    Ok(out)
}

fn classify_move(
    loss: i32,
    expected: i32,
    actual: i32,
    is_best: bool,
    second_best: Option<i32>,
    third_best: Option<i32>,
    material_delta: i32,
) -> MoveQuality {
    if !is_best
        && loss >= MISSED_LOSS_THRESHOLD
        && expected >= MISSED_EXPECTED_THRESHOLD
        && actual > MISSED_ACTUAL_THRESHOLD
    {
        return MoveQuality::Missed;
    }

    if is_best {
        let best_gap = second_best
            .into_iter()
            .chain(third_best)
            .map(|s| expected - s)
            .max()
            .unwrap_or(0);
        if material_delta <= BRILLIANT_MATERIAL_DELTA && loss <= BRILLIANT_LOSS_THRESHOLD {
            return MoveQuality::Brilliant;
        }
        if best_gap >= GREAT_GAP_THRESHOLD {
            return MoveQuality::Great;
        }
        return MoveQuality::Best;
    }

    if loss <= EXCELLENT_THRESHOLD {
        MoveQuality::Excellent
    } else if loss <= GOOD_THRESHOLD {
        MoveQuality::Good
    } else if loss <= INACCURACY_THRESHOLD {
        MoveQuality::Inaccuracy
    } else if loss <= MISTAKE_THRESHOLD {
        MoveQuality::Mistake
    } else {
        MoveQuality::Blunder
    }
}

fn reason_text(quality: MoveQuality, loss: i32, expected: i32, actual: i32) -> String {
    match quality {
        MoveQuality::Book => "Known opening move".to_string(),
        MoveQuality::Brilliant => "Best move with a meaningful sacrifice".to_string(),
        MoveQuality::Great => "Best move in a demanding position".to_string(),
        MoveQuality::Best => "Engine top choice".to_string(),
        MoveQuality::Excellent => format!("Very small drop ({} cp)", loss),
        MoveQuality::Good => format!("Playable with limited loss ({} cp)", loss),
        MoveQuality::Inaccuracy => format!("Noticeable loss ({} cp)", loss),
        MoveQuality::Mistake => format!("Major loss ({} cp)", loss),
        MoveQuality::Blunder => {
            format!(
                "Losing move ({} cp, {} -> {})",
                loss,
                expected / 100,
                actual / 100
            )
        }
        MoveQuality::Missed => "Missed a strong tactical chance".to_string(),
    }
}

fn mover_perspective_cp(white_cp: i32, mover: Color) -> i32 {
    if mover.is_white() {
        white_cp
    } else {
        -white_cp
    }
}

fn material_balance_cp(position: &Chess, perspective: Color) -> i32 {
    let mut white = 0;
    let mut black = 0;
    for rank in 0..8 {
        for file in 0..8 {
            let sq = Square::from_coords(File::new(file), Rank::new(rank));
            if let Some(piece) = position.board().piece_at(sq) {
                let v = role_value_cp(piece.role);
                if piece.color.is_white() {
                    white += v;
                } else {
                    black += v;
                }
            }
        }
    }
    let cp = white - black;
    if perspective.is_white() { cp } else { -cp }
}

fn role_value_cp(role: Role) -> i32 {
    match role {
        Role::Pawn => 100,
        Role::Knight => 320,
        Role::Bishop => 330,
        Role::Rook => 500,
        Role::Queen => 900,
        Role::King => 0,
    }
}

fn to_uci(mv: &Move) -> String {
    let mut uci = format!(
        "{}{}",
        mv.from().map(|s| s.to_string()).unwrap_or_default(),
        mv.to()
    );
    if let Some(promo) = mv.promotion() {
        let c = match promo {
            Role::Queen => 'q',
            Role::Rook => 'r',
            Role::Bishop => 'b',
            Role::Knight => 'n',
            Role::Pawn | Role::King => 'q',
        };
        uci.push(c);
    }
    uci
}
