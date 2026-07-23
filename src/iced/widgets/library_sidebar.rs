//! Library sidebar for browsing and managing study/review files.

use iced::widget::{Space, button, column, container, row, scrollable, svg, text, text_input};
use iced::{Alignment, Element, Length, Theme};
use std::path::PathBuf;

use crate::core::library::{EntryKind, Library, LibraryEntry};
use crate::iced::assets;
use crate::iced::style::{Palette, buttons, containers};

pub const SIDEBAR_WIDTH: f32 = 260.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LibraryFilter {
    #[default]
    All,
    Studies,
    Reviews,
    Favorites,
}

#[derive(Debug, Clone)]
pub enum LibrarySidebarMessage {
    Toggle,
    SearchChanged(String),
    ToggleFolder(PathBuf),
    OpenFile(PathBuf, EntryKind),
    CreateNew,
    CreateFolder,
    Import,
    Refresh,
    DeleteEntry(PathBuf),
    ToggleFavorite(PathBuf),
    SetFilter(LibraryFilter),
}

pub fn build_library_sidebar<'a, M: Clone + 'a>(
    theme: &Theme,
    library: &'a Library,
    search_query: &'a str,
    is_open: bool,
    recent: &'a [PathBuf],
    filter: LibraryFilter,
    map_msg: impl Fn(LibrarySidebarMessage) -> M + 'a + Copy,
) -> Element<'a, M> {
    if !is_open {
        return Space::new().width(0).into();
    }

    let collapse_icon = svg(assets::icon("close"))
        .width(Length::Fixed(14.0))
        .height(Length::Fixed(14.0))
        .style(|theme, _| iced::widget::svg::Style {
            color: Some(Palette::text_secondary(theme)),
        });

    let header = container(
        row![
            text("Library").size(14).color(Palette::text_primary(theme)),
            Space::new().width(Length::Fill),
            button(collapse_icon)
                .padding([4, 6])
                .style(buttons::icon)
                .on_press(map_msg(LibrarySidebarMessage::Toggle)),
        ]
        .align_y(Alignment::Center),
    )
    .padding([10, 12]);

    let filter_tabs = row![
        filter_tab(
            theme,
            "All",
            filter == LibraryFilter::All,
            map_msg(LibrarySidebarMessage::SetFilter(LibraryFilter::All))
        ),
        filter_tab(
            theme,
            "Studies",
            filter == LibraryFilter::Studies,
            map_msg(LibrarySidebarMessage::SetFilter(LibraryFilter::Studies))
        ),
        filter_tab(
            theme,
            "Reviews",
            filter == LibraryFilter::Reviews,
            map_msg(LibrarySidebarMessage::SetFilter(LibraryFilter::Reviews))
        ),
        filter_icon_tab(
            theme,
            filter == LibraryFilter::Favorites,
            map_msg(LibrarySidebarMessage::SetFilter(LibraryFilter::Favorites))
        ),
    ]
    .spacing(6);

    let search = container(
        text_input("Search...", search_query)
            .size(12)
            .padding([6, 10])
            .on_input(move |s| map_msg(LibrarySidebarMessage::SearchChanged(s))),
    )
    .padding([8, 12]);

    let actions = container(
        row![
            button(text("+ New").size(11))
                .padding([6, 12])
                .style(buttons::primary)
                .on_press(map_msg(LibrarySidebarMessage::CreateNew)),
            button(text("Import").size(11))
                .padding([6, 12])
                .style(buttons::secondary)
                .on_press(map_msg(LibrarySidebarMessage::Import)),
        ]
        .spacing(8),
    )
    .padding([8, 12]);

    let existing_recent: Vec<&PathBuf> = recent.iter().filter(|p| p.exists()).take(5).collect();
    let recent_section: Element<'_, M> = if !existing_recent.is_empty() {
        let recent_items: Vec<Element<'_, M>> = existing_recent
            .iter()
            .filter_map(|path| {
                let kind = library.kind_for_path(path)?;
                if !filter_accepts(kind, library.is_favorite(path), filter) {
                    return None;
                }

                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Unknown")
                    .to_string();
                let path_clone = (*path).clone();
                let favorite = library.is_favorite(path);

                let row = row![
                    kind_badge(theme, kind),
                    text(name).size(12).color(Palette::text_secondary(theme)),
                    if favorite {
                        favorite_icon(true, theme)
                    } else {
                        Space::new().width(Length::Fixed(12.0)).into()
                    }
                ]
                .spacing(6)
                .align_y(Alignment::Center);

                Some(
                    button(row)
                        .padding([4, 12])
                        .width(Length::Fill)
                        .style(file_button_style)
                        .on_press(map_msg(LibrarySidebarMessage::OpenFile(path_clone, kind)))
                        .into(),
                )
            })
            .collect();

        if recent_items.is_empty() {
            Space::new().height(0).into()
        } else {
            column![
                container(text("Recent").size(14).color(Palette::text_muted(theme)))
                    .padding([8, 12]),
                column(recent_items).spacing(2),
            ]
            .into()
        }
    } else {
        Space::new().height(0).into()
    };

    let tree_content = if search_query.is_empty() {
        build_tree_view(theme, &library.entries, library, filter, map_msg, 0)
    } else {
        let (kind_filter, favorites_only) = filter_to_params(filter);
        let results = library.search(search_query, kind_filter, favorites_only);
        let items: Vec<Element<'_, M>> = results
            .iter()
            .filter(|entry| !entry.is_folder())
            .map(|entry| build_entry_row(theme, entry, library, filter, map_msg, 0))
            .collect();

        if items.is_empty() {
            column![
                container(
                    text("No results")
                        .size(12)
                        .color(Palette::text_muted(theme))
                )
                .padding([16, 12])
            ]
            .into()
        } else {
            column(items).spacing(2).into()
        }
    };

    let tree_scrollable = scrollable(container(tree_content).padding([4, 0])).height(Length::Fill);

    container(
        column![
            header,
            iced::widget::rule::horizontal(1),
            actions,
            container(filter_tabs).padding([8, 12]),
            recent_section,
            Space::new().height(8),
            iced::widget::rule::horizontal(1),
            container(text("Files").size(14).color(Palette::text_muted(theme))).padding([8, 12]),
            search,
            tree_scrollable,
        ]
        .spacing(0),
    )
    .width(Length::Fixed(SIDEBAR_WIDTH))
    .height(Length::Fill)
    .style(containers::panel(0.0))
    .into()
}

