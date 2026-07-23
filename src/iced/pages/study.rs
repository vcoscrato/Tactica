use iced::Theme;
use iced::widget::{
    Space, button, column, row, scrollable, stack, svg, text, text_editor, text_input,
};
use iced::{Color, Element, Event, Length, Point, Subscription, Task, keyboard, mouse};
use shakmaty::{Move, Position, Role, Square};

use crate::core::config::AppSettings;
use crate::core::modes::study::Study;

use crate::iced::pages::{AnalysisMessage, GameMode, analysis_subscription};
use crate::iced::panels::GameLayout;
use crate::iced::style::{Palette, buttons};
use crate::iced::widgets::board::{Board, BoardEvent, BoardMessage, PositionNode};
use crate::iced::widgets::common::{
    confirm_cancel_row, game_result_banner, modal, promotion_modal,
};
use crate::iced::widgets::engine_ui::{self, EngineState};
use crate::iced::widgets::move_ribbon;
use crate::iced::widgets::sidebar;

use crate::core::openings::OpeningNames;
use std::sync::Arc;

const SAVE_ICON: &[u8] = include_bytes!("../../../assets/icons/save.svg");

pub struct StudyMode {
    pub study: Study,
    pub(crate) board: Board, // UI Board
    es: EngineState,
    note_content: text_editor::Content,
    editing_name: bool,
    name_input: String,

    pub settings: AppSettings,
    editing_note: Option<Vec<usize>>,
    editing_note_title: String,
    pending_promotion: Option<(Square, Square, Vec<Move>)>, // (from, to, promotion_moves)

    // Branching state
    show_branch_dialog: bool,
    branch_name_input: String,
    branch_menu_open_at: Option<(usize, Point)>, // Depth + Position
    pending_branch_name: Option<String>,
    cursor_position: Point,
    pending_delete_branch: Option<(usize, usize)>,

    current_title_input: String,

    openings: Arc<OpeningNames>,
}

#[derive(Debug, Clone)]
pub enum StudyMessage {
    Board(BoardMessage),
    ToggleAnalysis,
    OpenEngineSettings,
    PollEngine,

    PlayLine(usize),
    GoToPath(Vec<usize>),
    NoteAction(text_editor::Action),
    CurrentTitleChanged(String),
    SubmitCurrentTitle,
    Save,
    ReviewThisLine,
    StartEditName,
    NameInputChanged(String),
    FinishEditName,
    StartEditNote(Vec<usize>, String),
    EditNoteTitleChanged(String),
    FinishEditNote,
    DeleteNote(Vec<usize>),
    StepBackward,
    StepForward,
    Tick,
    PromoteTo(Role),
    CancelPromotion,
    KeyPressed(keyboard::Key, keyboard::Modifiers),

    // Branching
    CancelBranching,
    UpdateBranchNameInput(String),
    ConfirmBranching,
    ToggleBranchMenu(usize),
    SelectBranch(usize, usize), // (path_idx, child_idx)
    RequestDeleteBranch(usize, usize),
    ConfirmDeleteBranch,
    CancelDeleteBranch,
    UpdateCursor(Point),

    // Engine Settings
    SetMultiPV(u32),
    SetMaxDepth(Option<u32>),
    SetThreads(u32),
    SetHashMB(u32),

    None,
}

impl StudyMode {
    pub fn new(study: Study, settings: AppSettings, openings: Arc<OpeningNames>) -> Self {
        let note_content = text_editor::Content::with_text(study.current_note());
        let analyzing = settings.engine.enabled;
        let es = EngineState::new(analyzing);
        let mut mode = Self {
            study,
            board: {
                let mut b = Board::new();
                b.set_animation_speed(settings.animation_speed);
                b.set_theme(settings.board_theme);
                b
            },
            note_content,
            es,
            editing_name: false,
            name_input: String::new(),
            settings,
            editing_note: None,
            editing_note_title: String::new(),
            pending_promotion: None,
            show_branch_dialog: false,
            branch_name_input: String::new(),
            branch_menu_open_at: None,
            pending_branch_name: None,
            cursor_position: Point::default(),
            pending_delete_branch: None,
            current_title_input: String::new(),
            openings,
        };

        mode.sync_note();
        mode
    }

