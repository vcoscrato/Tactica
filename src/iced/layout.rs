use crate::core::config::{AppSettings, LayoutMode};
use crate::iced::style::Palette;
use iced::widget::{column, container, row};
use iced::{Element, Length};

pub const HORIZONTAL_BOARD_PORTION: u16 = 13;
pub const HORIZONTAL_SIDEBAR_PORTION: u16 = 7;

pub fn layout_spacing(ui_scale: f32) -> f32 {
    20.0 * ui_scale
}

pub fn board_cell_size(
    layout_mode: LayoutMode,
    ui_scale: f32,
    content_width: f32,
    content_height: f32,
) -> (f32, f32) {
    let spacing = layout_spacing(ui_scale);
    let inner_width = (content_width - (spacing * 2.0)).max(0.0);
    let inner_height = (content_height - (spacing * 2.0)).max(0.0);

    match layout_mode {
        LayoutMode::Horizontal => {
            let row_width = (inner_width - spacing).max(0.0);
            let total_portions = HORIZONTAL_BOARD_PORTION + HORIZONTAL_SIDEBAR_PORTION;
            let board_width = row_width * (HORIZONTAL_BOARD_PORTION as f32 / total_portions as f32);

            (board_width, inner_height)
        }
        LayoutMode::Vertical => {
            let column_height = (inner_height - spacing).max(0.0);

            (inner_width, column_height / 2.0)
        }
    }
}

/// A responsive layout that positions the board and content based on LayoutMode.
///
/// In Horizontal mode: Board on left, content columns stacked vertically on right.
/// In Vertical mode: Board centered on top, two columns below side-by-side.
///
/// Special handling for empty `left_content` (2-pane mode):
/// If `left_content` is Space(0) or empty, `right_content` fills the entire sidebar/bottom area.
pub fn responsive_layout<'a, Message>(
    board: Element<'a, Message>,
    left_content: Option<Element<'a, Message>>,
    right_content: Element<'a, Message>,
    settings: &AppSettings,
) -> Element<'a, Message>
where
    Message: 'a,
{
    let spacing = layout_spacing(settings.ui_scale);

    let compact_vertical =
        matches!(settings.layout_mode, LayoutMode::Vertical) && settings.board_size < 380.0;

    let layout: Element<'a, Message> = match settings.layout_mode {
        LayoutMode::Horizontal => {
            // Horizontal: board on left, content stacked vertically on right.

            let sidebar = if let Some(left) = left_content {
                column![
                    container(left).height(Length::FillPortion(1)),
                    container(right_content).height(Length::FillPortion(1))
                ]
                .spacing(spacing)
                .height(Length::Fill)
            } else {
                column![container(right_content).height(Length::Fill)]
                    .spacing(spacing)
                    .height(Length::Fill)
            };

            row![
                container(board)
                    .center_y(Length::Fill)
                    .width(Length::FillPortion(HORIZONTAL_BOARD_PORTION)), // Center board vertically, give it more space (13/20 ~ 65%)
                container(sidebar).width(Length::FillPortion(HORIZONTAL_SIDEBAR_PORTION)) // Sidebar gets 7/20 ~ 35%
            ]
            .spacing(spacing)
            .into()
        }
        LayoutMode::Vertical => {
            // Vertical: board centered on top, panels below. Stack on compact widths.

            let bottom_section: Element<'a, Message> = if compact_vertical {
                if let Some(left) = left_content {
                    column![
                        container(right_content).height(Length::FillPortion(1)),
                        container(left).height(Length::FillPortion(1)),
                    ]
                    .spacing(spacing)
                    .height(Length::FillPortion(1))
                    .into()
                } else {
                    column![container(right_content).height(Length::Fill)]
                        .spacing(spacing)
                        .height(Length::FillPortion(1))
                        .into()
                }
            } else if let Some(left) = left_content {
                row![
                    container(left)
                        .width(Length::FillPortion(1))
                        .height(Length::Fill),
                    container(right_content)
                        .width(Length::FillPortion(1))
                        .height(Length::Fill)
                ]
                .spacing(spacing)
                .height(Length::FillPortion(1)) // Bottom section gets 1/2 height
                .into()
            } else {
                row![
                    container(right_content)
                        .width(Length::Fill)
                        .height(Length::Fill)
                ]
                .spacing(spacing)
                .height(Length::FillPortion(1))
                .into()
            };

            column![
                container(board)
                    .center_x(Length::Fill)
                    .height(Length::FillPortion(1)), // Board gets 1/2 height
                bottom_section
            ]
            .spacing(spacing)
            .into()
        }
    };

    container(layout)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(spacing)
        .style(|theme| container::Style {
            background: Some(Palette::background(theme).into()),
            ..Default::default()
        })
        .into()
}