fn filter_to_params(filter: LibraryFilter) -> (Option<EntryKind>, bool) {
    match filter {
        LibraryFilter::All => (None, false),
        LibraryFilter::Studies => (Some(EntryKind::Study), false),
        LibraryFilter::Reviews => (Some(EntryKind::Review), false),
        LibraryFilter::Favorites => (None, true),
    }
}

fn filter_accepts(kind: EntryKind, favorite: bool, filter: LibraryFilter) -> bool {
    match filter {
        LibraryFilter::All => true,
        LibraryFilter::Studies => kind == EntryKind::Study,
        LibraryFilter::Reviews => kind == EntryKind::Review,
        LibraryFilter::Favorites => favorite,
    }
}

fn build_tree_view<'a, M: Clone + 'a>(
    theme: &Theme,
    entries: &'a [LibraryEntry],
    library: &'a Library,
    filter: LibraryFilter,
    map_msg: impl Fn(LibrarySidebarMessage) -> M + 'a + Copy,
    depth: usize,
) -> Element<'a, M> {
    let items: Vec<Element<'_, M>> = entries
        .iter()
        .map(|entry| build_entry_row(theme, entry, library, filter, map_msg, depth))
        .collect();
    column(items).spacing(2).into()
}

fn build_entry_row<'a, M: Clone + 'a>(
    theme: &Theme,
    entry: &'a LibraryEntry,
    library: &'a Library,
    filter: LibraryFilter,
    map_msg: impl Fn(LibrarySidebarMessage) -> M + 'a + Copy,
    depth: usize,
) -> Element<'a, M> {
    let indent = (12 + depth * 16) as f32;

    match entry {
        LibraryEntry::Folder {
            name,
            path,
            children,
        } => {
            let is_expanded = library.is_expanded(path);
            let arrow = if is_expanded { "v" } else { ">" };
            let path_clone = path.clone();

            let folder_row = button(
                row![
                    text(arrow).size(10).color(Palette::text_muted(theme)),
                    Space::new().width(4),
                    text(name).size(12).color(Palette::text_primary(theme)),
                ]
                .align_y(Alignment::Center),
            )
            .padding([4.0, indent])
            .width(Length::Fill)
            .style(folder_button_style)
            .on_press(map_msg(LibrarySidebarMessage::ToggleFolder(path_clone)));

            if is_expanded && !children.is_empty() {
                column![
                    folder_row,
                    build_tree_view(theme, children, library, filter, map_msg, depth + 1),
                ]
                .into()
            } else {
                folder_row.into()
            }
        }
        LibraryEntry::File {
            name,
            path,
            kind,
            favorite,
            ..
        } => {
            if !filter_accepts(*kind, *favorite, filter) {
                return Space::new().height(0).into();
            }

            let path_clone = path.clone();
            let delete_path = path.clone();
            let fav_path = path.clone();

            let delete_icon = svg(assets::icon("trash"))
                .width(Length::Fixed(12.0))
                .height(Length::Fixed(12.0))
                .style(|theme, _| iced::widget::svg::Style {
                    color: Some(Palette::text_muted(theme)),
                });

            let delete_btn = button(delete_icon)
                .padding([2, 4])
                .style(buttons::icon)
                .on_press(map_msg(LibrarySidebarMessage::DeleteEntry(delete_path)));

            let favorite_btn = button(favorite_icon(*favorite, theme))
                .padding([2, 4])
                .style(buttons::icon)
                .on_press(map_msg(LibrarySidebarMessage::ToggleFavorite(fav_path)));

            let file_btn = button(
                row![
                    kind_badge(theme, *kind),
                    text(name).size(12).color(Palette::text_secondary(theme)),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding([4, 0])
            .style(file_button_style)
            .on_press(map_msg(LibrarySidebarMessage::OpenFile(path_clone, *kind)));

            container(
                row![
                    file_btn,
                    Space::new().width(Length::Fill),
                    favorite_btn,
                    delete_btn
                ]
                .align_y(Alignment::Center),
            )
            .padding([0.0, indent])
            .width(Length::Fill)
            .into()
        }
    }
}

fn filter_tab<'a, M: Clone + 'a>(
    _theme: &Theme,
    label: &'a str,
    selected: bool,
    message: M,
) -> Element<'a, M> {
    button(text(label).size(11))
        .padding([4, 8])
        .style(move |theme, status| {
            if selected {
                buttons::primary(theme, status)
            } else {
                buttons::secondary(theme, status)
            }
        })
        .on_press(message)
        .into()
}