    pub fn shutdown(&mut self) {
        self.es.shutdown();
    }
}

impl GameMode for StudyMode {
    type Message = StudyMessage;

    fn set_settings(&mut self, settings: AppSettings) {
        self.board.set_animation_speed(settings.animation_speed);
        self.board.set_theme(settings.board_theme);
        self.es
            .apply_settings(&settings, Some(self.study.position()));
        self.settings = settings;
    }

    fn update(&mut self, message: StudyMessage) -> Task<StudyMessage> {
        match message {
            StudyMessage::Board(msg) => {
                if let Some(event) = self.board.update(self.study.position(), msg) {
                    match event {
                        BoardEvent::MoveMade(mv, was_dragged) => {
                            self.make_move_impl(mv, !was_dragged);
                        }
                        BoardEvent::MoveAttempted(_, _) => {
                            // Shake? Or just ignore.
                        }
                        BoardEvent::SelectionChanged(_) => {
                            // Handled by board UI
                        }
                        BoardEvent::PromotionRequired(from, to, moves) => {
                            self.pending_promotion = Some((from, to, moves));
                        }
                        BoardEvent::NavigationChanged => {}
                    }
                }
            }
            StudyMessage::ToggleAnalysis => {
                self.es.toggle(&mut self.settings, self.study.position());
            }
            StudyMessage::OpenEngineSettings => {}
            StudyMessage::PollEngine => {
                self.es.poll();
            }

            StudyMessage::PlayLine(line_idx) => {
                if let Some(uci) = self
                    .es
                    .analysis
                    .lines
                    .get(line_idx)
                    .and_then(|l| l.pv.first().cloned())
                {
                    self.play_uci_move(&uci);
                }
            }
            StudyMessage::GoToPath(path) => {
                self.save_note();
                self.study.go_to_path(&path);
                self.sync_note();
                self.board.deselect(); // Clear selection on UI
                if self.es.analyzing {
                    self.es
                        .start_with_settings(&self.settings.engine, self.study.position());
                }
            }
            StudyMessage::NoteAction(action) => {
                self.note_content.perform(action);
            }
            StudyMessage::CurrentTitleChanged(title) => {
                self.current_title_input = title;
            }
            StudyMessage::SubmitCurrentTitle => {
                self.study
                    .set_current_note_title(self.current_title_input.clone());
            }
            StudyMessage::Save => {
                // Handled by parent (app.rs) - maps to GlobalSave
            }
            StudyMessage::ReviewThisLine => {}
            StudyMessage::StartEditName => {
                self.editing_name = true;
                self.name_input = self.study.name.clone();
            }
            StudyMessage::NameInputChanged(name) => {
                self.name_input = name;
            }
            StudyMessage::FinishEditName => {
                self.editing_name = false;
                if !self.name_input.is_empty() && self.name_input != self.study.name {
                    self.study.rename(self.name_input.clone());
                }
            }
            StudyMessage::StartEditNote(path, current_title) => {
                self.editing_note = Some(path);
                self.editing_note_title = current_title;
            }
            StudyMessage::EditNoteTitleChanged(title) => {
                self.editing_note_title = title;
            }
            StudyMessage::FinishEditNote => {
                if let Some(path) = self.editing_note.take() {
                    self.study
                        .set_note_title_at_path(&path, self.editing_note_title.clone());
                }
            }
            StudyMessage::DeleteNote(path) => {
                self.study.clear_note_at_path(&path);
                self.sync_note();
            }
            StudyMessage::StepBackward => self.go_back(),
            StudyMessage::StepForward => self.go_forward(),
            StudyMessage::Tick => {
                self.es
                    .ensure_running(&self.settings.engine, self.study.position());
                self.board.tick();
                self.es.tick_eval_bar(self.settings.show_eval_bar);
            }
            StudyMessage::PromoteTo(role) => {
                if let Some((_from, _to, moves)) = self.pending_promotion.take()
                    && let Some(mv) = moves.iter().find(|m| m.promotion() == Some(role)).cloned()
                {
                    self.make_move(mv);
                }
            }
            StudyMessage::CancelPromotion => {
                self.pending_promotion = None;
            }
            StudyMessage::KeyPressed(key, modifiers) => {
                if modifiers.control() {
                    return Task::none();
                }
                match key.as_ref() {
                    keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => self.go_back(),
                    keyboard::Key::Named(keyboard::key::Named::ArrowRight) => self.go_forward(),
                    _ => {}
                }
            }

            StudyMessage::CancelBranching => {
                self.show_branch_dialog = false;
                self.branch_name_input.clear();
            }
            StudyMessage::UpdateBranchNameInput(s) => {
                self.branch_name_input = s;
            }
            StudyMessage::ConfirmBranching => {
                let input = self.branch_name_input.trim();
                let final_name = if input.is_empty() {
                    self.study
                        .current_node()
                        .san()
                        .map(|s| s.into())
                        .unwrap_or("Var".into())
                } else {
                    input.to_string()
                };

                self.study.set_current_note_title(final_name);
                self.show_branch_dialog = false;
                self.branch_name_input.clear();
            }
            StudyMessage::ToggleBranchMenu(idx) => {
                if let Some((curr_idx, _)) = self.branch_menu_open_at {
                    if curr_idx == idx {
                        self.branch_menu_open_at = None;
                    } else {
                        self.branch_menu_open_at = Some((idx, self.cursor_position));
                    }
                } else {
                    self.branch_menu_open_at = Some((idx, self.cursor_position));
                }
            }
            StudyMessage::UpdateCursor(p) => {
                self.cursor_position = p;
            }
            StudyMessage::SelectBranch(path_len, child_idx) => {
                let mut path: Vec<usize> = self
                    .study
                    .tree
                    .current_path()
                    .iter()
                    .take(path_len)
                    .cloned()
                    .collect();
                path.push(child_idx);
                self.study.go_to_path(&path);
                self.branch_menu_open_at = None;
                self.sync_note();
            }
            StudyMessage::RequestDeleteBranch(path_len, child_idx) => {
                self.pending_delete_branch = Some((path_len, child_idx));
                self.branch_menu_open_at = None;
            }
            StudyMessage::CancelDeleteBranch => {
                self.pending_delete_branch = None;
            }
            StudyMessage::ConfirmDeleteBranch => {
                if let Some((path_len, child_idx)) = self.pending_delete_branch.take() {
                    let parent_path = self
                        .study
                        .tree
                        .current_path()
                        .iter()
                        .take(path_len)
                        .cloned()
                        .collect::<Vec<_>>();

                    // If we are deleting a branch we are currently IN (or deeper)
                    let current_child_idx = if self.study.tree.current_path().len() > path_len {
                        Some(self.study.tree.current_path()[path_len])
                    } else {
                        None
                    };

                    self.study.delete_branch(&parent_path, child_idx);

                    if let Some(curr) = current_child_idx {
                        if curr == child_idx {
                            // We were in the deleted branch -> Go to parent
                            self.study.go_to_path(&parent_path);
                            self.board.deselect();
                        } else if curr > child_idx {
                            // We were in a later sibling -> Shift index
                            let mut new_path = self.study.tree.current_path().to_vec();
                            if path_len < new_path.len() {
                                new_path[path_len] -= 1;
                                self.study.go_to_path(&new_path);
                            }
                        }
                    }

                    self.sync_note();
                }
            }
            StudyMessage::SetMultiPV(pv) => {
                if pv != self.settings.engine.multi_pv {
                    self.settings.engine.multi_pv = pv;
                    if let Err(e) = self.settings.save() {
                        self.es.error = Some(e);
                        return Task::none();
                    }
                    self.es
                        .init(&self.settings.engine, Some(self.study.position()));
                }
            }
            StudyMessage::SetMaxDepth(depth) => {
                if depth != self.settings.engine.max_depth {
                    self.settings.engine.max_depth = depth;
                    if let Err(e) = self.settings.save() {
                        self.es.error = Some(e);
                        return Task::none();
                    }
                    self.es
                        .init(&self.settings.engine, Some(self.study.position()));
                }
            }
            StudyMessage::SetThreads(threads) => {
                if threads != self.settings.engine.threads {
                    self.settings.engine.threads = threads;
                    if let Err(e) = self.settings.save() {
                        self.es.error = Some(e);
                        return Task::none();
                    }
                    self.es
                        .init(&self.settings.engine, Some(self.study.position()));
                }
            }
            StudyMessage::SetHashMB(hash) => {
                if hash != self.settings.engine.hash_mb {
                    self.settings.engine.hash_mb = hash;
                    if let Err(e) = self.settings.save() {
                        self.es.error = Some(e);
                        return Task::none();
                    }
                    self.es
                        .init(&self.settings.engine, Some(self.study.position()));
                }
            }

            StudyMessage::None => {}
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<StudyMessage> {
        let mut subs = vec![
            iced::event::listen_with(|event, _status, _window| match event {
                iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                    Some(StudyMessage::KeyPressed(key, modifiers))
                }
                _ => None,
            }),
            iced::event::listen().map(|e| {
                if let Event::Mouse(mouse::Event::CursorMoved { position }) = e {
                    StudyMessage::UpdateCursor(position)
                } else {
                    StudyMessage::None
                }
            }),
        ];
        subs.push(
            analysis_subscription(
                self.es.analyzing,
                self.es.engine.is_some(),
                self.settings.show_eval_bar,
                self.board.is_animating(),
            )
            .map(|message| match message {
                AnalysisMessage::PollEngine => StudyMessage::PollEngine,
                AnalysisMessage::Tick => StudyMessage::Tick,
            }),
        );
        Subscription::batch(subs)
    }

    fn view(&self, theme: &Theme) -> Element<'_, StudyMessage> {
        let s = self.settings.ui_scale;
        let last_move = self.study.last_move().cloned();

        let board = self
            .board
            .view(
                self.study.position(),
                last_move.as_ref(),
                None,
                Length::Fixed(self.settings.board_size),
            )
            .map(StudyMessage::Board);

        // Analysis name (editable)
        let name_display: Element<'_, StudyMessage> = if self.editing_name {
            text_input("Name...", &self.name_input)
                .size(14.0 * s)
                .padding(4)
                .on_input(StudyMessage::NameInputChanged)
                .on_submit(StudyMessage::FinishEditName)
                .width(Length::Fill)
                .into()
        } else {
            button(text(&self.study.name).size(14.0 * s))
                .padding([3, 5])
                .style(|theme, _| button::Style {
                    background: None,
                    text_color: Palette::text_primary(theme),
                    ..Default::default()
                })
                .on_press(StudyMessage::StartEditName)
                .into()
        };

