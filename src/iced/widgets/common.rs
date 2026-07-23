//! Reusable widget builders for consistent UI patterns
//!
//! This module provides helper functions to construct common UI elements
//! with consistent styling, reducing duplication across the codebase.

use iced::widget::{Button, Container, Row, button, column, container, image, row, svg, text};
use iced::{Alignment, Background, Border, Element, Length, Theme};
use shakmaty::{Chess, Position, Role};

use crate::core::outcome::GameResult;
use crate::iced::assets;
use crate::iced::style::{self, Palette, buttons};
use crate::iced::widgets::board::get_piece_image;

/// Wraps content in a dark overlay (for modals)
fn modal_overlay<'a, M: 'a>(content: impl Into<Element<'a, M>>) -> Element<'a, M> {
    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(style::containers::overlay)
        .into()
}

/// Creates a styled modal box container
fn modal_box<'a, M: 'a>(content: impl Into<Element<'a, M>>) -> Container<'a, M, Theme> {
    container(content)
        .padding(24)
        .style(style::containers::modal_box)
}

/// Full modal: overlay + centered box
pub fn modal<'a, M: 'a>(content: impl Into<Element<'a, M>>) -> Element<'a, M> {
    modal_overlay(modal_box(content))
}

/// Icon button with standard styling
pub fn icon_button<'a, M: Clone + 'a>(icon_path: &str, message: M) -> Button<'a, M, Theme> {
    let icon = svg(assets::icon(icon_path))
        .width(Length::Fixed(20.0))
        .height(Length::Fixed(20.0))
        .style(|theme, _| iced::widget::svg::Style {
            color: Some(Palette::text_primary(theme)),
        });

    button(icon)
        .padding([6, 10])
        .on_press(message)
        .style(buttons::icon)
}

/// Toggle button showing On/Off state with appropriate coloring
pub fn toggle_button<'a, M: Clone + 'a>(
    theme: &Theme,
    is_on: bool,
    message: M,
    scale: f32,
) -> Button<'a, M, Theme> {
    let label = if is_on { "On" } else { "Off" };
    let color = if is_on {
        Palette::success(theme)
    } else {
        Palette::text_muted(theme)
    };

    button(text(label).size(12.0 * scale).color(color))
        .padding([4.0 * scale, 12.0 * scale])
        .on_press(message)
        .style(buttons::toggle)
}

/// Cancel/Confirm button pair for dialogs
pub fn confirm_cancel_row<'a, M: Clone + 'a>(
    cancel_msg: M,
    confirm_msg: M,
    confirm_text: &'a str,
    scale: f32,
) -> Row<'a, M, Theme> {
    row![
        button(text("Cancel").size(14.0 * scale))
            .padding([8, 16])
            .style(buttons::secondary)
            .on_press(cancel_msg),
        button(text(confirm_text).size(14.0 * scale))
            .padding([8, 16])
            .style(buttons::primary)
            .on_press(confirm_msg),
    ]
    .spacing(12)
}

/// Keyboard shortcut badge (for hotkeys modal)
pub fn kbd<'a, M: 'a>(
    _theme: &Theme,
    content: impl Into<Element<'a, M>>,
) -> Container<'a, M, Theme> {
    container(content)
        .padding([4, 8])
        .style(style::containers::kbd_badge)
}

/// Settings row with label and toggle button
pub fn settings_toggle_row<'a, M: Clone + 'a>(
    theme: &Theme,
    label: &'a str,
    is_on: bool,
    message: M,
    scale: f32,
) -> Row<'a, M, Theme> {
    row![
        text(label)
            .size(13.0 * scale)
            .color(Palette::text_primary(theme)),
        iced::widget::Space::new().width(Length::Fill),
        toggle_button(theme, is_on, message, scale),
    ]
    .align_y(Alignment::Center)
}