fn filter_icon_tab<'a, M: Clone + 'a>(theme: &Theme, selected: bool, message: M) -> Element<'a, M> {
    button(favorite_filter_icon(selected, theme))
        .padding([4, 8])
        .style(move |theme, status| {
            if selected {
                buttons::primary(theme, status)
            } else {
                buttons::secondary(theme, status)
            }
        })
        .on_press(message)
        .into()
}

fn favorite_filter_icon<'a, M: Clone + 'a>(selected: bool, theme: &Theme) -> Element<'a, M> {
    let path = if selected {
        "star-solid"
    } else {
        "star-outline"
    };
    let color = if selected {
        iced::Color::WHITE
    } else {
        Palette::text_muted(theme)
    };

    svg(assets::icon(path))
        .width(Length::Fixed(12.0))
        .height(Length::Fixed(12.0))
        .style(move |_, _| iced::widget::svg::Style { color: Some(color) })
        .into()
}

fn kind_badge<'a, M: Clone + 'a>(theme: &Theme, kind: EntryKind) -> Element<'a, M> {
    let (label, color) = match kind {
        EntryKind::Study => ("S", Palette::accent(theme)),
        EntryKind::Review => ("R", Palette::success(theme)),
    };

    container(text(label).size(10).color(color))
        .padding([1, 5])
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(iced::Color { a: 0.18, ..color })),
            border: iced::Border {
                color,
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn favorite_icon<'a, M: Clone + 'a>(favorite: bool, theme: &Theme) -> Element<'a, M> {
    let path = if favorite {
        "star-solid"
    } else {
        "star-outline"
    };
    let color = if favorite {
        Palette::accent(theme)
    } else {
        Palette::text_muted(theme)
    };

    svg(assets::icon(path))
        .width(Length::Fixed(12.0))
        .height(Length::Fixed(12.0))
        .style(move |_, _| iced::widget::svg::Style { color: Some(color) })
        .into()
}

fn folder_button_style(theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => Palette::border(theme),
        _ => iced::Color::TRANSPARENT,
    };
    button::Style {
        background: Some(iced::Background::Color(bg)),
        text_color: Palette::text_primary(theme),
        ..Default::default()
    }
}

fn file_button_style(theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => Palette::border(theme),
        _ => iced::Color::TRANSPARENT,
    };
    button::Style {
        background: Some(iced::Background::Color(bg)),
        text_color: Palette::text_secondary(theme),
        ..Default::default()
    }
}