        // Save button
        let save_icon = svg::Handle::from_memory(SAVE_ICON);
        let save_button = button(
            row![
                svg(save_icon)
                    .width(14)
                    .height(14)
                    .style(|theme, _| svg::Style {
                        color: Some(Palette::text_primary(theme))
                    }),
                text("Save").size(11.0 * s)
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
        )
        .padding([4, 8])
        .style(buttons::secondary)
        .on_press(StudyMessage::Save);

        let review_line_button = button(text("Review Line").size(11.0 * s))
            .padding([4, 8])
            .style(buttons::secondary)
            .on_press(StudyMessage::ReviewThisLine);

        let header_actions =
            sidebar::action_row(vec![save_button.into(), review_line_button.into()], s);

        let header_row = row![
            name_display,
            Space::new().width(Length::Fill),
            header_actions
        ]
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);

        let board_area = engine_ui::build_board_eval_area(
            board,
            &self.es.analysis,
            self.es.current_eval_pct,
            s,
            self.settings.board_size,
            self.settings.show_eval_bar && self.es.analyzing,
        );

        let lines = engine_ui::build_engine_lines(
            theme,
            s,
            self.study.position(),
            &self.es.analysis,
            StudyMessage::PlayLine,
        );

        let engine_controls = engine_ui::engine_controls_row(
            theme,
            engine_ui::EngineControlsState {
                ui_scale: s,
                analyzing: self.es.analyzing,
                current_depth: self.es.analysis.depth,
                max_depth: self.settings.engine.max_depth,
                error: self.es.error.as_deref(),
            },
            StudyMessage::ToggleAnalysis,
            StudyMessage::OpenEngineSettings,
        );

