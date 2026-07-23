//! Move Ribbon - Clean horizontal move navigation
//!
//! A shared component for displaying chess moves in a clean, scannable ribbon format.
//! Used by linear game views and analysis modes with branches and notes.

use iced::widget::{Row, button, container, mouse_area, row, scrollable, svg, text};
use iced::{Background, Border, Color, Element, Length, Shadow, Theme};
use shakmaty::{Chess, Move, Position};

use crate::core::review::MoveQuality;
use crate::iced::style::Palette;
use crate::iced::widgets::review_assets;

const EDIT_ICON: &[u8] = include_bytes!("../../../assets/icons/edit.svg");
const BRANCH_ICON: &[u8] = include_bytes!("../../../assets/icons/branch.svg");

/// Configuration for a single move in the ribbon
#[derive(Clone)]
pub struct RibbonMove {
    pub san: String,
    pub move_index: usize, // 0-based index in the sequence
    pub is_white: bool,    // White's move (for move numbering)
    pub has_note: bool,    // Show note indicator
    pub has_branch: bool,  // Show branch indicator
    pub badge: Option<MoveQuality>,
}

/// Build a move ribbon from a linear sequence of moves
pub fn build_linear_ribbon<Message>(
    theme: &Theme,
    moves: &[Move],
    current_depth: usize,
    on_click: impl Fn(usize) -> Message + 'static,
    on_right_click: impl Fn(usize) -> Message + 'static,
) -> Element<'static, Message>
where
    Message: Clone + 'static,
{
    build_linear_ribbon_with_start(
        theme,
        moves,
        &Chess::default(),
        current_depth,
        on_click,
        on_right_click,
    )
}

/// Build a move ribbon from a linear sequence of moves, starting from a custom position
pub fn build_linear_ribbon_with_start<Message>(
    theme: &Theme,
    moves: &[Move],
    start_pos: &Chess,
    current_depth: usize,
    on_click: impl Fn(usize) -> Message + 'static,
    on_right_click: impl Fn(usize) -> Message + 'static,
) -> Element<'static, Message>
where
    Message: Clone + 'static,
{
    let mut pos = start_pos.clone();
    let mut ribbon_moves = Vec::new();

    let start_turn_is_black = pos.turn().is_black();
    let start_fullmove = pos.fullmoves().get() as usize;

    for (i, mv) in moves.iter().enumerate() {
        let san = shakmaty::san::San::from_move(&pos, *mv).to_string();
        let is_white = pos.turn().is_white();
        ribbon_moves.push(RibbonMove {
            san,
            move_index: i + 1, // This is just an ID for click
            is_white,
            has_note: false,
            has_branch: false,
            badge: None,
        });
        pos = pos.clone().play(*mv).expect("Ribbon moves should be valid");
    }

    build_ribbon(
        theme,
        ribbon_moves,
        current_depth,
        start_fullmove,
        start_turn_is_black,
        on_click,
        on_right_click,
    )
}

