//! Shared review icon assets, colors, and quality helpers.
//!
//! Single source of truth for review badge icons and their associated colors,
//! used by the board canvas, move ribbon, right panel, and game review page.

use iced::{Color, Theme};

use crate::core::review::MoveQuality;
use crate::iced::style::Palette;

// -- Icon bytes (for iced svg widget) --

const BOOK_ICON: &[u8] = include_bytes!("../../../assets/review_icons/book.svg");
const BRILLIANT_ICON: &[u8] = include_bytes!("../../../assets/review_icons/brilliant.svg");
const GREAT_ICON: &[u8] = include_bytes!("../../../assets/review_icons/great.svg");
const BEST_ICON: &[u8] = include_bytes!("../../../assets/review_icons/best.svg");
const EXCELLENT_ICON: &[u8] = include_bytes!("../../../assets/review_icons/excellent.svg");
const GOOD_ICON: &[u8] = include_bytes!("../../../assets/review_icons/good.svg");
const INACCURACY_ICON: &[u8] = include_bytes!("../../../assets/review_icons/inaccuracy.svg");
const MISTAKE_ICON: &[u8] = include_bytes!("../../../assets/review_icons/mistake.svg");
const BLUNDER_ICON: &[u8] = include_bytes!("../../../assets/review_icons/blunder.svg");
const MISSED_ICON: &[u8] = include_bytes!("../../../assets/review_icons/missed.svg");

/// Get the raw SVG bytes for a move quality badge (for iced svg widget).
pub fn icon_bytes(quality: MoveQuality) -> &'static [u8] {
    match quality {
        MoveQuality::Book => BOOK_ICON,
        MoveQuality::Brilliant => BRILLIANT_ICON,
        MoveQuality::Great => GREAT_ICON,
        MoveQuality::Best => BEST_ICON,
        MoveQuality::Excellent => EXCELLENT_ICON,
        MoveQuality::Good => GOOD_ICON,
        MoveQuality::Inaccuracy => INACCURACY_ICON,
        MoveQuality::Mistake => MISTAKE_ICON,
        MoveQuality::Blunder => BLUNDER_ICON,
        MoveQuality::Missed => MISSED_ICON,
    }
}

// -- Icon SVG strings (for canvas rasterization) --

const BOOK_SVG: &str = include_str!("../../../assets/review_icons/book.svg");
const BRILLIANT_SVG: &str = include_str!("../../../assets/review_icons/brilliant.svg");
const GREAT_SVG: &str = include_str!("../../../assets/review_icons/great.svg");
const BEST_SVG: &str = include_str!("../../../assets/review_icons/best.svg");
const EXCELLENT_SVG: &str = include_str!("../../../assets/review_icons/excellent.svg");
const GOOD_SVG: &str = include_str!("../../../assets/review_icons/good.svg");
const INACCURACY_SVG: &str = include_str!("../../../assets/review_icons/inaccuracy.svg");
const MISTAKE_SVG: &str = include_str!("../../../assets/review_icons/mistake.svg");
const BLUNDER_SVG: &str = include_str!("../../../assets/review_icons/blunder.svg");
const MISSED_SVG: &str = include_str!("../../../assets/review_icons/missed.svg");

/// Get the SVG template string for a move quality badge (for canvas rasterization).
pub fn icon_svg_str(quality: MoveQuality) -> &'static str {
    match quality {
        MoveQuality::Book => BOOK_SVG,
        MoveQuality::Brilliant => BRILLIANT_SVG,
        MoveQuality::Great => GREAT_SVG,
        MoveQuality::Best => BEST_SVG,
        MoveQuality::Excellent => EXCELLENT_SVG,
        MoveQuality::Good => GOOD_SVG,
        MoveQuality::Inaccuracy => INACCURACY_SVG,
        MoveQuality::Mistake => MISTAKE_SVG,
        MoveQuality::Blunder => BLUNDER_SVG,
        MoveQuality::Missed => MISSED_SVG,
    }
}

// -- Theme-aware colors --

/// Get the theme-aware color for a move quality badge.
pub fn quality_color(theme: &Theme, quality: MoveQuality) -> Color {
    match quality {
        MoveQuality::Book => Palette::accent(theme),
        MoveQuality::Brilliant => Color::from_rgb(0.0, 0.85, 0.95),
        MoveQuality::Great => Color::from_rgb(0.2, 0.8, 0.45),
        MoveQuality::Best => Palette::success(theme),
        MoveQuality::Excellent => Color::from_rgb(0.45, 0.82, 0.55),
        MoveQuality::Good => Color::from_rgb(0.72, 0.78, 0.46),
        MoveQuality::Inaccuracy => Palette::warning(theme),
        MoveQuality::Mistake => Color::from_rgb(0.95, 0.45, 0.18),
        MoveQuality::Blunder => Palette::error(theme),
        MoveQuality::Missed => Color::from_rgb(0.95, 0.62, 0.15),
    }
}

/// Get a fixed (non-theme-aware) color for a move quality badge.
/// Used in contexts where no Theme is available (e.g., canvas rendering).
pub fn quality_color_fixed(quality: MoveQuality) -> Color {
    match quality {
        MoveQuality::Book => Color::from_rgb(0.43, 0.66, 1.0),
        MoveQuality::Brilliant => Color::from_rgb(0.0, 0.85, 0.95),
        MoveQuality::Great => Color::from_rgb(0.2, 0.8, 0.45),
        MoveQuality::Best => Color::from_rgb(0.11, 0.73, 0.33),
        MoveQuality::Excellent => Color::from_rgb(0.45, 0.82, 0.55),
        MoveQuality::Good => Color::from_rgb(0.72, 0.78, 0.46),
        MoveQuality::Inaccuracy => Color::from_rgb(0.94, 0.72, 0.29),
        MoveQuality::Mistake => Color::from_rgb(0.95, 0.45, 0.18),
        MoveQuality::Blunder => Color::from_rgb(0.88, 0.32, 0.32),
        MoveQuality::Missed => Color::from_rgb(0.96, 0.64, 0.2),
    }
}
