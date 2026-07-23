//! Top navigation bar with mode picker and action buttons.
//!
//! Provides consistent navigation across all modes with:
//! - Mode picker dropdown (left)
//! - Contextual title (center)
//! - Settings/Help buttons (right)

use iced::widget::{Space, button, container, row, svg, text};
use iced::{Alignment, Element, Length, Theme};

use crate::iced::assets;
use crate::iced::style::{Palette, buttons, containers};
use crate::metadata;

use std::fmt::Display;

/// Available application modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModeType {
    #[default]
    QuickBoard,
    Study,
    GameReview,
    Trivia,
    Chessle,
}

impl Display for ModeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

impl ModeType {
    pub fn label(&self) -> &'static str {
        match self {
            ModeType::QuickBoard => "Quick Board",
            ModeType::Study => "Study",
            ModeType::GameReview => "Game Review",
            ModeType::Trivia => "Trivia",
            ModeType::Chessle => "Chessle",
        }
    }

    pub fn selectable_for(current_mode: ModeType) -> &'static [ModeType] {
        const STANDARD: &[ModeType] = &[
            ModeType::QuickBoard,
            ModeType::Study,
            ModeType::Trivia,
            ModeType::Chessle,
        ];
        const WITH_REVIEW_CURRENT: &[ModeType] = &[
            ModeType::QuickBoard,
            ModeType::Study,
            ModeType::GameReview,
            ModeType::Trivia,
            ModeType::Chessle,
        ];

        if current_mode == ModeType::GameReview {
            WITH_REVIEW_CURRENT
        } else {
            STANDARD
        }
    }
}

/// Messages from the top bar
#[derive(Debug, Clone)]
pub enum TopBarMessage {
    SelectMode(ModeType),
    ToggleSidebar,
    OpenHelp,
    OpenSettings,
    ToggleTheme,
}

/// Builds the top bar element
pub fn build_top_bar<'a, M: Clone + 'a>(
    theme: &Theme,
    current_mode: ModeType,
    map_msg: impl Fn(TopBarMessage) -> M + 'a + Copy,
) -> Element<'a, M> {
    let mode_picker = iced::widget::pick_list(
        ModeType::selectable_for(current_mode),
        Some(current_mode),
        move |mode| map_msg(TopBarMessage::SelectMode(mode)),
    )
    .width(Length::Fixed(160.0))
    .padding([8, 12])
    .style(|theme, _| iced::widget::pick_list::Style {
        text_color: Palette::text_primary(theme),
        placeholder_color: Palette::text_muted(theme),
        background: iced::Background::Color(Palette::surface(theme)),
        border: iced::Border {
            color: Palette::border(theme),
            width: 1.0,
            radius: 6.0.into(),
        },
        handle_color: Palette::text_muted(theme),
    });

    let sidebar_toggle: Element<'_, M> = {
        let icon = svg(assets::icon("menu"))
            .width(Length::Fixed(18.0))
            .height(Length::Fixed(18.0))
            .style(|theme, _| iced::widget::svg::Style {
                color: Some(Palette::text_primary(theme)),
            });

        row![
            Space::new().width(8),
            button(icon)
                .padding([8, 10])
                .style(buttons::icon)
                .on_press(map_msg(TopBarMessage::ToggleSidebar))
        ]
        .align_y(Alignment::Center)
        .into()
    };

    let title_element: Element<'_, M> = text(metadata::APP_NAME)
        .size(18) // slightly bigger
        .color(Palette::accent(theme)) // Orange accent
        .into();

    let help_icon = toolbar_icon("help");
    let settings_icon = toolbar_icon("settings");

    let help_btn = button(help_icon)
        .padding([6, 8])
        .style(buttons::icon)
        .on_press(map_msg(TopBarMessage::OpenHelp));

    let settings_btn = button(settings_icon)
        .padding([6, 8])
        .style(buttons::icon)
        .on_press(map_msg(TopBarMessage::OpenSettings));

    let theme_icon = svg(assets::icon(match theme {
        Theme::Light => "moon",
        _ => "sun",
    }))
    .width(Length::Fixed(18.0))
    .height(Length::Fixed(18.0))
    .style(|theme, _| iced::widget::svg::Style {
        color: Some(Palette::text_primary(theme)),
    });

    let theme_btn = button(theme_icon)
        .padding([6, 8])
        .style(buttons::icon)
        .on_press(map_msg(TopBarMessage::ToggleTheme));

    let bar = container(
        row![
            mode_picker,
            sidebar_toggle,
            Space::new().width(Length::Fill),
            title_element,
            Space::new().width(Length::Fill),
            row![help_btn, settings_btn, theme_btn].spacing(8),
        ]
        .align_y(Alignment::Center)
        .padding([0, 8]),
    )
    .width(Length::Fill)
    .padding([8, 12])
    .style(containers::panel(0.0));

    bar.into()
}

fn toolbar_icon<'a, M: Clone + 'a>(icon_name: &'static str) -> Element<'a, M> {
    svg(assets::icon(icon_name))
        .width(Length::Fixed(18.0))
        .height(Length::Fixed(18.0))
        .style(|theme, _| iced::widget::svg::Style {
            color: Some(Palette::text_primary(theme)),
        })
        .into()
}