        let engine_content: Element<'_, StudyMessage> = if self.es.analyzing {
            column![engine_controls, lines].spacing(8).into()
        } else {
            engine_controls
        };

        let nav = self.build_navigation(theme);

        let note_editor = text_editor(&self.note_content)
            .height(Length::Fixed(100.0))
            .padding(6)
            .on_action(StudyMessage::NoteAction);

        let notes_list = self.build_notes_list(theme);

        // Info Panel (Left/Middle): Moves + Navigation
        let mut info_panel = column![header_row,].spacing(12.0 * s);

        if let Some(result) = game_result_banner(theme, self.study.position(), s) {
            info_panel = info_panel.push(result);
        }
        info_panel = info_panel
            .push(iced::widget::rule::horizontal(1))
            .push(engine_content)
            .push(iced::widget::rule::horizontal(1))
            .push(sidebar::section(theme, "Moves".into(), nav, s));

        // Control Panel (Right): Notes + Annotation
        let control_panel = column![
            sidebar::panel_header(theme, "Notes", None, s, None),
            sidebar::section(
                theme,
                "Current Position".into(),
                column![
                    text_input("Branch Title...", &self.current_title_input)
                        .size(13.0 * s)
                        .padding(6)
                        .on_input(StudyMessage::CurrentTitleChanged)
                        .on_submit(StudyMessage::SubmitCurrentTitle),
                    Space::new().height(4),
                    note_editor,
                ],
                s,
            ),
            iced::widget::rule::horizontal(1),
            sidebar::section(theme, "Saved Notes".into(), notes_list, s),
        ]
        .spacing(12.0 * s);

        let content = GameLayout::new(board_area, control_panel.into(), &self.settings)
            .with_info_panel(info_panel.into())
            .view();

        // Modals
        if self.show_branch_dialog {
            let modal_content = self.build_branch_modal(theme);
            stack![content, modal_content]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else if let Some((path_idx, child_idx)) = self.pending_delete_branch {
            let modal_content = self.build_delete_branch_confirm(theme, path_idx, child_idx);
            stack![content, modal_content]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else if self.pending_promotion.is_some() {
            let is_white = self.study.position().turn().is_white();
            let promo = promotion_modal(
                theme,
                is_white,
                StudyMessage::PromoteTo,
                StudyMessage::CancelPromotion,
            );
            stack![content, promo]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            content
        }
    }
    fn navigate_home(&mut self) {
        self.save_note();
        self.study.go_to_start();
        self.sync_note();
        self.board.deselect();
        self.board.clear_animation();
        if self.es.analyzing {
            self.es
                .start_with_settings(&self.settings.engine, self.study.position());
        }
    }

    fn navigate_end(&mut self) {
        self.save_note();
        self.study.go_to_end();
        self.sync_note();
        self.board.deselect();
        self.board.clear_animation();
        if self.es.analyzing {
            self.es
                .start_with_settings(&self.settings.engine, self.study.position());
        }
    }

    fn instructions(&self) -> String {
        "Comprehensive study environment.\n\n\
         • Navigation: Use arrow keys or click moves to navigate.\n\
         • Variations: Play new moves to create branches. Right-click moves in the list to delete or promote variations.\n\
         • Notes: Add text notes and move titles in the sidebar.\n\
         • Engine: Toggle the engine (top-right) for real-time evaluation.\n\
         • Tree: Moves are saved automatically to your study file."
        .to_string()
    }

    fn active_hotkeys(&self) -> Vec<(String, String)> {
        vec![("Left/Right".to_string(), "Navigate moves".to_string())]
    }
}

