//! FolderDetailPage component - main container for folder detail view.

use crate::containers::{
    CommonHeadersPanel, CommonHeadersPanelEvent, FolderOverviewPanel, FolderOverviewPanelEvent,
    SearchParamsPanel, SearchParamsPanelEvent,
};
use crate::state::ServiceState;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::tab::{Tab, TabBar, TabVariant};
use gpui_component::StyledExt;
use models::Folder;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum FolderDetailTab {
    Overview,
    CommonHeaders,
    SearchParameters,
}

impl FolderDetailTab {
    pub fn display_name(&self) -> SharedString {
        match self {
            FolderDetailTab::Overview => shared_t!("folders.tab.overview"),
            FolderDetailTab::CommonHeaders => shared_t!("folders.tab.common_headers"),
            FolderDetailTab::SearchParameters => shared_t!("folders.tab.search_parameters"),
        }
    }

    pub fn all_tabs() -> Vec<FolderDetailTab> {
        vec![
            FolderDetailTab::Overview,
            FolderDetailTab::CommonHeaders,
            FolderDetailTab::SearchParameters,
        ]
    }
}

#[derive(Debug, Clone)]
pub enum FolderDetailPageEvent {}

#[derive(Debug, Clone)]
pub enum FolderDetailError {
    ValidationError(String),
    DatabaseError(String),
    NetworkError(String),
    NotFound(String),
    InvalidData(String),
}

impl FolderDetailError {
    pub fn message(&self) -> String {
        match self {
            FolderDetailError::ValidationError(msg) => format!("Validation Error: {}", msg),
            FolderDetailError::DatabaseError(msg) => format!("Database Error: {}", msg),
            FolderDetailError::NetworkError(msg) => format!("Network Error: {}", msg),
            FolderDetailError::NotFound(msg) => format!("Not Found: {}", msg),
            FolderDetailError::InvalidData(msg) => format!("Invalid Data: {}", msg),
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            FolderDetailError::ValidationError(_) | FolderDetailError::NetworkError(_)
        )
    }
}

pub struct FolderDetailPage {
    service_state: Entity<ServiceState>,
    folder_id: Option<Uuid>,
    /// Snapshot of the folder this view renders; used for tab title/icon.
    folder: Folder,
    active_tab: FolderDetailTab,
    overview_panel: Entity<FolderOverviewPanel>,
    headers_panel: Entity<CommonHeadersPanel>,
    params_panel: Entity<SearchParamsPanel>,
    is_loading: bool,
    error_message: Option<String>,
    last_operation: Option<String>,
}

impl EventEmitter<FolderDetailPageEvent> for FolderDetailPage {}

impl FolderDetailPage {
    pub fn new(
        service_state: Entity<ServiceState>,
        folder: Folder,
        cx: &mut Context<Self>,
    ) -> Self {
        let overview_panel = cx.new(|_| FolderOverviewPanel::new());
        let headers_panel = cx.new(|_| CommonHeadersPanel::new());
        let params_panel = cx.new(|_| SearchParamsPanel::new());

        let ss = service_state.clone();
        cx.subscribe(
            &overview_panel,
            move |this, _panel, event: &FolderOverviewPanelEvent, cx| {
                if let Some(folder_id) = this.folder_id {
                    match event {
                        FolderOverviewPanelEvent::SaveChanges(name, description) => {
                            ss.update(cx, |s, cx| {
                                s.update_folder_info(
                                    folder_id,
                                    name.clone(),
                                    description.clone(),
                                    cx,
                                );
                            });
                        }
                    }
                }
            },
        )
        .detach();

        let ss = service_state.clone();
        cx.subscribe(
            &headers_panel,
            move |this, _panel, event: &CommonHeadersPanelEvent, cx| {
                if let Some(folder_id) = this.folder_id {
                    match event {
                        CommonHeadersPanelEvent::HeadersChanged(headers) => {
                            ss.update(cx, |s, cx| {
                                s.update_folder_common_headers(folder_id, headers.clone(), cx);
                            });
                        }
                    }
                }
            },
        )
        .detach();

        let ss = service_state.clone();
        cx.subscribe(
            &params_panel,
            move |this, _panel, event: &SearchParamsPanelEvent, cx| {
                if let Some(folder_id) = this.folder_id {
                    match event {
                        SearchParamsPanelEvent::ParamsChanged(params) => {
                            ss.update(cx, |s, cx| {
                                s.update_folder_search_params(folder_id, params.clone(), cx);
                            });
                        }
                    }
                }
            },
        )
        .detach();

        // Re-renders automatically when `service_state` changes: GPUI tracks the
        // entities `read` during `render` and invalidates this view on their `notify`.
        let mut page = Self {
            service_state,
            folder_id: Some(folder.id),
            folder: folder.clone(),
            active_tab: FolderDetailTab::Overview,
            overview_panel,
            headers_panel,
            params_panel,
            is_loading: false,
            error_message: None,
            last_operation: None,
        };
        page.update_panels_with_folder(&folder, cx);
        page
    }

    fn update_panels_with_folder(&mut self, folder: &Folder, cx: &mut Context<Self>) {
        self.overview_panel.update(cx, |panel, _| {
            panel.set_folder(folder.clone());
        });
        self.headers_panel.update(cx, |panel, _| {
            panel.set_headers(folder.common_headers.clone());
        });
        self.params_panel.update(cx, |panel, _| {
            panel.set_params(folder.common_search_params.clone());
        });
    }

    pub fn set_folder(&mut self, folder: Option<Folder>, cx: &mut Context<Self>) {
        self.folder_id = folder.as_ref().map(|f| f.id);
        if let Some(folder) = &folder {
            self.update_panels_with_folder(folder, cx);
        }
        cx.notify();
    }

