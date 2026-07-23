//! Tactica - A Rust + Iced chess learning application

use crate::core::config;
use crate::core::config::AppSettings;
use crate::core::game_review::GameReview;
use crate::core::library::EntryKind;
use crate::core::library::Library;
use crate::core::modes::study::{Study, StudyInit};
use crate::core::openings::OpeningNames;
use crate::core::pgn;

use crate::iced::controls::GlobalHotkey;
use crate::iced::pages::chessle::{self, ChessleMode};
use crate::iced::pages::game_review::{self, GameReviewMode};
use crate::iced::pages::quick_board::{self, QuickBoardMode};
use crate::iced::pages::study::{self, StudyMode};
use crate::iced::pages::trivia::{self, TriviaMode};
use crate::iced::pages::{GameMode, Mode, ModeMessage};
use crate::iced::widgets::common::{confirm_cancel_row, modal};
use crate::iced::widgets::engine_ui;
use crate::iced::widgets::library_sidebar::{self, LibrarySidebarMessage};
use crate::iced::widgets::right_panel::{
    self, RightPanelContext, RightPanelMessage, RightPanelTab,
};
use crate::iced::widgets::toast::{ToastManager, ToastType};
use crate::iced::widgets::top_bar::{self, ModeType, TopBarMessage};
use crate::iced::{layout, style};
use crate::metadata;
use crate::storage::Storage;

use iced::widget::{button, column, container, row, stack, text, text_editor};
use iced::{Alignment, Element, Length, Subscription, Task, Theme};
use shakmaty::Move;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub fn run() -> iced::Result {
    iced::application(ChessApp::new, ChessApp::update, ChessApp::view)
        .title(metadata::APP_NAME)
        .window(window_settings())
        .theme(|state: &ChessApp| state.theme.clone())
        .subscription(ChessApp::subscription)
        .run()
}

fn window_settings() -> iced::window::Settings {
    let settings = iced::window::Settings {
        icon: Some(
            iced::window::icon::from_file_data(include_bytes!("../../assets/tactica.png"), None)
                .expect("embedded Tactica icon must be a valid PNG"),
        ),
        ..Default::default()
    };

    #[cfg(target_os = "linux")]
    let settings = {
        let mut settings = settings;
        settings.platform_specific.application_id = metadata::APP_ID.to_string();
        settings
    };

    settings
}

/// Global application error for display in modal
#[derive(Debug, Clone)]
pub struct AppError {
    pub title: String,
    pub message: String,
}

impl AppError {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
        }
    }
}

struct ChessApp {
    mode: Mode,
    settings: AppSettings,
    theme: Theme,
    openings: Arc<OpeningNames>,
    error: Option<AppError>,
    toasts: ToastManager,

    // UI state
    library: Library,
    library_search: String,
    right_panel: RightPanelTab,
    show_mode_switch_confirm: bool,
    pending_mode_switch: Option<ModeType>,
    last_window_size: Option<(f32, f32)>,

    // Delete confirmation modal
    confirm_delete_path: Option<PathBuf>,

    // Import modal
    show_import_modal: bool,
    import_input: text_editor::Content,
    import_error: Option<String>,
    import_target: ImportTarget,
    library_filter: library_sidebar::LibraryFilter,
}

#[derive(Debug, Clone)]
pub enum Message {
    Mode(ModeMessage),
    StudyLoaded(Option<Box<Study>>),
    ReviewLoaded(Option<Box<GameReview>>),

    TopBar(TopBarMessage),

    RightPanel(RightPanelMessage),

    Library(LibrarySidebarMessage),

    ToggleLayoutMode,
    ToggleAutoLayout,
    ToggleAnimationSpeed,
    ToggleTheme,

    WindowResized(f32, f32),
    GlobalHome,
    GlobalEnd,
    GlobalFlipBoard,
    GlobalNewGame,
    GlobalSave,

    SwitchMode(ModeType),
    ConfirmModeSwitch,
    CancelModeSwitch,

    GoToStudy(StudyInit),

    ConfirmDelete(PathBuf),
    CancelDelete,
    ExecuteDelete,

    ShowImportModal,
    HideImportModal,
    SetImportTarget(ImportTarget),
    ImportInputChanged(text_editor::Action),
    ConfirmImport,

    LibraryRootSelected(Option<PathBuf>),

    ShowError(AppError),
    DismissError,
    ShowToast(ToastType, String),
    Tick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImportTarget {
    #[default]
    Study,
    GameReview,
}

impl Default for ChessApp {
    fn default() -> Self {
        let migration_error = Storage::migrate_legacy_app_dirs().err();
        let settings = AppSettings::load();
        let storage_error = migration_error.or_else(|| settings.storage().ensure_base_dirs().err());
        let library = Library::new();

        // Start with Quick Board mode by default
        let mode = Mode::QuickBoard(Box::new(QuickBoardMode::new(settings.clone())));

        Self {
            mode,
            settings,
            theme: Theme::Dark,
            openings: Arc::new(OpeningNames::new()),
            error: storage_error.map(|e| AppError::new("Library setup failed", e)),
            toasts: ToastManager::new(),
            library,
            library_search: String::new(),
            right_panel: RightPanelTab::None,
            show_mode_switch_confirm: false,
            pending_mode_switch: None,
            last_window_size: None,
            confirm_delete_path: None,
            show_import_modal: false,
            import_input: text_editor::Content::new(),
            import_error: None,
            import_target: ImportTarget::Study,
            library_filter: library_sidebar::LibraryFilter::All,
        }
    }
}

impl ChessApp {
    fn new() -> (Self, Task<Message>) {
        (Self::default(), Task::done(Message::Tick))
    }

