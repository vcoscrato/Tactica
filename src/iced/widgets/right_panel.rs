//! Right-side panel for Settings, Help, and other contextual content.
//!
//! This replaces the modal-based approach with a persistent, toggleable panel
//! that slides in from the right side of the screen.

use iced::widget::{Space, button, column, container, row, scrollable, svg, text};
use iced::{Alignment, Element, Length, Theme};

use crate::core::config::{AppSettings, EngineSettings, LayoutMode};
use crate::core::review::MoveQuality;
use crate::iced::assets;
use crate::iced::controls::GlobalHotkey;
use crate::iced::style::{Palette, buttons, containers};
use crate::iced::widgets::common::{
    kbd, settings_info_row, settings_toggle_row, settings_value_row,
};
use crate::iced::widgets::review_assets;
use crate::metadata;
use std::path::Path;

/// What content is displayed in the right panel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RightPanelTab {
    #[default]
    None,
    Settings,
    Help,
}

impl RightPanelTab {
    pub fn is_open(&self) -> bool {
        !matches!(self, RightPanelTab::None)
    }

    pub fn toggle(self, target: RightPanelTab) -> RightPanelTab {
        if self == target {
            RightPanelTab::None
        } else {
            target
        }
    }
}

pub struct RightPanelContext<'a> {
    pub settings: &'a AppSettings,
    pub instructions: String,
    pub hotkeys: Vec<(String, String)>,
    pub has_board: bool,
    pub is_analysis_mode: bool,
    pub is_game_review_mode: bool,
}

/// Messages that the right panel can emit
#[derive(Debug, Clone)]
pub enum RightPanelMessage {
    Close,
    SwitchTab(RightPanelTab),
    // Settings actions
    ToggleLayoutMode,
    ToggleAutoLayout,
    ToggleAnimationSpeed,
    ChooseLibraryRoot,
    // Engine settings (only in analysis mode)
    ToggleEngine,
    SetMultiPV(u32),
    SetMaxDepth(Option<u32>),
    SetThreads(u32),
    SetHashMB(u32),
    // Board Settings
    CycleBoardTheme,
    ToggleEvalBar,
}

/// Width of the right panel
pub const PANEL_WIDTH: f32 = 320.0;

/// Builds the right panel content based on the active tab
pub fn build_right_panel<'a, M: Clone + 'a>(
    theme: &Theme,
    tab: RightPanelTab,
    ctx: RightPanelContext<'a>,
    map_msg: impl Fn(RightPanelMessage) -> M + 'a + Copy,
) -> Element<'a, M> {
    if !tab.is_open() {
        return Space::new().width(0).into();
    }

    let header = build_header(theme, tab, map_msg);

    let content: Element<'_, M> = match tab {
        RightPanelTab::Settings => {
            build_settings_content(theme, ctx.settings, ctx.is_analysis_mode, map_msg)
        }
        RightPanelTab::Help => build_help_content(
            theme,
            ctx.instructions,
            ctx.hotkeys,
            ctx.has_board,
            ctx.settings.ui_scale,
            ctx.is_game_review_mode,
        ),
        RightPanelTab::None => Space::new().height(0).into(),
    };

    container(
        column![
            header,
            iced::widget::rule::horizontal(1),
            scrollable(container(content).padding([16, 0])).height(Length::Fill),
        ]
        .spacing(0),
    )
    .width(Length::Fixed(PANEL_WIDTH))
    .height(Length::Fill)
    .style(containers::panel(0.0))
    .into()
}

fn build_header<'a, M: Clone + 'a>(
    theme: &Theme,
    tab: RightPanelTab,
    map_msg: impl Fn(RightPanelMessage) -> M + 'a,
) -> Element<'a, M> {
    let title = match tab {
        RightPanelTab::Settings => "Settings",
        RightPanelTab::Help => "Help",
        RightPanelTab::None => "",
    };

    let close_icon = svg(assets::icon("close"))
        .width(Length::Fixed(16.0))
        .height(Length::Fixed(16.0))
        .style(|theme, _| iced::widget::svg::Style {
            color: Some(Palette::text_secondary(theme)),
        });

    let close_btn = button(close_icon)
        .padding([6, 8])
        .style(buttons::icon)
        .on_press(map_msg(RightPanelMessage::Close));

    container(
        row![
            text(title).size(18).color(Palette::text_primary(theme)),
            Space::new().width(Length::Fill),
            close_btn,
        ]
        .align_y(Alignment::Center),
    )
    .padding([12, 16])
    .into()
}