    pub fn get_folder_id(&self) -> Option<Uuid> {
        self.folder_id
    }

    pub fn set_active_tab(&mut self, tab: FolderDetailTab, cx: &mut Context<Self>) {
        self.active_tab = tab;
        cx.notify();
    }

    pub fn set_loading(
        &mut self,
        loading: bool,
        operation: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.is_loading = loading;
        self.last_operation = operation;
        if loading {
            self.error_message = None;
        }
        cx.notify();
    }

    pub fn set_error(&mut self, error: FolderDetailError, cx: &mut Context<Self>) {
        self.error_message = Some(error.message());
        self.is_loading = false;
        cx.notify();
    }

    pub fn clear_error(&mut self, cx: &mut Context<Self>) {
        self.error_message = None;
        cx.notify();
    }

    #[allow(unused)]
    fn validate_folder_data(&self) -> Result<(), FolderDetailError> {
        if self.folder_id.is_none() {
            return Err(FolderDetailError::NotFound(
                "No folder selected".to_string(),
            ));
        }
        Ok(())
    }

    fn render_tab_navigation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h_flex()
            .border_b_1()
            .border_color(rgb(0xe0e0e0))
            .child(
                TabBar::new("response_tabs")
                    .with_variant(TabVariant::Underline)
                    .selected_index(
                        FolderDetailTab::all_tabs()
                            .iter()
                            .position(|tab| tab == &self.active_tab)
                            .unwrap_or(0),
                    )
                    .on_click(cx.listener(|this, ix: &usize, _, cx| {
                        if let Some(tab) = FolderDetailTab::all_tabs().get(*ix) {
                            this.set_active_tab(tab.clone(), cx);
                        }
                    }))
                    .children(
                        FolderDetailTab::all_tabs()
                            .into_iter()
                            .map(|tab| Tab::new().label(tab.display_name())),
                    ),
            )
    }

    fn render_tab_content(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        match self.active_tab {
            FolderDetailTab::Overview => div().flex_1().p_1().child(self.overview_panel.clone()),
            FolderDetailTab::CommonHeaders => {
                div().flex_1().p_1().child(self.headers_panel.clone())
            }
            FolderDetailTab::SearchParameters => {
                div().flex_1().p_1().child(self.params_panel.clone())
            }
        }
    }

    fn render_error_message(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        self.error_message.as_ref().map(|error| {
            div()
                .h_flex()
                .items_center()
                .gap_3()
                .p_3()
                .mx_4()
                .bg(gpui::rgb(0xfef2f2))
                .border_1()
                .border_color(gpui::rgb(0xfecaca))
                .rounded_md()
                .child(
                    gpui_component::Icon::new(gpui_component::IconName::Settings)
                        .size_4()
                        .text_color(gpui::rgb(0xdc2626)),
                )
                .child(
                    div()
                        .flex_1()
                        .text_sm()
                        .text_color(gpui::rgb(0x991b1b))
                        .child(error.clone()),
                )
                .child(
                    Button::new("dismiss_error")
                        .ghost()
                        .compact()
                        .icon(gpui_component::IconName::Delete)
                        .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                            this.clear_error(cx);
                        })),
                )
        })
    }

    fn render_loading_indicator(&self, _cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if self.is_loading {
            Some(
                div()
                    .h_flex()
                    .items_center()
                    .gap_3()
                    .p_3()
                    .mx_4()
                    .bg(gpui::rgb(0xf0f9ff))
                    .border_1()
                    .border_color(rgb(0xbae6fd))
                    .rounded_md()
                    .child(div().w_4().h_4().bg(rgb(0x0ea5e9)).rounded_full())
                    .child(
                        div().text_sm().text_color(rgb(0x0c4a6e)).child(
                            self.last_operation
                                .as_ref()
                                .map(|op| format!("{}...", op))
                                .unwrap_or_else(|| t!("common.loading").to_string()),
                        ),
                    ),
            )
        } else {
            None
        }
    }
}

impl Render for FolderDetailPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let folder = self
            .folder_id
            .and_then(|id| self.service_state.read(cx).get_folder(id).cloned());

        if folder.is_none() {
            return div()
                .flex_1()
                .h_full()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .v_flex()
                        .items_center()
                        .gap_4()
                        .child(
                            gpui_component::Icon::new(gpui_component::IconName::Folder)
                                .size_12()
                                .text_color(gpui::rgb(0x9ca3af)),
                        )
                        .child(
                            div()
                                .text_lg()
                                .text_color(gpui::rgb(0x666666))
                                .child(shared_t!("folders.no_selected.title")),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(gpui::rgb(0x9ca3af))
                                .child(shared_t!("folders.no_selected.description")),
                        ),
                )
                .into_any_element();
        }

        div()
            .flex_1()
            .h_full()
            .py_2()
            .px_4()
            .v_flex()
            .gap_4()
            .when_some(self.render_error_message(cx), |container, error_msg| {
                container.child(error_msg)
            })
            .when_some(self.render_loading_indicator(cx), |container, loading| {
                container.child(loading)
            })
            .child(self.render_tab_navigation(cx))
            .child(self.render_tab_content(cx))
            .into_any_element()
    }
}

// Folder edits are persisted immediately via panel events, so the default
// `ContentView` save/is_modified behavior is sufficient.
impl crate::state::ContentView for FolderDetailPage {
    fn id(&self) -> Uuid {
        self.folder.id
    }

    fn title(&self) -> SharedString {
        self.folder.name.clone()
    }

    fn tab_icon(&self) -> AnyElement {
        gpui_component::IconName::Folder.into_any_element()
    }

    fn set_title(&mut self, title: SharedString, cx: &mut Context<Self>) {
        self.folder.name = title;
        cx.notify();
    }
}