    fn save_settings_and_apply(&mut self) -> Result<(), String> {
        self.settings.storage().ensure_base_dirs()?;
        self.settings.save()?;
        self.mode.update_settings(self.settings.clone());
        Ok(())
    }

    fn update_engine_setting_analysis<F>(&mut self, update: F) -> Task<Message>
    where
        F: FnOnce(&mut AppSettings) -> bool,
    {
        if update(&mut self.settings) {
            if let Err(e) = self.save_settings_and_apply() {
                return Task::done(Message::ShowError(AppError::new("Settings save failed", e)));
            }
            return Task::done(Message::ShowToast(
                ToastType::Info,
                "Engine settings updated".to_string(),
            ));
        }
        Task::none()
    }

    fn shutdown_current_mode(&mut self) -> Result<(), String> {
        match &mut self.mode {
            Mode::Study(mode) => {
                mode.shutdown();
                mode.study.save()?;
            }
            Mode::QuickBoard(mode) => mode.shutdown(),
            Mode::GameReview(mode) => {
                mode.shutdown();
                mode.data.save()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn set_study_mode(&mut self, study: Study) {
        self.mode = Mode::Study(Box::new(StudyMode::new(
            study,
            self.settings.clone(),
            self.openings.clone(),
        )));
        self.apply_current_window_layout();
    }

    fn set_game_review_mode(&mut self, data: GameReview) -> Task<Message> {
        let needs_review = data.review_results.is_empty();
        self.mode = Mode::GameReview(Box::new(GameReviewMode::new(
            data,
            self.settings.clone(),
            self.openings.clone(),
        )));
        self.apply_current_window_layout();
        if needs_review {
            Task::done(Message::Mode(ModeMessage::GameReview(
                crate::iced::pages::game_review::GameReviewMessage::RunReview,
            )))
        } else {
            Task::none()
        }
    }

    fn map_mode_task<M: Send + 'static>(
        task: Task<M>,
        wrap: fn(M) -> ModeMessage,
    ) -> Task<Message> {
        task.map(move |m| Message::Mode(wrap(m)))
    }

    fn try_go_to_study_from_moves(moves: Vec<Move>) -> Option<Task<Message>> {
        if moves.is_empty() {
            None
        } else {
            Some(Task::done(Message::GoToStudy(StudyInit::FromMoves(moves))))
        }
    }

    fn apply_library_root(&mut self, root: PathBuf) -> Task<Message> {
        if let Err(e) = self.shutdown_current_mode() {
            return Task::done(Message::ShowError(AppError::new("Save failed", e)));
        }

        self.settings.library_root = Some(root.clone());
        if let Err(e) = self.save_settings_and_apply() {
            return Task::done(Message::ShowError(AppError::new("Library setup failed", e)));
        }

        self.library = Library::new();
        self.library_search.clear();
        self.library_filter = library_sidebar::LibraryFilter::All;
        self.openings = Arc::new(OpeningNames::new());
        self.mode = Mode::QuickBoard(Box::new(QuickBoardMode::new(self.settings.clone())));
        self.apply_current_window_layout();

        Task::done(Message::ShowToast(
            ToastType::Success,
            format!("Library: {}", root.display()),
        ))
    }

    fn current_mode_type(&self) -> ModeType {
        match &self.mode {
            Mode::QuickBoard(_) => ModeType::QuickBoard,
            Mode::Study(_) => ModeType::Study,
            Mode::GameReview(_) => ModeType::GameReview,
            Mode::Trivia(_) => ModeType::Trivia,
            Mode::Chessle(_) => ModeType::Chessle,
        }
    }

    fn apply_current_window_layout(&mut self) {
        if let Some((width, height)) = self.last_window_size {
            self.apply_window_layout(width, height);
        }
    }

    fn apply_window_layout(&mut self, width: f32, height: f32) {
        use config::LayoutMode;

        let layout_mode = if self.settings.auto_layout && width < height {
            LayoutMode::Vertical
        } else if self.settings.auto_layout {
            LayoutMode::Horizontal
        } else {
            self.settings.layout_mode
        };

        self.apply_layout_mode(width, height, layout_mode);
    }

    fn apply_layout_mode(&mut self, width: f32, height: f32, layout_mode: config::LayoutMode) {
        const TOP_BAR_HEIGHT: f32 = 50.0;

        let library_width = if self.settings.library_sidebar_open {
            library_sidebar::SIDEBAR_WIDTH
        } else {
            0.0
        };

        let right_panel_width = if self.right_panel.is_open() {
            right_panel::PANEL_WIDTH
        } else {
            0.0
        };

        let side_panels_width = library_width + right_panel_width;
        let content_width = (width - side_panels_width).max(0.0);
        let content_height = (height - TOP_BAR_HEIGHT).max(0.0);
        let (board_cell_width, board_cell_height) = layout::board_cell_size(
            layout_mode,
            self.settings.ui_scale,
            content_width,
            content_height,
        );

        let max_by_width = if self.uses_analysis_board_layout() {
            engine_ui::fit_board_size_for_eval_area_width(self.settings.ui_scale, board_cell_width)
        } else {
            board_cell_width
        };
        let new_board_size = max_by_width.min(board_cell_height).max(280.0);

        let layout_changed = self.settings.layout_mode != layout_mode;
        let size_changed = (self.settings.board_size - new_board_size).abs() > 10.0;

        if layout_changed || size_changed {
            self.settings.layout_mode = layout_mode;
            self.settings.board_size = new_board_size;
            self.mode.update_settings(self.settings.clone());
        }
    }

    fn uses_analysis_board_layout(&self) -> bool {
        matches!(
            self.mode,
            Mode::QuickBoard(_) | Mode::Study(_) | Mode::GameReview(_)
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::WindowResized(width, height) => {
                self.last_window_size = Some((width, height));
                self.apply_window_layout(width, height);
            }

            Message::Mode(mode_msg) => match mode_msg {
                ModeMessage::QuickBoard(msg) => {
                    if matches!(msg, quick_board::QuickBoardMessage::ToggleAnalysis) {
                        self.settings.engine.enabled = !self.settings.engine.enabled;
                        if let Err(e) = self.save_settings_and_apply() {
                            return Task::done(Message::ShowError(AppError::new(
                                "Settings save failed",
                                e,
                            )));
                        }
                        return Task::none();
                    }
                    if let quick_board::QuickBoardMessage::SaveAsStudy(moves) = msg {
                        return Task::done(Message::GoToStudy(StudyInit::FromMoves(moves)));
                    }
                    if let quick_board::QuickBoardMessage::ReviewLine(moves) = msg {
                        let review = GameReview::from_moves("Quick Board Review", moves);
                        if let Err(e) = self.shutdown_current_mode() {
                            return Task::done(Message::ShowError(AppError::new("Save failed", e)));
                        }
                        return self.set_game_review_mode(review);
                    }
                    if matches!(msg, quick_board::QuickBoardMessage::OpenEngineSettings) {
                        return Task::done(Message::RightPanel(RightPanelMessage::SwitchTab(
                            RightPanelTab::Settings,
                        )));
                    }
                    if let Mode::QuickBoard(mode) = &mut self.mode {
                        return Self::map_mode_task(mode.update(msg), ModeMessage::QuickBoard);
                    }
                }
                ModeMessage::Study(study_msg) => {
                    if matches!(study_msg, study::StudyMessage::ToggleAnalysis) {
                        self.settings.engine.enabled = !self.settings.engine.enabled;
                        if let Err(e) = self.save_settings_and_apply() {
                            return Task::done(Message::ShowError(AppError::new(
                                "Settings save failed",
                                e,
                            )));
                        }
                        return Task::none();
                    }
                    if let Mode::Study(mode) = &mut self.mode {
                        if matches!(study_msg, study::StudyMessage::Save) {
                            return Task::done(Message::GlobalSave);
                        }
                        if matches!(study_msg, study::StudyMessage::ReviewThisLine) {
                            let review_name = format!("Review {}", mode.study.name);
                            let review_tree = mode.study.tree.clone();
                            if let Err(e) = self.shutdown_current_mode() {
                                return Task::done(Message::ShowError(AppError::new(
                                    "Save failed",
                                    e,
                                )));
                            }
                            let review = GameReview::from_tree(&review_name, review_tree);
                            return self.set_game_review_mode(review);
                        }
                        if matches!(study_msg, study::StudyMessage::OpenEngineSettings) {
                            return Task::done(Message::RightPanel(RightPanelMessage::SwitchTab(
                                RightPanelTab::Settings,
                            )));
                        }
                        return Self::map_mode_task(mode.update(study_msg), ModeMessage::Study);
                    }
                }
                ModeMessage::GameReview(msg) => {
                    if matches!(msg, game_review::GameReviewMessage::ToggleAnalysis) {
                        self.settings.engine.enabled = !self.settings.engine.enabled;
                        if let Err(e) = self.save_settings_and_apply() {
                            return Task::done(Message::ShowError(AppError::new(
                                "Settings save failed",
                                e,
                            )));
                        }
                        return Task::none();
                    }
                    if let Mode::GameReview(mode) = &mut self.mode {
                        if matches!(msg, game_review::GameReviewMessage::Save) {
                            return Task::done(Message::GlobalSave);
                        }
                        if matches!(msg, game_review::GameReviewMessage::OpenInStudy) {
                            let study_name = mode.data.name.clone();
                            let study_tree = mode.data.tree.clone();
                            if let Err(e) = self.shutdown_current_mode() {
                                return Task::done(Message::ShowError(AppError::new(
                                    "Save failed",
                                    e,
                                )));
                            }
                            let study = Study::from_tree(&study_name, study_tree);
                            self.set_study_mode(study);
                            return Task::done(Message::ShowToast(
                                ToastType::Info,
                                "Opened in Study".to_string(),
                            ));
                        }
                        if matches!(msg, game_review::GameReviewMessage::OpenEngineSettings) {
                            return Task::done(Message::RightPanel(RightPanelMessage::SwitchTab(
                                RightPanelTab::Settings,
                            )));
                        }
                        return Self::map_mode_task(mode.update(msg), ModeMessage::GameReview);
                    }
                }
                ModeMessage::Trivia(trivia_msg) => {
                    if let Mode::Trivia(trivia_mode) = &mut self.mode {
                        if matches!(trivia_msg, trivia::TriviaMessage::AnalyzeOpening)
                            && let Some(task) =
                                Self::try_go_to_study_from_moves(trivia_mode.get_opening_moves())
                        {
                            return task;
                        }
                        return Self::map_mode_task(
                            trivia_mode.update(trivia_msg),
                            ModeMessage::Trivia,
                        );
                    }
                }
                ModeMessage::Chessle(chessle_msg) => {
                    if let Mode::Chessle(chessle_mode) = &mut self.mode {
                        if let chessle::ChessleMessage::Notify(kind, msg) = &chessle_msg {
                            return Task::done(Message::ShowToast(kind.clone(), msg.clone()));
                        }
                        if matches!(chessle_msg, chessle::ChessleMessage::AnalyzeOpening)
                            && let Some(task) =
                                Self::try_go_to_study_from_moves(chessle_mode.get_opening_moves())
                        {
                            return task;
                        }
                        return Self::map_mode_task(
                            chessle_mode.update(chessle_msg),
                            ModeMessage::Chessle,
                        );
                    }
                }
            },

            Message::StudyLoaded(study_opt) => {
                if let Some(study) = study_opt {
                    let name = study.name.clone();
                    let path = study.file_path.clone();

                    // Add to recent
                    if let Some(p) = path {
                        self.settings.add_recent_item(p);
                        if let Err(e) = self.settings.save() {
                            return Task::done(Message::ShowError(AppError::new(
                                "Settings save failed",
                                e,
                            )));
                        }
                    }

                    self.set_study_mode(*study);
                    return Task::done(Message::ShowToast(
                        ToastType::Success,
                        format!("Loaded: {}", name),
                    ));
                }
            }

            Message::ReviewLoaded(review_opt) => {
                if let Some(review) = review_opt {
                    let name = review.name.clone();
                    let path = review.file_path.clone();

                    if let Some(p) = path {
                        self.settings.add_recent_item(p);
                        if let Err(e) = self.settings.save() {
                            return Task::done(Message::ShowError(AppError::new(
                                "Settings save failed",
                                e,
                            )));
                        }
                    }

                    let follow_up = self.set_game_review_mode(*review);
                    return Task::batch(vec![
                        follow_up,
                        Task::done(Message::ShowToast(
                            ToastType::Success,
                            format!("Loaded: {}", name),
                        )),
                    ]);
                }
            }

            Message::TopBar(top_msg) => match top_msg {
                TopBarMessage::SelectMode(mode_type) => {
                    if mode_type != self.current_mode_type() {
                        return Task::done(Message::SwitchMode(mode_type));
                    }
                }
                TopBarMessage::ToggleSidebar => {
                    self.settings.library_sidebar_open = !self.settings.library_sidebar_open;
                    if let Err(e) = self.settings.save() {
                        return Task::done(Message::ShowError(AppError::new(
                            "Settings save failed",
                            e,
                        )));
                    }
                    self.apply_current_window_layout();
                }
                TopBarMessage::OpenHelp => {
                    self.right_panel = self.right_panel.toggle(RightPanelTab::Help);
                    self.apply_current_window_layout();
                }
                TopBarMessage::OpenSettings => {
                    self.right_panel = self.right_panel.toggle(RightPanelTab::Settings);
                    self.apply_current_window_layout();
                }
                TopBarMessage::ToggleTheme => {
                    return Task::done(Message::ToggleTheme);
                }
            },

            Message::RightPanel(panel_msg) => {
                match panel_msg {
                    RightPanelMessage::Close => {
                        self.right_panel = RightPanelTab::None;
                        self.apply_current_window_layout();
                    }
                    RightPanelMessage::SwitchTab(tab) => {
                        self.right_panel = tab;
                        self.apply_current_window_layout();
                    }
                    RightPanelMessage::ToggleLayoutMode => {
                        return Task::done(Message::ToggleLayoutMode);
                    }
                    RightPanelMessage::ToggleAutoLayout => {
                        return Task::done(Message::ToggleAutoLayout);
                    }
                    RightPanelMessage::ToggleAnimationSpeed => {
                        return Task::done(Message::ToggleAnimationSpeed);
                    }
                    RightPanelMessage::ChooseLibraryRoot => {
                        return Task::perform(
                            async {
                                rfd::AsyncFileDialog::new()
                                    .set_title("Choose Tactica library")
                                    .pick_folder()
                                    .await
                                    .map(|handle| handle.path().to_path_buf())
                            },
                            Message::LibraryRootSelected,
                        );
                    }
                    RightPanelMessage::ToggleEvalBar => {
                        self.settings.show_eval_bar = !self.settings.show_eval_bar;
                        if let Err(e) = self.save_settings_and_apply() {
                            return Task::done(Message::ShowError(AppError::new(
                                "Settings save failed",
                                e,
                            )));
                        }
                        return Task::done(Message::ShowToast(
                            ToastType::Info,
                            if self.settings.show_eval_bar {
                                "Evaluation Bar: On".to_string()
                            } else {
                                "Evaluation Bar: Off".to_string()
                            },
                        ));
                    }
                    RightPanelMessage::CycleBoardTheme => {
                        self.settings.board_theme = self.settings.board_theme.cycle();
                        if let Err(e) = self.save_settings_and_apply() {
                            return Task::done(Message::ShowError(AppError::new(
                                "Settings save failed",
                                e,
                            )));
                        }
                        return Task::done(Message::ShowToast(
                            ToastType::Info,
                            format!("Board Theme: {}", self.settings.board_theme.label()),
                        ));
                    }
                    // Engine settings
                    RightPanelMessage::ToggleEngine => {
                        self.settings.engine.enabled = !self.settings.engine.enabled;
                        if let Err(e) = self.save_settings_and_apply() {
                            return Task::done(Message::ShowError(AppError::new(
                                "Settings save failed",
                                e,
                            )));
                        }
                        return Task::done(Message::ShowToast(
                            ToastType::Info,
                            if self.settings.engine.enabled {
                                "Engine enabled".to_string()
                            } else {
                                "Engine disabled".to_string()
                            },
                        ));
                    }
                    RightPanelMessage::SetMultiPV(pv) => {
                        return self.update_engine_setting_analysis(|settings| {
                            if settings.engine.multi_pv == pv {
                                false
                            } else {
                                settings.engine.multi_pv = pv;
                                true
                            }
                        });
                    }
                    RightPanelMessage::SetMaxDepth(depth) => {
                        return self.update_engine_setting_analysis(|settings| {
                            if settings.engine.max_depth == depth {
                                false
                            } else {
                                settings.engine.max_depth = depth;
                                true
                            }
                        });
                    }
                    RightPanelMessage::SetThreads(threads) => {
                        return self.update_engine_setting_analysis(|settings| {
                            if settings.engine.threads == threads {
                                false
                            } else {
                                settings.engine.threads = threads;
                                true
                            }
                        });
                    }
                    RightPanelMessage::SetHashMB(hash) => {
                        return self.update_engine_setting_analysis(|settings| {
                            if settings.engine.hash_mb == hash {
                                false
                            } else {
                                settings.engine.hash_mb = hash;
                                true
                            }
                        });
                    }
                }
            }

            Message::Library(lib_msg) => {
                match lib_msg {
                    LibrarySidebarMessage::Toggle => {
                        self.settings.library_sidebar_open = !self.settings.library_sidebar_open;
                        if let Err(e) = self.settings.save() {
                            return Task::done(Message::ShowError(AppError::new(
                                "Settings save failed",
                                e,
                            )));
                        }
                        self.apply_current_window_layout();
                    }
                    LibrarySidebarMessage::SearchChanged(query) => {
                        self.library_search = query;
                    }
                    LibrarySidebarMessage::SetFilter(filter) => {
                        self.library_filter = filter;
                    }
                    LibrarySidebarMessage::ToggleFolder(path) => {
                        self.library.toggle_folder(&path);
                    }
                    LibrarySidebarMessage::OpenFile(path, kind) => {
                        return match kind {
                            EntryKind::Study => Task::perform(load_study(path), |s| {
                                Message::StudyLoaded(s.map(Box::new))
                            }),
                            EntryKind::Review => Task::perform(load_review(path), |r| {
                                Message::ReviewLoaded(r.map(Box::new))
                            }),
                        };
                    }
                    LibrarySidebarMessage::CreateNew => {
                        // Save current mode first
                        if let Err(e) = self.shutdown_current_mode() {
                            return Task::done(Message::ShowError(AppError::new("Save failed", e)));
                        }

                        let study = Study::new(&format!("Study {}", timestamp()));
                        self.set_study_mode(study);
                    }
                    LibrarySidebarMessage::CreateFolder => {
                        let _ = self.library.create_folder("New Folder");
                    }
                    LibrarySidebarMessage::Import => {
                        self.show_import_modal = true;
                        self.import_input = text_editor::Content::new();
                        self.import_error = None;
                    }
                    LibrarySidebarMessage::Refresh => {
                        self.library.refresh();
                    }
                    LibrarySidebarMessage::DeleteEntry(path) => {
                        // Show confirmation dialog instead of deleting immediately
                        self.confirm_delete_path = Some(path);
                    }
                    LibrarySidebarMessage::ToggleFavorite(path) => {
                        match self.library.toggle_favorite(&path) {
                            Ok(true) => {
                                return Task::done(Message::ShowToast(
                                    ToastType::Info,
                                    "Added to favorites".to_string(),
                                ));
                            }
                            Ok(false) => {
                                return Task::done(Message::ShowToast(
                                    ToastType::Info,
                                    "Removed from favorites".to_string(),
                                ));
                            }
                            Err(e) => {
                                return Task::done(Message::ShowError(AppError::new(
                                    "Favorite failed",
                                    e,
                                )));
                            }
                        }
                    }
                }
            }

            Message::ToggleLayoutMode => {
                use config::LayoutMode;
                self.settings.layout_mode = match self.settings.layout_mode {
                    LayoutMode::Horizontal => LayoutMode::Vertical,
                    LayoutMode::Vertical => LayoutMode::Horizontal,
                };
                self.settings.auto_layout = false;
                self.apply_current_window_layout();
                if let Err(e) = self.save_settings_and_apply() {
                    return Task::done(Message::ShowError(AppError::new(
                        "Settings save failed",
                        e,
                    )));
                }
                let msg = format!("Layout: {:?}", self.settings.layout_mode);
                return Task::done(Message::ShowToast(ToastType::Info, msg));
            }
            Message::ToggleAutoLayout => {
                self.settings.auto_layout = !self.settings.auto_layout;
                self.apply_current_window_layout();
                if let Err(e) = self.save_settings_and_apply() {
                    return Task::done(Message::ShowError(AppError::new(
                        "Settings save failed",
                        e,
                    )));
                }
                let msg = if self.settings.auto_layout {
                    "Auto-layout enabled"
                } else {
                    "Auto-layout disabled"
                };
                return Task::done(Message::ShowToast(ToastType::Info, msg.to_string()));
            }
            Message::ToggleAnimationSpeed => {
                self.settings.animation_speed = self.settings.animation_speed.cycle();
                if let Err(e) = self.save_settings_and_apply() {
                    return Task::done(Message::ShowError(AppError::new(
                        "Settings save failed",
                        e,
                    )));
                }
                let msg = format!("Animation: {}", self.settings.animation_speed.label());
                return Task::done(Message::ShowToast(ToastType::Info, msg));
            }
            Message::ToggleTheme => {
                self.theme = match self.theme {
                    Theme::Light => Theme::Dark,
                    _ => Theme::Light,
                };
                let msg = format!("Theme: {:?}", self.theme);
                return Task::done(Message::ShowToast(ToastType::Info, msg));
            }

            Message::SwitchMode(mode_type) => {
                if self.mode.has_pending_action() {
                    self.show_mode_switch_confirm = true;
                    self.pending_mode_switch = Some(mode_type);
                } else {
                    return self.do_switch_mode(mode_type);
                }
            }
            Message::ConfirmModeSwitch => {
                self.show_mode_switch_confirm = false;
                if let Some(mode_type) = self.pending_mode_switch.take() {
                    return self.do_switch_mode(mode_type);
                }
            }
            Message::CancelModeSwitch => {
                self.show_mode_switch_confirm = false;
                self.pending_mode_switch = None;
            }

            Message::GoToStudy(init) => {
                // Shutdown current mode
                if let Err(e) = self.shutdown_current_mode() {
                    return Task::done(Message::ShowError(AppError::new("Save failed", e)));
                }

                let study = match init.into_study("From Game") {
                    Ok(s) => s,
                    Err(e) => {
                        return Task::done(Message::ShowError(AppError::new("Import failed", e)));
                    }
                };

                self.set_study_mode(study);
            }

            Message::ConfirmDelete(path) => {
                self.confirm_delete_path = Some(path);
            }
            Message::CancelDelete => {
                self.confirm_delete_path = None;
            }
            Message::ExecuteDelete => {
                if let Some(path) = self.confirm_delete_path.take() {
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("file")
                        .to_string();

                    if let Err(e) = self.library.delete(&path) {
                        return Task::done(Message::ShowError(AppError::new("Delete failed", e)));
                    }
                    self.library.refresh();
                    return Task::done(Message::ShowToast(
                        ToastType::Info,
                        format!("Deleted: {}", name),
                    ));
                }
            }

            Message::ShowImportModal => {
                self.show_import_modal = true;
                self.import_input = text_editor::Content::new();
                self.import_error = None;
                self.import_target = ImportTarget::Study;
            }
            Message::HideImportModal => {
                self.show_import_modal = false;
                self.import_input = text_editor::Content::new();
                self.import_error = None;
                self.import_target = ImportTarget::Study;
            }
            Message::SetImportTarget(target) => {
                self.import_target = target;
            }
            Message::ImportInputChanged(action) => {
                self.import_input.perform(action);
                self.import_error = None;
            }
            Message::ConfirmImport => {
                let input = self.import_input.text();
                let input = input.trim();
                if input.is_empty() {
                    self.import_error = Some("Please enter FEN or PGN".to_string());
                    return Task::none();
                }

                let import_result: Result<(Task<Message>, Option<String>), String> =
                    match self.import_target {
                        ImportTarget::Study => match parse_import_study(input) {
                            Ok((study, warning)) => {
                                if let Err(e) = self.shutdown_current_mode() {
                                    return Task::done(Message::ShowError(AppError::new(
                                        "Save failed",
                                        e,
                                    )));
                                }
                                self.set_study_mode(study);
                                Ok((Task::none(), warning))
                            }
                            Err(e) => Err(e),
                        },
                        ImportTarget::GameReview => match parse_import_review(input) {
                            Ok((review, warning)) => {
                                if let Err(e) = self.shutdown_current_mode() {
                                    return Task::done(Message::ShowError(AppError::new(
                                        "Save failed",
                                        e,
                                    )));
                                }
                                Ok((self.set_game_review_mode(review), warning))
                            }
                            Err(e) => Err(e),
                        },
                    };

                match import_result {
                    Ok((follow_up, warning)) => {
                        self.show_import_modal = false;
                        self.import_input = text_editor::Content::new();
                        self.import_error = None;

                        let mut tasks = vec![
                            follow_up,
                            Task::done(Message::ShowToast(
                                ToastType::Success,
                                "Imported successfully".to_string(),
                            )),
                        ];
                        if let Some(warning) = warning {
                            tasks.push(Task::done(Message::ShowToast(ToastType::Warning, warning)));
                        }
                        return Task::batch(tasks);
                    }
                    Err(e) => self.import_error = Some(e),
                }
            }

            Message::LibraryRootSelected(root) => {
                if let Some(root) = root {
                    return self.apply_library_root(root);
                }
            }

            Message::GlobalHome => {
                self.mode.navigate_home();
            }
            Message::GlobalEnd => {
                self.mode.navigate_end();
            }
            Message::GlobalFlipBoard => {
                if !self.right_panel.is_open()
                    && let Some(board) = self.mode.board_mut()
                {
                    board.flipped = !board.flipped;
                }
            }
            Message::GlobalNewGame => match &mut self.mode {
                Mode::Study(_) => {
                    return Task::done(Message::Library(LibrarySidebarMessage::CreateNew));
                }
                Mode::QuickBoard(m) => {
                    return m
                        .update(quick_board::QuickBoardMessage::ResetBoard)
                        .map(|m| Message::Mode(ModeMessage::QuickBoard(m)));
                }
                Mode::GameReview(_) => {
                    // No "new game" concept for review
                }
                Mode::Trivia(m) => {
                    return m
                        .update(trivia::TriviaMessage::RequestNewTrivia)
                        .map(|m| Message::Mode(ModeMessage::Trivia(m)));
                }
                Mode::Chessle(m) => {
                    return m
                        .update(chessle::ChessleMessage::NewGame)
                        .map(|m| Message::Mode(ModeMessage::Chessle(m)));
                }
            },
            Message::GlobalSave => {
                // Save whichever saveable mode is active
                let save_result = match &mut self.mode {
                    Mode::Study(mode) => Some(mode.study.save()),
                    Mode::GameReview(mode) => Some(mode.data.save()),
                    _ => None,
                };
                if let Some(result) = save_result {
                    match result {
                        Ok(_) => {
                            self.library.refresh();
                            return Task::done(Message::ShowToast(
                                ToastType::Success,
                                "Saved".to_string(),
                            ));
                        }
                        Err(e) => {
                            return Task::done(Message::ShowError(AppError::new("Save failed", e)));
                        }
                    }
                }
            }

            Message::ShowError(error) => {
                self.error = Some(error);
            }
            Message::DismissError => {
                self.error = None;
            }
            Message::ShowToast(kind, msg) => {
                self.toasts.add(msg, kind);
            }
            Message::Tick => {
                self.toasts.update();
            }
        }
        Task::none()
    }

    fn do_switch_mode(&mut self, mode_type: ModeType) -> Task<Message> {
        // Shut down and save current mode if applicable
        if let Err(e) = self.shutdown_current_mode() {
            return Task::done(Message::ShowError(AppError::new("Save failed", e)));
        }

        match mode_type {
            ModeType::QuickBoard => {
                self.mode = Mode::QuickBoard(Box::new(QuickBoardMode::new(self.settings.clone())));
            }
            ModeType::Study => {
                let study = Study::new(&format!("Study {}", timestamp()));
                self.set_study_mode(study);
            }
            ModeType::GameReview => {
                // Game Review starts empty — user imports a game
                let data = crate::core::game_review::GameReview::empty();
                return self.set_game_review_mode(data);
            }
            ModeType::Trivia => {
                self.mode = Mode::Trivia(TriviaMode::new(self.settings.clone()));
            }
            ModeType::Chessle => {
                self.mode = Mode::Chessle(ChessleMode::new(self.settings.clone()));
            }
        }
        self.apply_current_window_layout();
        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        let mode_sub = match &self.mode {
            Mode::QuickBoard(mode) => mode
                .subscription()
                .map(|m| Message::Mode(ModeMessage::QuickBoard(m))),
            Mode::Study(mode) => mode
                .subscription()
                .map(|m| Message::Mode(ModeMessage::Study(m))),
            Mode::GameReview(mode) => mode
                .subscription()
                .map(|m| Message::Mode(ModeMessage::GameReview(m))),
            Mode::Trivia(mode) => mode
                .subscription()
                .map(|m| Message::Mode(ModeMessage::Trivia(m))),
            Mode::Chessle(mode) => mode
                .subscription()
                .map(|m| Message::Mode(ModeMessage::Chessle(m))),
        };

        Subscription::batch(vec![
            iced::time::every(std::time::Duration::from_millis(style::ENGINE_POLL_MS))
                .map(|_| Message::Tick),
            iced::event::listen_with(|event, _status, _window| match event {
                iced::Event::Window(iced::window::Event::Opened { size, .. }) => {
                    Some(Message::WindowResized(size.width, size.height))
                }
                iced::Event::Window(iced::window::Event::Resized(size)) => {
                    Some(Message::WindowResized(size.width, size.height))
                }
                iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key, modifiers, ..
                }) => {
                    if GlobalHotkey::Save.matches(&key, modifiers) {
                        return Some(Message::GlobalSave);
                    }
                    if GlobalHotkey::NewGame.matches(&key, modifiers) {
                        return Some(Message::GlobalNewGame);
                    }
                    if GlobalHotkey::Import.matches(&key, modifiers) {
                        return Some(Message::Library(LibrarySidebarMessage::Import));
                    }
                    if GlobalHotkey::ToggleLibrary.matches(&key, modifiers) {
                        return Some(Message::Library(LibrarySidebarMessage::Toggle));
                    }
                    if GlobalHotkey::NavigateHome.matches(&key, modifiers) {
                        return Some(Message::GlobalHome);
                    }
                    if GlobalHotkey::NavigateEnd.matches(&key, modifiers) {
                        return Some(Message::GlobalEnd);
                    }
                    if GlobalHotkey::FlipBoard.matches(&key, modifiers) {
                        return Some(Message::GlobalFlipBoard);
                    }
                    if GlobalHotkey::ClosePanel.matches(&key, modifiers) {
                        return Some(Message::RightPanel(RightPanelMessage::Close));
                    }
                    None
                }
                _ => None,
            }),
            mode_sub,
        ])
    }