impl StudyMode {
    fn go_back(&mut self) {
        self.save_note();
        if let Some(mv) = self.study.last_move() {
            let mv_clone = *mv;
            if self.study.go_back() {
                self.board
                    .animate_move(&mv_clone, self.study.position(), true);
                self.sync_note();
                self.board.deselect();
                if self.es.analyzing {
                    self.es
                        .start_with_settings(&self.settings.engine, self.study.position());
                }
            }
        }
    }

    fn go_forward(&mut self) {
        self.save_note();
        if self.study.go_forward() {
            if let Some(mv) = self.study.last_move() {
                let m: Move = *mv;
                self.board.animate_move(&m, self.study.position(), false);
            }
            self.sync_note();
            self.board.deselect();
            if self.es.analyzing {
                self.es
                    .start_with_settings(&self.settings.engine, self.study.position());
            }
        }
    }

    fn save_note(&mut self) {
        self.study.set_current_note(self.note_content.text());
    }

    fn sync_note(&mut self) {
        self.note_content = text_editor::Content::with_text(self.study.current_note());
        self.current_title_input = self.study.current_node().annotation.title.clone();
    }

    fn play_uci_move(&mut self, uci_str: &str) {
        if let Some(mv) = engine_ui::parse_uci_move(self.study.position(), uci_str) {
            self.make_move(mv);
        }
    }