/// Settings row with label and value button
pub fn settings_value_row<'a, M: Clone + 'a>(
    theme: &Theme,
    label: impl Into<String>,
    value: impl Into<String>,
    message: M,
    scale: f32,
) -> Row<'a, M, Theme> {
    row![
        text(label.into())
            .size(13.0 * scale)
            .color(Palette::text_primary(theme)),
        iced::widget::Space::new().width(Length::Fill),
        button(text(value.into()).size(12))
            .padding(8)
            .on_press(message)
            .style(buttons::toggle),
    ]
    .align_y(Alignment::Center)
}

/// Settings row with a read-only value.
pub fn settings_info_row<'a, M: Clone + 'a>(
    theme: &Theme,
    label: impl Into<String>,
    value: impl Into<String>,
    scale: f32,
) -> Row<'a, M, Theme> {
    row![
        text(label.into())
            .size(13.0 * scale)
            .color(Palette::text_primary(theme)),
        iced::widget::Space::new().width(Length::Fill),
        text(value.into())
            .size(12.0 * scale)
            .color(Palette::text_secondary(theme)),
    ]
    .align_y(Alignment::Center)
}

/// A prominent result banner for a terminal chess position.
pub fn game_result_banner<'a, M: 'a>(
    theme: &Theme,
    position: &Chess,
    scale: f32,
) -> Option<Element<'a, M>> {
    let result = GameResult::from_position(position)?;
    let description = match result {
        GameResult::WhiteWins => "Checkmate · White wins",
        GameResult::BlackWins => "Checkmate · Black wins",
        GameResult::Draw if position.is_stalemate() => "Draw · Stalemate",
        GameResult::Draw => "Draw · Insufficient material",
    };

    Some(
        container(
            column![
                text(result.notation())
                    .size(30.0 * scale)
                    .color(Palette::accent(theme)),
                text(description)
                    .size(12.0 * scale)
                    .color(Palette::text_primary(theme)),
            ]
            .spacing(3.0 * scale)
            .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .padding(12.0 * scale)
        .style(|theme| container::Style {
            background: Some(Background::Color(Palette::panel(theme))),
            border: Border {
                color: Palette::accent(theme),
                width: 1.5,
                radius: 8.0.into(),
            },
            ..Default::default()
        })
        .into(),
    )
}

/// Promotion piece picker shown as a modal overlay.
///
/// `is_white_turn` determines which color pieces to show.
/// `on_promote` maps a `Role` to the caller's message type.
/// `cancel_msg` is emitted when the cancel button is pressed.
pub fn promotion_modal<'a, M: Clone + 'a>(
    theme: &Theme,
    is_white_turn: bool,
    on_promote: impl Fn(Role) -> M + 'a,
    cancel_msg: M,
) -> Element<'a, M> {
    let (q, r, b, n) = if is_white_turn {
        ('Q', 'R', 'B', 'N')
    } else {
        ('q', 'r', 'b', 'n')
    };
    let piece_size: u32 = 48;
    let img_len = Length::Fixed(piece_size as f32);

    let mk_btn = |ch: char, role: Role| -> Element<'a, M> {
        if let Some(handle) = get_piece_image(ch, piece_size) {
            button(image(handle).width(img_len).height(img_len))
                .padding(4)
                .style(buttons::secondary)
                .on_press(on_promote(role))
                .into()
        } else {
            button(text(ch.to_string()).size(24))
                .padding(8)
                .style(buttons::secondary)
                .on_press(on_promote(role))
                .into()
        }
    };

    modal(
        column![
            text("Promote to:")
                .size(14)
                .color(Palette::text_primary(theme)),
            row![
                mk_btn(q, Role::Queen),
                mk_btn(r, Role::Rook),
                mk_btn(b, Role::Bishop),
                mk_btn(n, Role::Knight),
            ]
            .spacing(8),
            button(text("Cancel").size(11))
                .padding(4)
                .style(buttons::secondary)
                .on_press(cancel_msg),
        ]
        .spacing(12)
        .align_x(Alignment::Center),
    )
}
