use iced::widget::{Space, button, column, container, row, svg, text};
use iced::{Color, Element, Length, Theme};
use shakmaty::{Chess, Move, Position, san::San};
use std::str::FromStr;

use crate::core::board::position_to_fen;
use crate::core::config::{AppSettings, EngineSettings};
use crate::core::engine::{Engine, EngineAnalysis};
use crate::iced::assets;
use crate::iced::style;
use crate::iced::style::Palette;
use crate::iced::style::buttons;

const EVAL_BAR_WIDTH: f32 = 24.0;
const EVAL_BAR_BOARD_GAP_RATIO: f32 = 0.018;
const EVAL_BAR_BOARD_GAP_MIN: f32 = 5.0;
const EVAL_BAR_BOARD_GAP_MAX: f32 = 10.0;
const EVAL_BAR_MIN_PCT: f32 = 0.01;
const EVAL_BAR_MAX_PCT: f32 = 0.99;

// ---------------------------------------------------------------------------
// Shared engine state for modes that embed a live analysis engine
// ---------------------------------------------------------------------------

/// Encapsulates the engine process, analysis results, and eval-bar animation
/// state that is duplicated across Study, QuickBoard, and GameReview modes.
pub struct EngineState {
    pub engine: Option<Engine>,
    pub analysis: EngineAnalysis,
    pub analyzing: bool,
    pub current_eval_pct: f32,
    pub error: Option<String>,
}

impl EngineState {
    /// Create a new engine state (does NOT spawn the engine process yet).
    pub fn new(enabled: bool) -> Self {
        Self {
            engine: None,
            analysis: EngineAnalysis::default(),
            analyzing: enabled,
            current_eval_pct: 0.5,
            error: None,
        }
    }

    /// (Re-)initialize the engine process from settings.
    /// If a previous engine exists it is stopped first.
    /// When `self.analyzing` is true *and* a position is provided the engine
    /// will immediately start analysing that position.
    pub fn init(&mut self, settings: &EngineSettings, position: Option<&Chess>) {
        if let Some(ref mut engine) = self.engine {
            let _ = engine.stop();
        }
        self.engine = None;
        self.error = None;

        match Engine::new_with_settings(settings) {
            Ok(engine) => {
                self.engine = Some(engine);
                if self.analyzing
                    && let Some(pos) = position
                {
                    self.start(pos);
                }
            }
            Err(e) => {
                self.error = Some(e);
            }
        }
    }

    /// Stop the engine process gracefully.
    pub fn shutdown(&mut self) {
        if let Some(ref mut engine) = self.engine {
            let _ = engine.stop();
        }
    }

    /// Begin infinite analysis of *position*.
    pub fn start(&mut self, position: &Chess) {
        let fen = position_to_fen(position);
        let is_white = position.turn().is_white();
        if let Some(ref mut engine) = self.engine {
            if let Err(e) = engine.stop() {
                self.error = Some(e);
                return;
            }
            if let Err(e) = engine.set_position(&fen, is_white) {
                self.error = Some(e);
                return;
            }
            if let Err(e) = engine.go() {
                self.error = Some(e);
                return;
            }
            self.error = None;
        } else if self.analyzing {
            self.error = Some("Stockfish is not running".to_string());
        }
    }

    /// Start analysis, creating Stockfish first when the UI has not initialized it yet.
    pub fn start_with_settings(&mut self, settings: &EngineSettings, position: &Chess) {
        if self.engine.is_none() {
            self.init(settings, Some(position));
        } else {
            self.start(position);
        }
    }

    pub fn ensure_running(&mut self, settings: &EngineSettings, position: &Chess) {
        if self.analyzing && self.engine.is_none() && self.error.is_none() {
            self.init(settings, Some(position));
        }
    }

    /// Stop the current analysis (but keep the engine alive).
    pub fn stop(&mut self) {
        if let Some(ref mut engine) = self.engine
            && let Err(e) = engine.stop()
        {
            self.error = Some(e);
        }
    }

