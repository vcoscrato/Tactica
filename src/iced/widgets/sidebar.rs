//! Sidebar Widgets - Standard components for the right-hand panel
//!
//! Provides consistent headers, section dividers, and action rows.

use crate::iced::style::Palette;
use iced::widget::{Space, column, row, text};
use iced::{Alignment, Element, Length, Theme};

/// A standard sidebar header with title, optional subtitle, and optional actions
pub fn panel_header<'a, Message: 'a>(
    theme: &Theme,
    title: impl Into<String>,
    subtitle: Option<String>,
    scale: f32,
    actions: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let title_size = 20.0 * scale;
    let subtitle_size = 13.0 * scale;
    let spacing = 4.0 * scale;

    let mut col = column![
        text(title.into())
            .size(title_size)
            .color(Palette::accent(theme))
    ];

    if let Some(sub) = subtitle {
        col = col.push(
            text(sub)
                .size(subtitle_size)
                .color(Palette::text_secondary(theme)),
        );
    }

    let left = col.spacing(spacing);

    if let Some(actions) = actions {
        row![left, Space::new().width(Length::Fill), actions]
            .align_y(Alignment::Center)
            .into()
    } else {
        left.into()
    }
}

/// A labeled section container
pub fn section<'a, Message>(
    theme: &Theme,
    title: String,
    content: impl Into<Element<'a, Message>>,
    scale: f32,
) -> Element<'a, Message>
where
    Message: 'a,
{
    let title_size = 12.0 * scale;
    let spacing = 8.0 * scale;
    column![
        text(title)
            .size(title_size)
            .color(Palette::text_muted(theme)),
        content.into()
    ]
    .spacing(spacing)
    .into()
}

/// A row of action buttons with consistent spacing
pub fn action_row<'a, Message>(
    buttons: Vec<Element<'a, Message>>,
    scale: f32,
) -> Element<'a, Message>
where
    Message: 'a,
{
    let spacing = 10.0 * scale;
    let mut r = row![].spacing(spacing).align_y(Alignment::Center);
    for btn in buttons {
        r = r.push(btn);
    }
    r.into()
}