/// Build the actual ribbon UI
pub fn build_ribbon<Message>(
    theme: &Theme,
    moves: Vec<RibbonMove>,
    current_depth: usize,
    start_move_num: usize,
    start_with_black: bool, // If true, the first move in `moves` is Black's
    on_click: impl Fn(usize) -> Message + 'static,
    on_right_click: impl Fn(usize) -> Message + 'static,
) -> Element<'static, Message>
where
    Message: Clone + 'static,
{
    if moves.is_empty() {
        return container(text("No moves").size(12).color(Palette::text_muted(theme)))
            .padding(8)
            .into();
    }

    let on_click = std::sync::Arc::new(on_click);
    let on_right_click = std::sync::Arc::new(on_right_click);
    let mut elements: Vec<Element<'static, Message>> = Vec::new();
    let mut move_num = start_move_num;
    let mut i = 0;

    // If starting with Black, render "N... " then Black's move
    if start_with_black && i < moves.len() {
        elements.push(
            text(format!("{}...", move_num))
                .size(11)
                .color(Palette::text_muted(theme))
                .into(),
        );

        let mv = &moves[i];
        elements.push(build_move_button(
            theme,
            mv,
            mv.move_index,
            current_depth,
            on_click.clone(),
            on_right_click.clone(),
        ));
        i += 1;
        elements.push(container(text("")).width(Length::Fixed(8.0)).into());
        move_num += 1;
    }

    while i < moves.len() {
        // Move number
        elements.push(
            text(format!("{}.", move_num))
                .size(11)
                .color(Palette::text_muted(theme))
                .into(),
        );

        // White's move
        if i < moves.len() {
            let mv = &moves[i];
            // Safety check: expects White, but relies on caller correctness or simple alternation
            elements.push(build_move_button(
                theme,
                mv,
                mv.move_index,
                current_depth,
                on_click.clone(),
                on_right_click.clone(),
            ));
            i += 1;
        }

        // Black's move
        if i < moves.len() {
            let mv = &moves[i];
            elements.push(build_move_button(
                theme,
                mv,
                mv.move_index,
                current_depth,
                on_click.clone(),
                on_right_click.clone(),
            ));
            i += 1;
        }

        // Add spacing between pairs
        elements.push(container(text("")).width(Length::Fixed(8.0)).into());

        move_num += 1;
    }

    scrollable(Row::with_children(elements).spacing(3).wrap())
        .height(Length::Shrink)
        .width(Length::Fill)
        .into()
}

fn build_move_button<Message>(
    theme: &Theme,
    mv: &RibbonMove,
    depth: usize,
    current_depth: usize,
    on_click: std::sync::Arc<dyn Fn(usize) -> Message>,
    on_right_click: std::sync::Arc<dyn Fn(usize) -> Message>,
) -> Element<'static, Message>
where
    Message: Clone + 'static,
{
    let is_current = depth == current_depth;

    // Colors
    let text_color = if is_current {
        Palette::background(theme)
    } else if mv.is_white {
        Palette::text_primary(theme)
    } else {
        Palette::text_secondary(theme)
    };

    let mut content = row![text(mv.san.clone()).size(12).color(text_color)]
        .spacing(2)
        .align_y(iced::Alignment::Center);

    if let Some(quality) = mv.badge {
        let badge_color_value = review_assets::quality_color(theme, quality);
        content = content.push(
            svg(svg::Handle::from_memory(review_assets::icon_bytes(quality)))
                .width(Length::Fixed(11.0))
                .height(Length::Fixed(11.0))
                .style(move |_, _| svg::Style {
                    color: Some(badge_color_value),
                }),
        );
    }

    if mv.has_branch {
        content = content.push(
            svg(svg::Handle::from_memory(BRANCH_ICON))
                .width(Length::Fixed(10.0))
                .height(Length::Fixed(10.0))
                .style(move |_, _| svg::Style {
                    color: Some(text_color),
                }),
        );
    }
    if mv.has_note {
        content = content.push(
            svg(svg::Handle::from_memory(EDIT_ICON))
                .width(Length::Fixed(10.0))
                .height(Length::Fixed(10.0))
                .style(move |_, _| svg::Style {
                    color: Some(text_color),
                }),
        );
    }

    let bg_color = if is_current {
        Some(Palette::success(theme))
    } else {
        None
    };

    let border_color = Color::TRANSPARENT;
    let border_width = 0.0;

    let depth_clone = depth;
    let msg = on_click(depth);
    let msg_right = on_right_click(depth_clone);

    let btn = button(container(content).padding([2, 6]))
        .padding(0)
        .style(move |_, _| button::Style {
            background: bg_color.map(Background::Color),
            text_color,
            border: Border {
                color: border_color,
                width: border_width,
                radius: 4.0.into(),
            },
            shadow: Shadow::default(), // Fix blue outline
            ..Default::default()
        })
        .on_press(msg);

    mouse_area(btn).on_right_press(msg_right).into()
}