    /// Toggle analysis on/off, persisting the preference to *settings*.
    pub fn toggle(&mut self, settings: &mut AppSettings, position: &Chess) {
        self.analyzing = !self.analyzing;
        settings.engine.enabled = self.analyzing;
        if let Err(e) = settings.save() {
            self.error = Some(e);
            return;
        }
        if self.analyzing {
            self.start_with_settings(&settings.engine, position);
        } else {
            self.stop();
            self.error = None;
        }
    }

    /// Poll the engine for new analysis results.
    pub fn poll(&mut self) {
        if let Some(ref mut engine) = self.engine
            && let Some(analysis) = engine.poll_analysis()
        {
            self.analysis = analysis.clone();
        }
    }

    /// Advance the smooth eval-bar animation by one tick.
    pub fn tick_eval_bar(&mut self, show_eval_bar: bool) {
        if show_eval_bar && self.analyzing {
            let target = target_eval_pct(&self.analysis);
            self.current_eval_pct += (target - self.current_eval_pct) * style::EVAL_LERP_FACTOR;
        }
    }

    /// Apply new settings: sync the `analyzing` flag and reinitialize the
    /// engine.  The caller must also update board theme / animation speed
    /// separately.
    pub fn apply_settings(&mut self, settings: &AppSettings, position: Option<&Chess>) {
        self.analyzing = settings.engine.enabled;
        if !self.analyzing {
            self.stop();
            self.engine = None;
            self.error = None;
        } else if self.engine.is_some() {
            self.init(&settings.engine, position);
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EngineControlsState<'a> {
    pub ui_scale: f32,
    pub analyzing: bool,
    pub current_depth: u32,
    pub max_depth: Option<u32>,
    pub error: Option<&'a str>,
}

pub fn engine_controls_row<Message>(
    theme: &Theme,
    state: EngineControlsState<'_>,
    toggle_msg: Message,
    open_settings_msg: Message,
) -> Element<'static, Message>
where
    Message: Clone + 'static,
{
    let toggle = button(
        text(if state.analyzing {
            "Engine On"
        } else {
            "Engine Off"
        })
        .size(11.0 * state.ui_scale),
    )
    .padding([4, 8])
    .style(if state.analyzing {
        buttons::primary
    } else {
        buttons::secondary
    })
    .on_press(toggle_msg);

    let settings_icon = svg(assets::icon("settings"))
        .width(Length::Fixed(14.0))
        .height(Length::Fixed(14.0))
        .style(|theme, _| iced::widget::svg::Style {
            color: Some(Palette::text_secondary(theme)),
        });

    let settings_btn = button(settings_icon)
        .padding([4, 6])
        .style(buttons::icon)
        .on_press(open_settings_msg);

    let depth_indicator: Element<'static, Message> = if let Some(error) = state.error {
        text(error.to_string())
            .size(10.0 * state.ui_scale)
            .color(Palette::error(theme))
            .into()
    } else if state.analyzing {
        let depth_label = match state.max_depth {
            Some(max) => format!("Depth = {} / {}", state.current_depth, max),
            None => format!("Depth = {} / MAX", state.current_depth),
        };

        text(depth_label)
            .size(10.0 * state.ui_scale)
            .color(Palette::text_muted(theme))
            .into()
    } else {
        Space::new().width(0).into()
    };

    row![
        toggle,
        container(depth_indicator)
            .width(Length::Fill)
            .center_x(Length::Fill),
        settings_btn
    ]
    .align_y(iced::Alignment::Center)
    .spacing(6)
    .into()
}

fn build_eval_bar_slot<Message>(
    analysis: &EngineAnalysis,
    pct: f32,
    ui_scale: f32,
    board_size: f32,
    visible: bool,
) -> Element<'static, Message>
where
    Message: Clone + 'static,
{
    let bar_width = eval_bar_width(ui_scale);

    if visible {
        container(build_vertical_eval_bar(analysis, pct, ui_scale))
            .width(Length::Fixed(bar_width))
            .height(Length::Fixed(board_size))
            .into()
    } else {
        container(Space::new().height(Length::Fill))
            .width(Length::Fixed(bar_width))
            .height(Length::Fixed(board_size))
            .into()
    }
}

pub fn build_board_eval_area<'a, Message>(
    board: Element<'a, Message>,
    analysis: &EngineAnalysis,
    pct: f32,
    ui_scale: f32,
    board_size: f32,
    visible: bool,
) -> Element<'a, Message>
where
    Message: Clone + 'static,
{
    let eval_bar_slot = build_eval_bar_slot(analysis, pct, ui_scale, board_size, visible);
    let board_area_width = board_eval_area_width(ui_scale, board_size);
    let gap = eval_bar_board_gap(ui_scale, board_size);

    let board_pair = row![eval_bar_slot, board]
        .spacing(gap)
        .align_y(iced::Alignment::Center)
        .width(Length::Fixed(board_area_width))
        .height(Length::Fixed(board_size));

    container(board_pair)
        .width(Length::Fill)
        .height(Length::Fixed(board_size))
        .align_x(iced::Alignment::Center)
        .align_y(iced::Alignment::Center)
        .into()
}