    fn make_move(&mut self, mv: Move) {
        self.make_move_impl(mv, true);
    }

    fn make_move_impl(&mut self, mv: Move, animate: bool) {
        let current_node = self.study.current_node();

        let matching_child_idx = if let Ok(next_pos) = current_node.position().clone().play(mv) {
            current_node
                .children()
                .iter()
                .position(|c| c.position() == &next_pos)
        } else {
            None
        };

        let is_new_move = matching_child_idx.is_none();

        if is_new_move {
            let is_branching = !current_node.children().is_empty();
            let temp_pos = current_node
                .position()
                .clone()
                .play(mv)
                .unwrap_or(current_node.position().clone());
            let eco_name = self
                .openings
                .lookup(&temp_pos)
                .or_else(|| self.openings.lookup_book(&temp_pos))
                .cloned();

            self.save_note();
            self.study.make_move(mv).ok();

            if let Some(name) = eco_name {
                self.study.set_current_note_title(name);
            } else if is_branching {
                self.show_branch_dialog = true;
                self.branch_name_input.clear();
                self.pending_branch_name = None;
            }
        } else {
            self.save_note();
            self.study.make_move(mv).ok();
        }

        if animate {
            self.board.animate_move(&mv, self.study.position(), false);
        }

        self.sync_note();
        self.board.deselect();
        if self.es.analyzing {
            self.es
                .start_with_settings(&self.settings.engine, self.study.position());
        }
    }

    // handle_square_click and select_square removed as they are handled by Board

    // ... helpers ...
    fn build_navigation(&self, theme: &Theme) -> Element<'_, StudyMessage> {
        let s = self.settings.ui_scale;
        let t = |v: f32| v * s;
        let mut ribbon_moves = Vec::new();
        let mut node = self.study.tree.root();
        let mut is_white = true;

        for (i, &child_idx) in self.study.tree.current_path().iter().enumerate() {
            if child_idx >= node.children().len() {
                break;
            }

            let has_branch = node.children().len() > 1;
            let child = &node.children()[child_idx];

            if let Some(san) = child.san() {
                let badge = None;

                ribbon_moves.push(move_ribbon::RibbonMove {
                    san: san.to_string(),
                    move_index: i + 1,
                    is_white,
                    has_note: !child.annotation.note.trim().is_empty(),
                    has_branch,
                    badge,
                });
                is_white = !is_white;
            }
            node = child;
        }

        let mut proj_node = node;
        let mut proj_is_white = is_white;
        let mut proj_depth = ribbon_moves.len();