    fn view(&self) -> Element<'_, Message> {
        // Mode content
        let mode_content: Element<'_, ModeMessage> = match &self.mode {
            Mode::QuickBoard(mode) => mode.view(&self.theme).map(ModeMessage::QuickBoard),
            Mode::Study(mode) => mode.view(&self.theme).map(ModeMessage::Study),
            Mode::GameReview(mode) => mode.view(&self.theme).map(ModeMessage::GameReview),
            Mode::Trivia(mode) => mode.view(&self.theme).map(ModeMessage::Trivia),
            Mode::Chessle(mode) => mode.view(&self.theme).map(ModeMessage::Chessle),
        };
        let mode_content = mode_content.map(Message::Mode);

        // Build top_bar
        let top_bar =
            top_bar::build_top_bar(&self.theme, self.current_mode_type(), Message::TopBar);

        // Build library sidebar (available in all modes)
        let library_sidebar: Element<'_, Message> = library_sidebar::build_library_sidebar(
            &self.theme,
            &self.library,
            &self.library_search,
            self.settings.library_sidebar_open,
            &self.settings.recent_items,
            self.library_filter,
            Message::Library,
        );

        // Get mode info for panels
        let instructions = self.mode.instructions();
        let hotkeys = self.mode.active_hotkeys();
        let has_board = self.mode.has_board();

        // Build right panel
        let is_analysis_mode = matches!(
            self.mode,
            Mode::Study(_) | Mode::QuickBoard(_) | Mode::GameReview(_)
        );
        let is_game_review_mode = matches!(self.mode, Mode::GameReview(_));
        let right_panel = right_panel::build_right_panel(
            &self.theme,
            self.right_panel,
            RightPanelContext {
                settings: &self.settings,
                instructions,
                hotkeys,
                has_board,
                is_analysis_mode,
                is_game_review_mode,
            },
            Message::RightPanel,
        );

        // Main layout: sidebar | content | right panel
        let main_row = row![
            library_sidebar,
            container(mode_content)
                .width(Length::Fill)
                .height(Length::Fill),
            right_panel,
        ]
        .width(Length::Fill)
        .height(Length::Fill);

        // Full layout: top bar + main row
        let full_layout = column![top_bar, main_row,]
            .width(Length::Fill)
            .height(Length::Fill);

        let with_toasts = stack![full_layout, self.toasts.view(&self.theme)]
            .width(Length::Fill)
            .height(Length::Fill);

        // Layer modals on top
        let final_content: Element<'_, Message> = if let Some(ref error) = self.error {
            stack![with_toasts, self.build_error_modal(&self.theme, error)]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else if self.show_mode_switch_confirm {
            stack![
                with_toasts,
                self.build_mode_switch_confirm_modal(&self.theme)
            ]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        } else if let Some(ref path) = self.confirm_delete_path {
            stack![
                with_toasts,
                self.build_delete_confirm_modal(&self.theme, path)
            ]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        } else if self.show_import_modal {
            stack![with_toasts, self.build_import_modal(&self.theme)]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            with_toasts.into()
        };

        container(final_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(style::containers::background)
            .into()
    }

    fn build_error_modal<'a>(&'a self, theme: &Theme, error: &'a AppError) -> Element<'a, Message> {
        let s = self.settings.ui_scale;

        let content = column![
            text(&error.title)
                .size(20.0 * s)
                .color(style::Palette::error(theme)),
            text(&error.message)
                .size(14.0 * s)
                .color(style::Palette::text_secondary(theme)),
            button(text("OK").size(14.0 * s))
                .padding([8, 24])
                .width(Length::Fill)
                .style(style::buttons::primary)
                .on_press(Message::DismissError),
        ]
        .spacing(16)
        .align_x(Alignment::Center)
        .width(300.0);

        modal(content)
    }

    fn build_mode_switch_confirm_modal(&self, theme: &Theme) -> Element<'_, Message> {
        let s = self.settings.ui_scale;

        let content = column![
            text("Switch Mode?")
                .size(20.0 * s)
                .color(style::Palette::text_primary(theme)),
            text("You have unsaved changes that may be lost.")
                .size(14.0 * s)
                .color(style::Palette::text_muted(theme)),
            confirm_cancel_row(
                Message::CancelModeSwitch,
                Message::ConfirmModeSwitch,
                "Switch",
                s
            ),
        ]
        .spacing(16)
        .align_x(Alignment::Center);

        modal(content)
    }

    fn build_delete_confirm_modal(&self, theme: &Theme, path: &Path) -> Element<'_, Message> {
        let s = self.settings.ui_scale;
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("this file");

        let content = column![
            text("Delete Study?")
                .size(20.0 * s)
                .color(style::Palette::text_primary(theme)),
            text(format!("Are you sure you want to delete \"{}\"?", name))
                .size(14.0 * s)
                .color(style::Palette::text_muted(theme)),
            text("This action cannot be undone.")
                .size(12.0 * s)
                .color(style::Palette::error(theme)),
            confirm_cancel_row(Message::CancelDelete, Message::ExecuteDelete, "Delete", s),
        ]
        .spacing(12)
        .align_x(Alignment::Center);

        modal(content)
    }

    fn build_import_modal(&self, theme: &Theme) -> Element<'_, Message> {
        let s = self.settings.ui_scale;

        let study_btn = button(text("Study").size(12.0 * s))
            .padding([4, 10])
            .style(if self.import_target == ImportTarget::Study {
                style::buttons::primary
            } else {
                style::buttons::secondary
            })
            .on_press(Message::SetImportTarget(ImportTarget::Study));

        let review_btn = button(text("Game Review").size(12.0 * s))
            .padding([4, 10])
            .style(if self.import_target == ImportTarget::GameReview {
                style::buttons::primary
            } else {
                style::buttons::secondary
            })
            .on_press(Message::SetImportTarget(ImportTarget::GameReview));

        let mut content = column![
            text("Import FEN or PGN")
                .size(18.0 * s)
                .color(style::Palette::text_primary(theme)),
            text("Paste a FEN position or PGN game below:")
                .size(12.0 * s)
                .color(style::Palette::text_muted(theme)),
            row![study_btn, review_btn].spacing(8),
            text_editor(&self.import_input)
                .placeholder("Paste FEN or PGN here...")
                .size(14.0 * s)
                .padding(10)
                .height(200)
                .on_action(Message::ImportInputChanged),
        ]
        .spacing(12)
        .align_x(Alignment::Center)
        .width(400);

        // Show error if present
        if let Some(ref error) = self.import_error {
            content = content.push(
                text(error)
                    .size(12.0 * s)
                    .color(style::Palette::error(theme)),
            );
        }

        content = content.push(confirm_cancel_row(
            Message::HideImportModal,
            Message::ConfirmImport,
            "Import",
            s,
        ));

        modal(content)
    }
}

