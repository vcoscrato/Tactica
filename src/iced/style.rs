//! Style constants and color palette for the application.
//!
//! Provides a consistent design system - not all colors may be currently used.

use iced::{Color, Theme};

// -- Shared timing constants --

/// Engine polling interval (milliseconds).
pub const ENGINE_POLL_MS: u64 = 100;
/// Animation tick interval (milliseconds) -- targets ~60 FPS.
pub const ANIMATION_TICK_MS: u64 = 16;
/// Eval bar lerp factor per tick (0.0 = static, 1.0 = instant snap).
pub const EVAL_LERP_FACTOR: f32 = 0.1;

pub struct Palette;

impl Palette {
    // Orange Accent
    pub const ORANGE: Color = Color::from_rgb(1.0, 0.55, 0.0); // #FF8C00
    pub const ORANGE_HOVER: Color = Color::from_rgb(1.0, 0.65, 0.1);

    pub fn background(theme: &Theme) -> Color {
        match theme {
            Theme::Light => Color::from_rgb(0.94, 0.94, 0.94), // #F0F0F0
            _ => Color::from_rgb(0.12, 0.12, 0.12),            // #1E1E1E
        }
    }

    pub fn surface(theme: &Theme) -> Color {
        match theme {
            Theme::Light => Color::from_rgb(0.98, 0.98, 0.98), // #FAFAFA
            _ => Color::from_rgb(0.16, 0.16, 0.16),            // #292929
        }
    }

    pub fn panel(theme: &Theme) -> Color {
        match theme {
            Theme::Light => Color::from_rgb(0.90, 0.90, 0.90), // #E5E5E5
            _ => Color::from_rgb(0.14, 0.14, 0.14),            // #242424
        }
    }

    pub fn border(theme: &Theme) -> Color {
        match theme {
            Theme::Light => Color::from_rgb(0.80, 0.80, 0.80), // #CCCCCC
            _ => Color::from_rgb(0.25, 0.25, 0.25),            // #404040
        }
    }

    pub fn text_primary(theme: &Theme) -> Color {
        match theme {
            Theme::Light => Color::from_rgb(0.1, 0.1, 0.1), // #1A1A1A
            _ => Color::from_rgb(0.9, 0.9, 0.9),            // #E6E6E6
        }
    }

    pub fn text_secondary(theme: &Theme) -> Color {
        match theme {
            Theme::Light => Color::from_rgb(0.4, 0.4, 0.4), // #666666
            _ => Color::from_rgb(0.6, 0.6, 0.6),            // #999999
        }
    }

    pub fn text_muted(theme: &Theme) -> Color {
        match theme {
            Theme::Light => Color::from_rgb(0.6, 0.6, 0.6), // #999999
            _ => Color::from_rgb(0.4, 0.4, 0.4),            // #666666
        }
    }

    pub fn accent(_theme: &Theme) -> Color {
        Self::ORANGE
    }

    pub fn accent_hover(_theme: &Theme) -> Color {
        Self::ORANGE_HOVER
    }

    pub fn success(_theme: &Theme) -> Color {
        Color::from_rgb(0.4, 0.8, 0.4)
    }

    pub fn error(_theme: &Theme) -> Color {
        Color::from_rgb(0.9, 0.4, 0.4)
    }

    pub fn warning(_theme: &Theme) -> Color {
        Color::from_rgb(0.9, 0.7, 0.2)
    }

    pub fn board_white(_theme: &Theme) -> Color {
        Color::from_rgb(0.93, 0.85, 0.76) // #EDE0C2
    }

    pub fn board_black(_theme: &Theme) -> Color {
        Color::from_rgb(0.66, 0.55, 0.45) // #A88C73
    }

    pub fn board_highlight_move(_theme: &Theme) -> Color {
        Color::from_rgba(1.0, 0.65, 0.0, 0.4) // Orange tint
    }

    pub fn board_highlight_check(_theme: &Theme) -> Color {
        Color::from_rgba(1.0, 0.2, 0.2, 0.6)
    }