        while !proj_node.children().is_empty() && proj_depth < 200 {
            let has_branch = proj_node.children().len() > 1;
            let child = &proj_node.children()[0];

            if let Some(san) = child.san() {
                let badge = None;

                ribbon_moves.push(move_ribbon::RibbonMove {
                    san: san.to_string(),
                    move_index: proj_depth + 1,
                    is_white: proj_is_white,
                    has_note: !child.annotation.note.trim().is_empty(),
                    has_branch,
                    badge,
                });

                proj_depth += 1;
                proj_is_white = !proj_is_white;
            }
            proj_node = child;
        }

        let current_path_len = self.study.tree.current_path().len();
        let current_path_clone = self.study.tree.current_path().to_vec();

        let root_pos = self.study.tree.root().position();
        let start_move_num = root_pos.fullmoves().get() as usize;
        let start_with_black = root_pos.turn().is_black();

        let ribbon = move_ribbon::build_ribbon(
            theme,
            ribbon_moves,
            current_path_len,
            start_move_num,
            start_with_black,
            move |depth| {
                if depth == 0 {
                    return StudyMessage::GoToPath(Vec::new());
                }

                if depth <= current_path_len {
                    let p: Vec<usize> = current_path_clone.iter().take(depth).cloned().collect();
                    StudyMessage::GoToPath(p)
                } else {
                    let mut p = current_path_clone.clone();
                    let steps_needed = depth - current_path_len;
                    p.extend(std::iter::repeat_n(0, steps_needed));
                    StudyMessage::GoToPath(p)
                }
            },
            move |depth| {
                if depth == 0 {
                    return StudyMessage::GoToPath(Vec::new());
                }
                StudyMessage::ToggleBranchMenu(depth - 1)
            },
        );

        let continuations_area = if node.children().is_empty() {
            Element::from(iced::widget::Space::new().height(0))
        } else {
            let continuations: Vec<Element<'_, StudyMessage>> = node
                .children()
                .iter()
                .enumerate()
                .map(|(i, child)| {
                    let san = child.san().map(|s| s.into()).unwrap_or("?".to_string());
                    button(text(san).size(t(12.0)))
                        .padding([2, 8])
                        .on_press(StudyMessage::SelectBranch(
                            self.study.tree.current_path().len(),
                            i,
                        ))
                        .style(buttons::secondary)
                        .into()
                })
                .collect();

            column![iced::widget::row(continuations).spacing(6),]
                .spacing(12)
                .align_x(iced::Alignment::Center)
                .into()
        };