fn build_settings_content<'a, M: Clone + 'a>(
    theme: &Theme,
    settings: &AppSettings,
    is_analysis_mode: bool,
    map_msg: impl Fn(RightPanelMessage) -> M + 'a + Copy,
) -> Element<'a, M> {
    let s = settings.ui_scale;

    let layout_label = match settings.layout_mode {
        LayoutMode::Horizontal => "Horizontal",
        LayoutMode::Vertical => "Vertical",
    };

    let mut content = column![
        settings_value_row(
            theme,
            "Library",
            compact_path(&settings.library_root()),
            map_msg(RightPanelMessage::ChooseLibraryRoot),
            s
        ),
        settings_value_row(
            theme,
            "Layout",
            layout_label,
            map_msg(RightPanelMessage::ToggleLayoutMode),
            s
        ),
        settings_toggle_row(
            theme,
            "Auto Layout",
            settings.auto_layout,
            map_msg(RightPanelMessage::ToggleAutoLayout),
            s
        ),
        settings_value_row(
            theme,
            "Animation",
            settings.animation_speed.label(),
            map_msg(RightPanelMessage::ToggleAnimationSpeed),
            s
        ),
        settings_value_row(
            theme,
            "Board Theme",
            settings.board_theme.label(),
            map_msg(RightPanelMessage::CycleBoardTheme),
            s
        ),
        settings_info_row(theme, "Version", metadata::VERSION, s),
        settings_info_row(theme, "License", metadata::LICENSE, s),
    ]
    .spacing(8)
    .padding([0, 16]);

    // Engine settings section (only in analysis mode)
    if is_analysis_mode {
        content = content.push(Space::new().height(16));
        content = content.push(iced::widget::rule::horizontal(1));
        content = content.push(Space::new().height(16));
        content = content.push(build_engine_settings_section(theme, settings, map_msg));
    }

    content.into()
}

fn compact_path(path: &Path) -> String {
    let full = path.to_string_lossy();
    if full.len() <= 28 {
        return full.to_string();
    }

    let parts = path
        .components()
        .rev()
        .filter_map(|component| component.as_os_str().to_str())
        .take(2)
        .collect::<Vec<_>>();

    match parts.as_slice() {
        [leaf, parent, ..] => format!(".../{parent}/{leaf}"),
        [leaf] => format!(".../{leaf}"),
        _ => "...".to_string(),
    }
}

fn build_engine_settings_section<'a, M: Clone + 'a>(
    theme: &Theme,
    settings: &AppSettings,
    map_msg: impl Fn(RightPanelMessage) -> M + 'a + Copy,
) -> Element<'a, M> {
    let s = settings.ui_scale;
    let engine = &settings.engine;

    // Engine enabled toggle
    let enabled_row = settings_toggle_row(
        theme,
        "Engine",
        engine.enabled,
        map_msg(RightPanelMessage::ToggleEngine),
        s,
    );

    let eval_row = if engine.enabled {
        settings_toggle_row(
            theme,
            "Eval Bar",
            settings.show_eval_bar,
            map_msg(RightPanelMessage::ToggleEvalBar),
            s,
        )
    } else {
        // Disabled appearance
        row![
            text("Eval Bar")
                .size(12)
                .color(Palette::text_muted(theme))
                .width(Length::Fixed(120.0)),
            text("Off (Engine Disabled)")
                .size(10)
                .color(Palette::text_muted(theme))
        ]
        .padding([4, 0])
        .align_y(Alignment::Center)
    };

    fn option_row<'a, M: Clone + 'a>(
        theme: &Theme,
        label: &'a str,
        btns: Vec<Element<'a, M>>,
        suffix: Option<Element<'a, M>>,
    ) -> Element<'a, M> {
        let mut row = row![
            text(label)
                .size(12)
                .color(Palette::text_secondary(theme))
                .width(Length::Fixed(70.0)),
            iced::widget::row(btns).spacing(2),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        if let Some(suffix) = suffix {
            row = row.push(suffix);
        }
        row.into()
    }

    let lines_row: Element<'_, M> = {
        let btns = (1..=5)
            .map(|pv| {
                let is_selected = engine.multi_pv == pv;
                button(text(pv.to_string()).size(10))
                    .padding([2, 6])
                    .style(if is_selected {
                        buttons::primary
                    } else {
                        buttons::secondary
                    })
                    .on_press(map_msg(RightPanelMessage::SetMultiPV(pv)))
                    .into()
            })
            .collect();
        option_row(theme, "Lines", btns, None)
    };

    let depth_row: Element<'_, M> = {
        let btns = EngineSettings::available_depth_options()
            .iter()
            .map(|&d| {
                let is_selected = engine.max_depth == d;
                let label = EngineSettings::format_depth(d);
                button(text(label).size(10))
                    .padding([2, 6])
                    .style(if is_selected {
                        buttons::primary
                    } else {
                        buttons::secondary
                    })
                    .on_press(map_msg(RightPanelMessage::SetMaxDepth(d)))
                    .into()
            })
            .collect();
        option_row(theme, "Depth", btns, None)
    };

    let threads_row: Element<'_, M> = {
        let max_threads = EngineSettings::max_threads();
        let btns = EngineSettings::available_thread_options()
            .iter()
            .map(|&t| {
                let is_selected = engine.threads == t;
                button(text(t.to_string()).size(10))
                    .padding([2, 6])
                    .style(if is_selected {
                        buttons::primary
                    } else {
                        buttons::secondary
                    })
                    .on_press(map_msg(RightPanelMessage::SetThreads(t)))
                    .into()
            })
            .collect();
        option_row(
            theme,
            "Threads",
            btns,
            Some(
                text(format!("/{}", max_threads))
                    .size(10)
                    .color(Palette::text_muted(theme))
                    .into(),
            ),
        )
    };

    let hash_row: Element<'_, M> = {
        let btns = EngineSettings::available_hash_options()
            .iter()
            .map(|&h| {
                let is_selected = engine.hash_mb == h;
                button(text(h.to_string()).size(10))
                    .padding([2, 6])
                    .style(if is_selected {
                        buttons::primary
                    } else {
                        buttons::secondary
                    })
                    .on_press(map_msg(RightPanelMessage::SetHashMB(h)))
                    .into()
            })
            .collect();
        option_row(
            theme,
            "Hash",
            btns,
            Some(text("MB").size(10).color(Palette::text_muted(theme)).into()),
        )
    };

    column![
        enabled_row,
        eval_row,
        Space::new().height(4),
        lines_row,
        depth_row,
        threads_row,
        hash_row,
    ]
    .spacing(6)
    .into()
}