    pub fn board_theme_colors(theme: crate::core::config::BoardTheme) -> (Color, Color) {
        match theme {
            crate::core::config::BoardTheme::Blue => (
                Color::from_rgb(0.93, 0.93, 0.95), // White
                Color::from_rgb(0.55, 0.65, 0.77), // Blue
            ),
            crate::core::config::BoardTheme::Green => (
                Color::from_rgb(0.93, 0.93, 0.82), // White
                Color::from_rgb(0.46, 0.59, 0.33), // Green
            ),
            crate::core::config::BoardTheme::Brown => (
                Color::from_rgb(0.93, 0.85, 0.76), // White (#EDE0C2)
                Color::from_rgb(0.66, 0.55, 0.45), // Brown (#A88C73)
            ),
        }
    }
}

// Container styles
pub mod containers {
    use super::Palette;
    use iced::widget::container;
    use iced::{Background, Border, Color, Theme};

    /// Dark overlay for modals
    pub fn overlay(_: &Theme) -> container::Style {
        container::Style {
            background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.7))),
            ..Default::default()
        }
    }

    /// Modal box with rounded corners
    pub fn modal_box(theme: &Theme) -> container::Style {
        container::Style {
            background: Some(Background::Color(Palette::surface(theme))),
            border: Border {
                color: Palette::border(theme),
                width: 1.0,
                radius: 12.0.into(),
            },
            ..Default::default()
        }
    }

    /// Panel with configurable radius
    pub fn panel(radius: f32) -> impl Fn(&Theme) -> container::Style {
        move |theme| container::Style {
            background: Some(Background::Color(Palette::surface(theme))),
            border: Border {
                color: Palette::border(theme),
                width: 1.0,
                radius: radius.into(),
            },
            ..Default::default()
        }
    }

    /// Keyboard shortcut badge
    pub fn kbd_badge(theme: &Theme) -> container::Style {
        container::Style {
            background: Some(Background::Color(Palette::surface(theme))),
            border: Border {
                radius: 4.0.into(),
                color: Palette::border(theme),
                width: 1.0,
            },
            ..Default::default()
        }
    }

    /// Background fill only (no border)
    pub fn background(theme: &Theme) -> container::Style {
        container::Style {
            background: Some(Background::Color(Palette::background(theme))),
            ..Default::default()
        }
    }
}

// Button styles helper
pub mod buttons {
    use super::Palette;
    use iced::widget::button;
    use iced::{Background, Border, Color, Theme};

    /// Icon button style (for toolbar icons)
    pub fn icon(theme: &Theme, _status: button::Status) -> button::Style {
        button::Style {
            background: Some(Background::Color(Palette::surface(theme))),
            border: Border {
                color: Palette::border(theme),
                width: 1.0,
                radius: 6.0.into(),
            },
            text_color: Palette::text_primary(theme),
            ..Default::default()
        }
    }

    /// Toggle button style (for On/Off settings)
    pub fn toggle(theme: &Theme, _status: button::Status) -> button::Style {
        button::Style {
            background: Some(Background::Color(Palette::panel(theme))),
            border: Border {
                color: Palette::border(theme),
                width: 1.0,
                radius: 4.0.into(),
            },
            text_color: Palette::text_primary(theme),
            ..Default::default()
        }
    }

    pub fn primary(theme: &Theme, status: button::Status) -> button::Style {
        let base = button::Style {
            background: Some(Background::Color(Palette::accent(theme))),
            text_color: Color::WHITE,
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        };
        match status {
            button::Status::Hovered | button::Status::Pressed => button::Style {
                background: Some(Background::Color(Palette::accent_hover(theme))),
                ..base
            },
            _ => base,
        }
    }

    pub fn secondary(theme: &Theme, status: button::Status) -> button::Style {
        let base = button::Style {
            background: Some(Background::Color(Palette::surface(theme))),
            text_color: Palette::text_primary(theme),
            border: Border {
                color: Palette::border(theme),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        };
        match status {
            button::Status::Hovered | button::Status::Pressed => button::Style {
                background: Some(Background::Color(Palette::border(theme))),
                ..base
            },
            _ => base,
        }
    }

    pub fn danger(_theme: &Theme, status: button::Status) -> button::Style {
        let base = button::Style {
            background: Some(Background::Color(Color::from_rgb(0.6, 0.2, 0.2))),
            text_color: Color::WHITE,
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        };
        match status {
            button::Status::Hovered | button::Status::Pressed => button::Style {
                background: Some(Background::Color(Color::from_rgb(0.7, 0.25, 0.25))),
                ..base
            },
            _ => base,
        }
    }
}