fn board_eval_area_width(ui_scale: f32, board_size: f32) -> f32 {
    eval_bar_width(ui_scale) + eval_bar_board_gap(ui_scale, board_size) + board_size
}

pub fn fit_board_size_for_eval_area_width(ui_scale: f32, available_width: f32) -> f32 {
    let available = (available_width - eval_bar_width(ui_scale)).max(0.0);
    let mut low = 0.0;
    let mut high = available;

    for _ in 0..20 {
        let mid = (low + high) / 2.0;
        let width = mid + eval_bar_board_gap(ui_scale, mid);

        if width <= available {
            low = mid;
        } else {
            high = mid;
        }
    }

    low
}

fn eval_bar_width(ui_scale: f32) -> f32 {
    EVAL_BAR_WIDTH * ui_scale
}

fn eval_bar_board_gap(ui_scale: f32, board_size: f32) -> f32 {
    (board_size * EVAL_BAR_BOARD_GAP_RATIO).clamp(
        EVAL_BAR_BOARD_GAP_MIN * ui_scale,
        EVAL_BAR_BOARD_GAP_MAX * ui_scale,
    )
}

pub fn target_eval_pct(analysis: &EngineAnalysis) -> f32 {
    let best_line = analysis.best_line();
    let (score_cp, mate_in) = if let Some(line) = best_line {
        (line.score_cp, line.mate_in)
    } else {
        (Some(0), None)
    };

    if let Some(m) = mate_in {
        if m > 0 {
            EVAL_BAR_MAX_PCT
        } else {
            EVAL_BAR_MIN_PCT
        }
    } else if let Some(c) = score_cp {
        (1.0 / (1.0 + (-(c as f32) / 400.0).exp())).clamp(EVAL_BAR_MIN_PCT, EVAL_BAR_MAX_PCT)
    } else {
        0.5
    }
}

