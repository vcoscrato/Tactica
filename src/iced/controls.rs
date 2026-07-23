use iced::keyboard::{Key, Modifiers, key::Named};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalHotkey {
    Save,
    NewGame, // Context sensitive: New Analysis, New Trivia, etc.
    Import,
    ToggleLibrary,
    NavigateHome,
    NavigateEnd,
    FlipBoard,
    ClosePanel,
}

impl GlobalHotkey {
    pub fn all() -> &'static [Self] {
        &[
            Self::Save,
            Self::NewGame,
            Self::Import,
            Self::ToggleLibrary,
            Self::NavigateHome,
            Self::NavigateEnd,
            Self::FlipBoard,
            Self::ClosePanel,
        ]
    }

    pub fn matches(&self, key: &Key, modifiers: Modifiers) -> bool {
        match self {
            Self::Save => modifiers.control() && is_key(key, "s"),
            Self::NewGame => modifiers.control() && is_key(key, "n"),
            Self::Import => modifiers.control() && is_key(key, "i"),
            Self::ToggleLibrary => modifiers.control() && is_key(key, "b"),
            Self::NavigateHome => matches!(key, Key::Named(Named::Home)),
            Self::NavigateEnd => matches!(key, Key::Named(Named::End)),
            Self::FlipBoard => !modifiers.control() && is_key(key, "f"), // F is usually just F, not Ctrl+F
            Self::ClosePanel => matches!(key, Key::Named(Named::Escape)),
        }
    }

    pub fn shortcut(&self) -> String {
        match self {
            Self::Save => "Ctrl+S".into(),
            Self::NewGame => "Ctrl+N".into(),
            Self::Import => "Ctrl+I".into(),
            Self::ToggleLibrary => "Ctrl+B".into(),
            Self::NavigateHome => "Home".into(),
            Self::NavigateEnd => "End".into(),
            Self::FlipBoard => "F".into(),
            Self::ClosePanel => "Esc".into(),
        }
    }

    pub fn description(&self) -> String {
        match self {
            Self::Save => "Save Analysis".into(),
            Self::NewGame => "New Game / Analysis".into(),
            Self::Import => "Import PGN".into(),
            Self::ToggleLibrary => "Toggle Sidebar".into(),
            Self::NavigateHome => "Go to Start".into(),
            Self::NavigateEnd => "Go to End".into(),
            Self::FlipBoard => "Flip Board".into(),
            Self::ClosePanel => "Close Panel".into(),
        }
    }
}

fn is_key(key: &Key, char_code: &str) -> bool {
    if let Key::Character(c) = key {
        c.to_lowercase() == char_code.to_lowercase()
    } else {
        false
    }
}
