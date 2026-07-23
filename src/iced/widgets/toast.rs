//! Toast Notifications - Non-intrusive user feedback
//!
//! Provides a ToastManager for handling temporary messages (e.g. "Game Saved", "Invalid Move").

use crate::iced::style::Palette;
use iced::widget::{container, text};
use iced::{Alignment, Element, Length};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub enum ToastType {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub kind: ToastType,
    pub created_at: Instant,
    pub duration: Duration,
}

pub struct ToastManager {
    toasts: Vec<Toast>,
}

impl Default for ToastManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ToastManager {
    pub fn new() -> Self {
        Self { toasts: Vec::new() }
    }

    pub fn add(&mut self, message: String, kind: ToastType) {
        let duration = match kind {
            ToastType::Error => Duration::from_secs(4),
            ToastType::Warning => Duration::from_secs(4),
            _ => Duration::from_secs(2),
        };
        self.toasts.push(Toast {
            message,
            kind,
            created_at: Instant::now(),
            duration,
        });
    }

    pub fn update(&mut self) {
        let now = Instant::now();
        self.toasts
            .retain(|t| now.duration_since(t.created_at) < t.duration);
    }

    pub fn view<'a, Message: 'a + Clone + 'static>(
        &self,
        theme: &iced::Theme,
    ) -> Element<'a, Message> {
        if self.toasts.is_empty() {
            return iced::widget::column![].into();
        }

        let content: Vec<Element<'a, Message>> = self
            .toasts
            .iter()
            .map(|t| {
                let (bg, text_color) = match t.kind {
                    ToastType::Info => (Palette::panel(theme), Palette::text_primary(theme)),
                    ToastType::Success => (Palette::success(theme), Palette::background(theme)),
                    ToastType::Warning => (Palette::warning(theme), Palette::background(theme)),
                    ToastType::Error => (Palette::error(theme), Palette::background(theme)),
                };

                let c = container(text(t.message.clone()).size(14).color(text_color))
                    .padding([8, 12])
                    .style(move |_| container::Style {
                        background: Some(bg.into()),
                        border: iced::Border {
                            radius: 4.0.into(),
                            width: 1.0,
                            color: iced::Color::from_rgb(0.3, 0.3, 0.3),
                        },
                        text_color: Some(text_color),
                        ..Default::default()
                    });

                let e: Element<'a, Message> = c.into();
                e
            })
            .collect();

        let col = iced::widget::column(content)
            .spacing(10)
            .align_x(Alignment::End);

        container(col)
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::End)
            .align_y(Alignment::End)
            .into()
    }
}