fn build_help_content<'a, M: 'a>(
    theme: &Theme,
    instructions: String,
    hotkeys: Vec<(String, String)>,
    has_board: bool,
    scale: f32,
    is_game_review_mode: bool,
) -> Element<'a, M> {
    let mut content = column![].spacing(8).padding([0, 16]);

    // Instructions section
    if !instructions.is_empty() {
        content = content.push(section_header(theme, "About This Mode"));
        content = content.push(
            text(instructions)
                .size(13.0 * scale)
                .color(Palette::text_secondary(theme)),
        );
        content = content.push(Space::new().height(16));
    }

    if is_game_review_mode {
        content = content.push(section_header(theme, "Review Legend"));
        for (quality, icon, color) in review_legend_items() {
            content = content.push(review_legend_row(
                theme,
                quality.label(),
                quality.description(),
                icon,
                color,
                scale,
            ));
        }
        content = content.push(Space::new().height(16));
    }

    // Mode-specific hotkeys
    if !hotkeys.is_empty() {
        content = content.push(section_header(theme, "Mode Shortcuts"));
        for (key, desc) in hotkeys.into_iter() {
            content = content.push(shortcut_row(theme, key, desc));
        }
        content = content.push(Space::new().height(16));
    }

    // Global hotkeys
    content = content.push(section_header(theme, "Global Shortcuts"));

    for hotkey in GlobalHotkey::all() {
        if !has_board && matches!(hotkey, GlobalHotkey::FlipBoard) {
            continue;
        }
        content = content.push(shortcut_row(theme, hotkey.shortcut(), hotkey.description()));
    }

    content.into()
}

fn review_legend_items() -> Vec<(MoveQuality, &'static [u8], iced::Color)> {
    use MoveQuality::*;
    let qualities = [
        Book, Brilliant, Great, Best, Excellent, Good, Inaccuracy, Mistake, Blunder, Missed,
    ];
    qualities
        .into_iter()
        .map(|q| {
            (
                q,
                review_assets::icon_bytes(q),
                review_assets::quality_color_fixed(q),
            )
        })
        .collect()
}

fn review_legend_row<'a, M: 'a>(
    theme: &Theme,
    label: &'a str,
    description: &'a str,
    icon: &'static [u8],
    color: iced::Color,
    scale: f32,
) -> Element<'a, M> {
    row![
        svg(iced::widget::svg::Handle::from_memory(icon))
            .width(Length::Fixed(14.0 * scale))
            .height(Length::Fixed(14.0 * scale))
            .style(move |_, _| iced::widget::svg::Style { color: Some(color) }),
        Space::new().width(8),
        column![
            text(label)
                .size(12.0 * scale)
                .color(Palette::text_primary(theme)),
            text(description)
                .size(10.0 * scale)
                .color(Palette::text_muted(theme)),
        ]
        .spacing(1),
    ]
    .align_y(Alignment::Center)
    .into()
}

fn section_header<'a, M: 'a>(theme: &Theme, title: &'a str) -> Element<'a, M> {
    text(title).size(14).color(Palette::accent(theme)).into()
}

fn shortcut_row<'a, M: 'a>(
    theme: &Theme,
    key: impl ToString,
    description: impl ToString,
) -> Element<'a, M> {
    row![
        kbd(
            theme,
            text(key.to_string()).size(11).color(Palette::accent(theme))
        ),
        Space::new().width(12),
        text(description.to_string())
            .size(12)
            .color(Palette::text_secondary(theme)),
    ]
    .align_y(Alignment::Center)
    .into()
}
