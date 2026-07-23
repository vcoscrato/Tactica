use crate::core::config::AppSettings;
use crate::iced::style::containers;
use iced::widget::container;
use iced::{Element, Length};

/// A standardized layout for all game modes.
///
/// Supports two configurations via `responsive_layout`:
/// 1. **Board Only**: `info_panel` is None. Layout matches `Trivia`.
/// 2. **Board + Info**: `info_panel` is Some. Layout matches `Analysis` (3-pane in horizontal).
pub struct GameLayout<'a, Message> {
    board: Element<'a, Message>,
    info_panel: Option<Element<'a, Message>>, // Left/Middle panel (e.g., Move List, Eval)
    control_panel: Element<'a, Message>,      // Right/Bottom panel (e.g., Notes, Actions)
    settings: &'a AppSettings,
}

impl<'a, Message> GameLayout<'a, Message>
where
    Message: 'a + Clone,
{
    pub fn new(
        board: Element<'a, Message>,
        control_panel: Element<'a, Message>,
        settings: &'a AppSettings,
    ) -> Self {
        Self {
            board,
            info_panel: None,
            control_panel,
            settings,
        }
    }

    /// Add an info panel (e.g. for Analysis mode's move list/eval)
    pub fn with_info_panel(mut self, content: Element<'a, Message>) -> Self {
        self.info_panel = Some(content);
        self
    }

    pub fn view(self) -> Element<'a, Message> {
        // Wrap panels in standard styling
        // Note: We do NOT use scrollable here anymore.
        // The content itself must handle scrolling if needed.
        // This allows layouts to use Space::Fill to push buttons to the bottom.

        let control_styled = container(self.control_panel)
            .padding(15)
            .style(containers::panel(10.0))
            .width(Length::Fill)
            .height(Length::Fill); // Fill available space in the grid cell

        let info_styled = self.info_panel.map(|content| {
            container(content)
                .padding(15)
                .style(containers::panel(10.0))
                .width(Length::Fill)
                .height(Length::Fill) // Fill available space
                .into()
        });

        use crate::iced::layout;
        layout::responsive_layout(
            self.board,
            info_styled,
            control_styled.into(),
            self.settings,
        )
    }
}
