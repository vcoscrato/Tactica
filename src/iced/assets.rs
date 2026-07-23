//! Embedded UI assets for installed builds.

use iced::widget::svg;

const ARROW_LEFT_ICON: &[u8] = include_bytes!("../../assets/icons/arrow-left.svg");
const BRANCH_ICON: &[u8] = include_bytes!("../../assets/icons/branch.svg");
const CLOSE_ICON: &[u8] = include_bytes!("../../assets/icons/close.svg");
const EDIT_ICON: &[u8] = include_bytes!("../../assets/icons/edit.svg");
const HELP_ICON: &[u8] = include_bytes!("../../assets/icons/help.svg");
const MENU_ICON: &[u8] = include_bytes!("../../assets/icons/menu.svg");
const MOON_ICON: &[u8] = include_bytes!("../../assets/icons/moon.svg");
const SAVE_ICON: &[u8] = include_bytes!("../../assets/icons/save.svg");
const SETTINGS_ICON: &[u8] = include_bytes!("../../assets/icons/settings.svg");
const STAR_OUTLINE_ICON: &[u8] = include_bytes!("../../assets/icons/star-outline.svg");
const STAR_SOLID_ICON: &[u8] = include_bytes!("../../assets/icons/star-solid.svg");
const SUN_ICON: &[u8] = include_bytes!("../../assets/icons/sun.svg");
const TRASH_ICON: &[u8] = include_bytes!("../../assets/icons/trash.svg");

const WHITE_BISHOP: &str = include_str!("../../assets/pieces/B.svg");
const WHITE_KING: &str = include_str!("../../assets/pieces/K.svg");
const WHITE_KNIGHT: &str = include_str!("../../assets/pieces/N.svg");
const WHITE_PAWN: &str = include_str!("../../assets/pieces/P.svg");
const WHITE_QUEEN: &str = include_str!("../../assets/pieces/Q.svg");
const WHITE_ROOK: &str = include_str!("../../assets/pieces/R.svg");
const BLACK_BISHOP: &str = include_str!("../../assets/pieces/b.svg");
const BLACK_KING: &str = include_str!("../../assets/pieces/k.svg");
const BLACK_KNIGHT: &str = include_str!("../../assets/pieces/n.svg");
const BLACK_PAWN: &str = include_str!("../../assets/pieces/p.svg");
const BLACK_QUEEN: &str = include_str!("../../assets/pieces/q.svg");
const BLACK_ROOK: &str = include_str!("../../assets/pieces/r.svg");

pub fn icon(name: &str) -> svg::Handle {
    svg::Handle::from_memory(icon_bytes(name).unwrap_or(HELP_ICON))
}

pub fn icon_bytes(name: &str) -> Option<&'static [u8]> {
    let name = name
        .strip_prefix("assets/icons/")
        .unwrap_or(name)
        .strip_suffix(".svg")
        .unwrap_or_else(|| name.strip_prefix("assets/icons/").unwrap_or(name));

    match name {
        "arrow-left" => Some(ARROW_LEFT_ICON),
        "branch" => Some(BRANCH_ICON),
        "close" => Some(CLOSE_ICON),
        "edit" => Some(EDIT_ICON),
        "help" => Some(HELP_ICON),
        "menu" => Some(MENU_ICON),
        "moon" => Some(MOON_ICON),
        "save" => Some(SAVE_ICON),
        "settings" => Some(SETTINGS_ICON),
        "star-outline" => Some(STAR_OUTLINE_ICON),
        "star-solid" => Some(STAR_SOLID_ICON),
        "sun" => Some(SUN_ICON),
        "trash" => Some(TRASH_ICON),
        _ => None,
    }
}

pub fn piece_svg(piece: char) -> Option<&'static str> {
    match piece {
        'B' => Some(WHITE_BISHOP),
        'K' => Some(WHITE_KING),
        'N' => Some(WHITE_KNIGHT),
        'P' => Some(WHITE_PAWN),
        'Q' => Some(WHITE_QUEEN),
        'R' => Some(WHITE_ROOK),
        'b' => Some(BLACK_BISHOP),
        'k' => Some(BLACK_KING),
        'n' => Some(BLACK_KNIGHT),
        'p' => Some(BLACK_PAWN),
        'q' => Some(BLACK_QUEEN),
        'r' => Some(BLACK_ROOK),
        _ => None,
    }
}