async fn load_study(path: PathBuf) -> Option<Study> {
    Study::load_from_file(&path).ok()
}

async fn load_review(path: PathBuf) -> Option<GameReview> {
    GameReview::load_from_file(&path).ok()
}

fn parse_import_study(input: &str) -> Result<(Study, Option<String>), String> {
    if pgn::looks_like_fen(input) {
        let pos = pgn::parse_fen(input)?;
        return Ok((
            Study::from_position(&format!("Import {}", timestamp()), pos),
            None,
        ));
    }

    let parsed = pgn::parse_pgn_detailed(input)?;
    let warning = parsed.warning_summary();
    let name = parsed.name;
    let tree = parsed.tree;
    Ok((Study::from_tree(&name, tree), warning))
}

fn parse_import_review(input: &str) -> Result<(GameReview, Option<String>), String> {
    if pgn::looks_like_fen(input) {
        let pos = pgn::parse_fen(input)?;
        return Ok((
            GameReview::from_position(&format!("Import {}", timestamp()), pos),
            None,
        ));
    }

    let parsed = pgn::parse_pgn_detailed(input)?;
    let warning = parsed.warning_summary();
    let name = parsed.name;
    let tree = parsed.tree;
    Ok((GameReview::from_tree(&name, tree), warning))
}

fn timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("System clock is before UNIX epoch")
        .as_secs()
        % 100000
}