pub fn build_engine_lines<Message>(
    theme: &Theme,
    ui_scale: f32,
    pos: &Chess,
    analysis: &EngineAnalysis,
    on_play_line: fn(usize) -> Message,
) -> Element<'static, Message>
where
    Message: Clone + 'static,
{
    if analysis.lines.is_empty() {
        return text("").into();
    }

    let items: Vec<Element<'static, Message>> = analysis
        .lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let eval = format_eval(line.score_cp, line.mate_in);
            let pv = format_pv_san(pos, &line.pv, 5);
            button(
                row![
                    text(eval)
                        .size(11.0 * ui_scale)
                        .color(Palette::text_primary(theme))
                        .width(Length::Fixed(45.0)),
                    text(pv)
                        .size(9.0 * ui_scale)
                        .color(Palette::text_secondary(theme)),
                ]
                .spacing(3),
            )
            .padding([3, 5])
            .width(Length::Fill)
            .style(|theme, _| button::Style {
                background: Some(iced::Background::Color(Palette::panel(theme))),
                border: iced::Border {
                    radius: 3.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .on_press(on_play_line(i))
            .into()
        })
        .collect();

    column(items).spacing(2).into()
}

pub fn build_vertical_eval_bar<Message>(
    analysis: &EngineAnalysis,
    pct: f32,
    ui_scale: f32,
) -> Element<'static, Message>
where
    Message: Clone + 'static,
{
    let best_line = analysis.best_line();
    let (score_cp, mate_in) = if let Some(line) = best_line {
        (line.score_cp, line.mate_in)
    } else {
        (Some(0), None)
    };

    let eval_text = format_eval(score_cp, mate_in);
    let bar_width = eval_bar_width(ui_scale);
    let white_height = Length::FillPortion((pct * 1000.0) as u16);
    let black_height = Length::FillPortion(((1.0 - pct) * 1000.0) as u16);

    let bar = column![
        container(
            text(if pct < 0.5 {
                eval_text.clone()
            } else {
                String::new()
            })
            .size(10)
            .color(Color::WHITE)
            .align_x(iced::alignment::Horizontal::Center)
        )
        .width(Length::Fill)
        .height(black_height)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(Color::from_rgb(0.2, 0.2, 0.2))),
            ..Default::default()
        })
        .align_y(iced::alignment::Vertical::Bottom)
        .padding(2),
        container(
            text(if pct >= 0.5 { eval_text } else { String::new() })
                .size(10)
                .color(Color::BLACK)
                .align_x(iced::alignment::Horizontal::Center)
        )
        .width(Length::Fill)
        .height(white_height)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(Color::WHITE)),
            ..Default::default()
        })
        .align_y(iced::alignment::Vertical::Top)
        .padding(2)
    ];

    container(bar)
        .width(Length::Fixed(bar_width))
        .height(Length::Fill)
        .into()
}

pub fn parse_uci_move(position: &Chess, uci: &str) -> Option<Move> {
    shakmaty::uci::UciMove::from_str(uci)
        .ok()
        .and_then(|u| u.to_move(position).ok())
}

pub fn format_eval(cp: Option<i32>, mate: Option<i32>) -> String {
    if let Some(m) = mate {
        if m > 0 {
            format!("M{}", m)
        } else {
            format!("-M{}", -m)
        }
    } else if let Some(c) = cp {
        let p = c as f32 / 100.0;
        if p >= 0.0 {
            format!("+{:.1}", p)
        } else {
            format!("{:.1}", p)
        }
    } else {
        "0.0".into()
    }
}

pub fn format_pv_san(pos: &Chess, pv: &[String], max: usize) -> String {
    let mut rendered = Vec::new();
    let mut p = pos.clone();
    for uci in pv.iter().take(max) {
        if let Some(mv) = parse_uci_move(&p, uci) {
            rendered.push(San::from_move(&p, mv).to_string());
            p = p.play(mv).unwrap();
        } else {
            break;
        }
    }
    rendered.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::engine::AnalysisLine;

    fn analysis_with_mate(mate_in: i32) -> EngineAnalysis {
        EngineAnalysis {
            lines: vec![AnalysisLine {
                mate_in: Some(mate_in),
                ..AnalysisLine::default()
            }],
            ..EngineAnalysis::default()
        }
    }

    #[test]
    fn mate_evaluations_keep_both_bar_segments_visible() {
        assert_eq!(target_eval_pct(&analysis_with_mate(1)), EVAL_BAR_MAX_PCT);
        assert_eq!(target_eval_pct(&analysis_with_mate(-1)), EVAL_BAR_MIN_PCT);
    }

    #[test]
    fn mate_evaluations_keep_the_mate_label() {
        assert_eq!(format_eval(None, Some(1)), "M1");
        assert_eq!(format_eval(None, Some(-3)), "-M3");
    }
}