        column![ribbon, continuations_area].spacing(10).into()
    }

    fn build_notes_list(&self, theme: &Theme) -> Element<'_, StudyMessage> {
        let s = self.settings.ui_scale;
        let mut notes: Vec<(Vec<usize>, String, String, String)> = Vec::new();
        collect_notes_impl(self.study.tree.root(), Vec::new(), &mut notes, 1, true);

        if notes.is_empty() {
            return text("No notes yet")
                .size(9.0 * s)
                .color(Palette::text_muted(theme))
                .into();
        }

        let current_path = self.study.tree.current_path().to_vec();
        let editing_note = self.editing_note.clone();

        let items: Vec<Element<'_, StudyMessage>> = notes
            .into_iter()
            .map(|(path, move_text, title, note)| {
                let is_current = path == current_path;
                let is_editing = editing_note.as_ref() == Some(&path);
                let path_clone = path.clone();
                let path_clone2 = path.clone();
                let path_clone3 = path.clone();

                let display_title = if title.is_empty() {
                    move_text.clone()
                } else {
                    title.clone()
                };
                let preview = if note.len() > 25 {
                    format!("{}...", &note[..25])
                } else {
                    note
                };

                let color = if is_current {
                    Palette::success(theme)
                } else {
                    Palette::warning(theme)
                };

                if is_editing {
                    row![
                        text_input("Title...", &self.editing_note_title)
                            .size(10.0 * s)
                            .padding(3)
                            .on_input(StudyMessage::EditNoteTitleChanged)
                            .on_submit(StudyMessage::FinishEditNote)
                            .width(Length::Fill),
                        button(
                            text("✓")
                                .size(10.0 * s)
                                .color(Color::from_rgb(0.4, 0.9, 0.4))
                        )
                        .padding([2, 6])
                        .on_press(StudyMessage::FinishEditNote),
                    ]
                    .spacing(2)
                    .into()
                } else {
                    row![
                        button(
                            column![
                                text(display_title).size(10.0 * s).color(color),
                                text(preview)
                                    .size(8.0 * s)
                                    .color(Palette::text_muted(theme)),
                            ]
                            .spacing(1)
                        )
                        .padding([3, 6])
                        .width(Length::Fill)
                        .style(move |theme, _| button::Style {
                            background: Some(iced::Background::Color(Palette::panel(theme))),
                            text_color: color,
                            border: iced::Border {
                                radius: 3.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .on_press(StudyMessage::GoToPath(path_clone)),
                        button(text("Edit").size(9.0 * s))
                            .padding([2, 6])
                            .style(buttons::secondary)
                            .on_press(StudyMessage::StartEditNote(path_clone2, title)),
                        button(text("Del").size(9.0 * s))
                            .padding([2, 6])
                            .style(buttons::danger)
                            .on_press(StudyMessage::DeleteNote(path_clone3)),
                    ]
                    .spacing(3)
                    .into()
                }
            })
            .collect();

        scrollable(column(items).spacing(3))
            .height(Length::Fixed(120.0))
            .width(Length::Fill)
            .into()
    }

    fn build_branch_modal(&self, theme: &Theme) -> Element<'_, StudyMessage> {
        let s = self.settings.ui_scale;
        modal(
            column![
                text("Create New Branch")
                    .size(16)
                    .color(Palette::text_primary(theme)),
                text_input("Variation Name", &self.branch_name_input)
                    .on_input(StudyMessage::UpdateBranchNameInput)
                    .on_submit(StudyMessage::ConfirmBranching)
                    .padding(5),
                row![
                    button(text("Cancel").size(12.0 * s))
                        .padding(6)
                        .style(buttons::secondary)
                        .on_press(StudyMessage::CancelBranching),
                    button(text("Create").size(12.0 * s))
                        .padding(6)
                        .style(buttons::primary)
                        .on_press(StudyMessage::ConfirmBranching),
                ]
                .spacing(10)
            ]
            .spacing(15)
            .align_x(iced::Alignment::Center)
            .width(400),
        )
    }

    fn build_delete_branch_confirm(
        &self,
        theme: &Theme,
        _path_idx: usize,
        _child_idx: usize,
    ) -> Element<'_, StudyMessage> {
        let s = self.settings.ui_scale;
        modal(
            column![
                text("Delete Branch?")
                    .size(16)
                    .color(Palette::text_primary(theme)),
                text("This action cannot be undone.")
                    .size(12.0 * s)
                    .color(Palette::text_secondary(theme)),
                confirm_cancel_row(
                    StudyMessage::CancelDeleteBranch,
                    StudyMessage::ConfirmDeleteBranch,
                    "Delete",
                    s,
                )
            ]
            .spacing(12)
            .align_x(iced::Alignment::Center),
        )
    }
}

// Standalone helpers

fn collect_notes_impl(
    node: &PositionNode,
    path: Vec<usize>,
    notes: &mut Vec<(Vec<usize>, String, String, String)>,
    move_num: usize,
    is_white: bool,
) {
    if !node.annotation.note.trim().is_empty() {
        let move_text = if let Some(san) = node.san() {
            if is_white {
                format!("{}. {}", move_num, san)
            } else {
                format!("{}...{}", move_num, san)
            }
        } else {
            "Start".to_string()
        };
        notes.push((
            path.clone(),
            move_text,
            node.annotation.title.clone(),
            node.annotation.note.clone(),
        ));
    }

    for (i, child) in node.children().iter().enumerate() {
        let mut child_path = path.clone();
        child_path.push(i);

        let (next_num, next_white) = if node.san().is_some() {
            if is_white {
                (move_num, false)
            } else {
                (move_num + 1, true)
            }
        } else {
            (move_num, is_white)
        };

        collect_notes_impl(child, child_path, notes, next_num, next_white);
    }
}
