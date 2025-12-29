use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{StyledExt, Disableable};
use models::{Folder, Header, SearchParameter, Workspace};
use uuid::Uuid;

/// Tab options for the folder detail page
#[derive(Debug, Clone, PartialEq)]
pub enum FolderDetailTab {
    Overview,
    CommonHeaders,
    SearchParameters,
}

impl FolderDetailTab {
    /// Get the display name for the tab
    pub fn display_name(&self) -> &'static str {
        match self {
            FolderDetailTab::Overview => "Overview",
            FolderDetailTab::CommonHeaders => "Common Headers",
            FolderDetailTab::SearchParameters => "Search Parameters",
        }
    }

    /// Get all available tabs
    pub fn all_tabs() -> Vec<FolderDetailTab> {
        vec![
            FolderDetailTab::Overview,
            FolderDetailTab::CommonHeaders,
            FolderDetailTab::SearchParameters,
        ]
    }
}

/// Events emitted by the folder detail page
#[derive(Debug, Clone)]
pub enum FolderDetailPageEvent {
    NavigateBack,
    NavigateToFolder(Uuid),
    UpdateCommonHeaders(Uuid, Vec<Header>),
    UpdateSearchParams(Uuid, Vec<SearchParameter>),
}

/// Error types for folder detail operations
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
        matches!(self, 
            FolderDetailError::ValidationError(_) | 
            FolderDetailError::NetworkError(_)
        )
    }
}

/// Events emitted by the common headers panel
#[derive(Debug, Clone)]
pub enum CommonHeadersPanelEvent {
    HeadersChanged(Vec<Header>),
}

/// Events emitted by the search params panel
#[derive(Debug, Clone)]
pub enum SearchParamsPanelEvent {
    ParamsChanged(Vec<SearchParameter>),
}

/// Main folder detail page component
pub struct FolderDetailPage {
    folder_id: Option<Uuid>,
    folder: Option<Folder>,
    workspace: Option<Workspace>,
    active_tab: FolderDetailTab,
    overview_panel: Entity<FolderOverviewPanel>,
    headers_panel: Entity<CommonHeadersPanel>,
    params_panel: Entity<SearchParamsPanel>,
    
    // Error handling and loading states
    is_loading: bool,
    error_message: Option<String>,
    last_operation: Option<String>,
}

impl EventEmitter<FolderDetailPageEvent> for FolderDetailPage {}

impl FolderDetailPage {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let overview_panel = cx.new(|_| FolderOverviewPanel::new());
        let headers_panel = cx.new(|_| CommonHeadersPanel::new());
        let params_panel = cx.new(|_| SearchParamsPanel::new());

        // Subscribe to child panel events
        cx.subscribe(&headers_panel, |this, _panel, event: &CommonHeadersPanelEvent, cx| {
            if let Some(folder_id) = this.folder_id {
                match event {
                    CommonHeadersPanelEvent::HeadersChanged(headers) => {
                        cx.emit(FolderDetailPageEvent::UpdateCommonHeaders(folder_id, headers.clone()));
                    }
                }
            }
        }).detach();

        cx.subscribe(&params_panel, |this, _panel, event: &SearchParamsPanelEvent, cx| {
            if let Some(folder_id) = this.folder_id {
                match event {
                    SearchParamsPanelEvent::ParamsChanged(params) => {
                        cx.emit(FolderDetailPageEvent::UpdateSearchParams(folder_id, params.clone()));
                    }
                }
            }
        }).detach();

        Self {
            folder_id: None,
            folder: None,
            workspace: None,
            active_tab: FolderDetailTab::Overview,
            overview_panel,
            headers_panel,
            params_panel,
            is_loading: false,
            error_message: None,
            last_operation: None,
        }
    }

    /// Set the folder to display
    pub fn set_folder(&mut self, folder: Option<Folder>, cx: &mut Context<Self>) {
        self.folder_id = folder.as_ref().map(|f| f.id);
        self.folder = folder.clone();

        // Update child panels with the new folder data
        if let Some(folder) = &folder {
            self.overview_panel.update(cx, |panel, _| {
                panel.set_folder(folder.clone());
            });
            
            // Store headers for later initialization in the panel
            let headers = folder.common_headers.clone();
            self.headers_panel.update(cx, |panel, _| {
                panel.set_headers(headers);
            });
            
            self.params_panel.update(cx, |panel, _| {
                panel.set_params(folder.common_search_params.clone());
            });
        }

        cx.notify();
    }

    /// Set the workspace for breadcrumb navigation
    pub fn set_workspace(&mut self, workspace: Option<Workspace>) {
        self.workspace = workspace;
    }

    /// Set the active tab
    pub fn set_active_tab(&mut self, tab: FolderDetailTab, cx: &mut Context<Self>) {
        self.active_tab = tab;
        cx.notify();
    }

    /// Set loading state
    pub fn set_loading(&mut self, loading: bool, operation: Option<String>, cx: &mut Context<Self>) {
        self.is_loading = loading;
        self.last_operation = operation;
        if loading {
            self.error_message = None; // Clear errors when starting new operation
        }
        cx.notify();
    }

    /// Set error message
    pub fn set_error(&mut self, error: FolderDetailError, cx: &mut Context<Self>) {
        self.error_message = Some(error.message());
        self.is_loading = false;
        cx.notify();
    }

    /// Clear error message
    pub fn clear_error(&mut self, cx: &mut Context<Self>) {
        self.error_message = None;
        cx.notify();
    }

    /// Validate folder data before operations
    #[allow(unused)]
    fn validate_folder_data(&self) -> Result<(), FolderDetailError> {
        if self.folder.is_none() {
            return Err(FolderDetailError::NotFound("No folder selected".to_string()));
        }
        
        if self.folder_id.is_none() {
            return Err(FolderDetailError::InvalidData("Invalid folder ID".to_string()));
        }
        
        Ok(())
    }

    /// Build breadcrumb navigation path for the current folder
    fn build_breadcrumb_path(&self) -> Vec<(Uuid, String)> {
        let mut path = Vec::new();
        
        if let (Some(workspace), Some(current_folder)) = (&self.workspace, &self.folder) {
            let mut folder_id = Some(current_folder.id);
            
            // Build path from current folder up to root
            while let Some(id) = folder_id {
                if let Some(folder) = workspace.folders.iter().find(|f| f.id == id) {
                    path.push((folder.id, folder.name.to_string()));
                    folder_id = folder.parent_id;
                } else {
                    break;
                }
            }
            
            // Reverse to show root -> current
            path.reverse();
        }
        
        path
    }

    /// Render breadcrumb navigation
    fn render_breadcrumb_navigation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let breadcrumb_path = self.build_breadcrumb_path();
        
        if breadcrumb_path.is_empty() {
            return div().into_any_element();
        }

        let path_len = breadcrumb_path.len();

        div()
            .h_flex()
            .items_center()
            .gap_2()
            .text_sm()
            .text_color(gpui::rgb(0x666666))
            .children(
                breadcrumb_path.into_iter().enumerate().map(|(index, (folder_id, folder_name))| {
                    let is_current = Some(folder_id) == self.folder_id;
                    
                    div()
                        .h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            if is_current {
                                // Current folder - not clickable
                                div()
                                    .text_color(gpui::rgb(0x333333))
                                    .font_medium()
                                    .child(folder_name)
                                    .into_any_element()
                            } else {
                                // Parent folder - clickable
                                Button::new(format!("breadcrumb_{}", folder_id))
                                    .ghost()
                                    .compact()
                                    .label(folder_name)
                                    .on_click(cx.listener(move |_this, _, _, cx| {
                                        cx.emit(FolderDetailPageEvent::NavigateToFolder(folder_id));
                                    }))
                                    .into_any_element()
                            }
                        )
                        .when(index < path_len - 1, |container| {
                            container.child(
                                gpui_component::Icon::new(gpui_component::IconName::ChevronRight)
                                    .size_3()
                                    .text_color(gpui::rgb(0x999999))
                            )
                        })
                })
            )
            .into_any_element()
    }

    /// Render the tab navigation
    fn render_tab_navigation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h_flex()
            .border_b_1()
            .border_color(gpui::rgb(0xe0e0e0))
            .children(
                FolderDetailTab::all_tabs().into_iter().map(|tab| {
                    let is_active = tab == self.active_tab;
                    let tab_clone = tab.clone();
                    
                    Button::new(format!("tab_{:?}", tab))
                        .px_4()
                        .py_2()
                        .rounded_none()
                        .border_b_2()
                        .when(is_active, |btn: Button| {
                            btn.primary()
                                .border_color(gpui::rgb(0x007bff))
                        })
                        .when(!is_active, |btn: Button| {
                            btn.ghost()
                                .border_color(gpui::hsla(0.0, 0.0, 0.0, 0.0))
                        })
                        .label(tab.display_name())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_active_tab(tab_clone.clone(), cx);
                        }))
                })
            )
    }

    /// Render the active tab content
    fn render_tab_content(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        match self.active_tab {
            FolderDetailTab::Overview => {
                div()
                    .flex_1()
                    .p_4()
                    .child(self.overview_panel.clone())
            }
            FolderDetailTab::CommonHeaders => {
                div()
                    .flex_1()
                    .p_4()
                    .child(self.headers_panel.clone())
            }
            FolderDetailTab::SearchParameters => {
                div()
                    .flex_1()
                    .p_4()
                    .child(self.params_panel.clone())
            }
        }
    }

    /// Render the header with folder name and navigation
    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let folder_name = self.folder
            .as_ref()
            .map(|f| f.name.to_string())
            .unwrap_or_else(|| "Folder Details".to_string());
        
        div()
            .v_flex()
            .gap_2()
            .p_4()
            .border_b_1()
            .border_color(gpui::rgb(0xe0e0e0))
            .child(
                // Top row with back button and folder name
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap_3()
                            .child(
                                Button::new("back_button")
                                    .ghost()
                                    .icon(gpui_component::IconName::ChevronLeft)
                                    .label("Back to Services")
                                    .on_click(cx.listener(|_this, _, _, cx| {
                                        cx.emit(FolderDetailPageEvent::NavigateBack);
                                    }))
                            )
                            .child(
                                div()
                                    .text_xl()
                                    .font_semibold()
                                    .child(folder_name.clone())
                            )
                    )
                    .child(
                        // Folder switching info - simplified to avoid lifetime issues
                        div()
                            .h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(gpui::rgb(0x666666))
                                    .child("Use breadcrumbs below to navigate")
                            )
                    )
            )
            .child(
                // Breadcrumb navigation row
                div()
                    .pl_12() // Align with content below back button
                    .child(self.render_breadcrumb_navigation(cx))
            )
    }

    /// Render error message if present
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
                        .text_color(gpui::rgb(0xdc2626))
                )
                .child(
                    div()
                        .flex_1()
                        .text_sm()
                        .text_color(gpui::rgb(0x991b1b))
                        .child(error.clone())
                )
                .child(
                    Button::new("dismiss_error")
                        .ghost()
                        .compact()
                        .icon(gpui_component::IconName::Delete)
                        .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                            this.clear_error(cx);
                        }))
                )
        })
    }

    /// Render loading indicator if present
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
                    .border_color(gpui::rgb(0xbae6fd))
                    .rounded_md()
                    .child(
                        // Simple loading indicator - in a real app you'd use a spinner
                        div()
                            .w_4()
                            .h_4()
                            .bg(gpui::rgb(0x0ea5e9))
                            .rounded_full()
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(gpui::rgb(0x0c4a6e))
                            .child(
                                self.last_operation
                                    .as_ref()
                                    .map(|op| format!("{}...", op))
                                    .unwrap_or_else(|| "Loading...".to_string())
                            )
                    )
            )
        } else {
            None
        }
    }
}

impl Render for FolderDetailPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.folder.is_none() {
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
                                .text_color(gpui::rgb(0x9ca3af))
                        )
                        .child(
                            div()
                                .text_lg()
                                .text_color(gpui::rgb(0x666666))
                                .child("No folder selected")
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(gpui::rgb(0x9ca3af))
                                .child("Select a folder from the sidebar to view its details")
                        )
                )
                .into_any_element();
        }

        div()
            .flex_1()
            .h_full()
            .v_flex()
            .child(self.render_header(cx))
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

/// Overview panel showing basic folder information
pub struct FolderOverviewPanel {
    folder: Option<Folder>,
}

impl FolderOverviewPanel {
    pub fn new() -> Self {
        Self { folder: None }
    }

    pub fn set_folder(&mut self, folder: Folder) {
        self.folder = Some(folder);
    }

    fn format_timestamp(timestamp: i64) -> String {
        // Convert timestamp to readable format
        // In a real implementation, you'd use chrono or similar
        format!("Timestamp: {}", timestamp)
    }
}

impl Render for FolderOverviewPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let Some(folder) = &self.folder else {
            return div()
                .child("No folder information available")
                .into_any_element();
        };

        let folder_name = folder.name.to_string();
        let folder_type_str = match folder.folder_type {
            models::FolderType::Service => "Service Folder",
            models::FolderType::Schema => "Schema Folder",
        };
        let description = folder.description.clone();
        let created_at = folder.created_at;
        let updated_at = folder.updated_at;
        let headers_count = folder.common_headers.len();
        let params_count = folder.common_search_params.len();

        div()
            .v_flex()
            .gap_4()
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .font_semibold()
                            .child("Folder Information")
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .child(div().font_medium().child("Name:"))
                                    .child(div().child(folder_name))
                            )
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .child(div().font_medium().child("Type:"))
                                    .child(div().child(folder_type_str))
                            )
                            .when(description.is_some(), |container| {
                                container.child(
                                    div()
                                        .h_flex()
                                        .gap_2()
                                        .child(div().font_medium().child("Description:"))
                                        .child(div().child(description.unwrap()))
                                )
                            })
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .child(div().font_medium().child("Created:"))
                                    .child(div().child(Self::format_timestamp(created_at)))
                            )
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .child(div().font_medium().child("Updated:"))
                                    .child(div().child(Self::format_timestamp(updated_at)))
                            )
                    )
            )
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .font_semibold()
                            .child("Statistics")
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .child(div().font_medium().child("Common Headers:"))
                                    .child(div().child(format!("{}", headers_count)))
                            )
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .child(div().font_medium().child("Search Parameters:"))
                                    .child(div().child(format!("{}", params_count)))
                            )
                    )
            )
            .into_any_element()
    }
}

/// Input row for header key-value pairs
struct HeaderInputRow {
    key: Entity<InputState>,
    value: Entity<InputState>,
}

impl HeaderInputRow {
    fn new(header: Option<&Header>, window: &mut Window, cx: &mut Context<CommonHeadersPanel>) -> Self {
        let (key_value, value_value) = if let Some(header) = header {
            (header.key.to_string(), header.value.to_string())
        } else {
            (String::new(), String::new())
        };

        Self {
            key: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(&key_value)
                    .placeholder("Header name")
            }),
            value: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(&value_value)
                    .placeholder("Header value")
            }),
        }
    }

    fn to_header(&self, cx: &App) -> Header {
        Header {
            key: self.key.read(cx).value().to_string().into(),
            value: self.value.read(cx).value().to_string().into(),
        }
    }

    fn is_valid(&self, cx: &App) -> bool {
        let key_value = self.key.read(cx).value();
        let value_value = self.value.read(cx).value();
        let key = key_value.trim();
        let value = value_value.trim();
        !key.is_empty() && !value.is_empty()
    }
}

/// Panel for managing common headers
pub struct CommonHeadersPanel {
    header_rows: Vec<HeaderInputRow>,
    stored_headers: Vec<Header>,
    needs_add_button: bool,
    
    // Error handling and validation
    is_saving: bool,
    save_error: Option<String>,
    validation_errors: std::collections::HashMap<usize, String>,
}

impl EventEmitter<CommonHeadersPanelEvent> for CommonHeadersPanel {}

impl CommonHeadersPanel {
    pub fn new() -> Self {
        Self {
            header_rows: Vec::new(),
            stored_headers: Vec::new(),
            needs_add_button: true,
            is_saving: false,
            save_error: None,
            validation_errors: std::collections::HashMap::new(),
        }
    }

    pub fn set_headers(&mut self, headers: Vec<Header>) {
        self.stored_headers = headers;
        self.header_rows.clear(); // Will be recreated in render
        self.validation_errors.clear();
        self.save_error = None;
    }

    /// Set saving state
    pub fn set_saving(&mut self, saving: bool, cx: &mut Context<Self>) {
        self.is_saving = saving;
        if saving {
            self.save_error = None;
        }
        cx.notify();
    }

    /// Set save error
    pub fn set_save_error(&mut self, error: String, cx: &mut Context<Self>) {
        self.save_error = Some(error);
        self.is_saving = false;
        cx.notify();
    }

    #[allow(unused)]
    fn add_header(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let new_row = HeaderInputRow::new(None, window, cx);
        self.header_rows.push(new_row);
        cx.notify();
    }

    fn remove_header(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.header_rows.len() {
            self.header_rows.remove(index);
            self.emit_headers_changed(cx);
            cx.notify();
        }
    }

    fn emit_headers_changed(&self, cx: &mut Context<Self>) {
        // Validate all headers before emitting
        let mut valid_headers = Vec::new();
        let mut has_errors = false;
        
        for (index, row) in self.header_rows.iter().enumerate() {
            if let Some(_error) = self.get_validation_error(index, cx) {
                has_errors = true;
                // Don't emit if there are validation errors
                continue;
            }
            
            if row.is_valid(cx) {
                valid_headers.push(row.to_header(cx));
            }
        }
        
        // Only emit if no validation errors
        if !has_errors {
            cx.emit(CommonHeadersPanelEvent::HeadersChanged(valid_headers));
        }
    }

    /// Enhanced header name validation
    fn validate_header_name(name: &str) -> Result<(), String> {
        let trimmed = name.trim();
        
        if trimmed.is_empty() {
            return Err("Header name cannot be empty".to_string());
        }
        
        if trimmed.len() > 100 {
            return Err("Header name too long (max 100 characters)".to_string());
        }
        
        // HTTP header name validation (RFC 7230)
        if !trimmed.chars().all(|c| {
            c.is_ascii() && 
            (c.is_alphanumeric() || c == '-' || c == '_') &&
            !c.is_control() && 
            c != ':' && 
            c != ' '
        }) {
            return Err("Invalid header name format (use only letters, numbers, hyphens, and underscores)".to_string());
        }
        
        // Check for reserved headers
        let lower_name = trimmed.to_lowercase();
        if matches!(lower_name.as_str(), 
            "host" | "connection" | "content-length" | "transfer-encoding" | 
            "upgrade" | "proxy-connection" | "te" | "trailer"
        ) {
            return Err(format!("'{}' is a reserved header and cannot be set manually", trimmed));
        }
        
        Ok(())
    }

    /// Enhanced header value validation
    fn validate_header_value(value: &str) -> Result<(), String> {
        let trimmed = value.trim();
        
        if trimmed.is_empty() {
            return Err("Header value cannot be empty".to_string());
        }
        
        if trimmed.len() > 8192 {
            return Err("Header value too long (max 8192 characters)".to_string());
        }
        
        // HTTP header value validation (RFC 7230)
        if !trimmed.chars().all(|c| {
            c.is_ascii() && !c.is_control() || c == '\t'
        }) {
            return Err("Invalid header value format (contains invalid characters)".to_string());
        }
        
        Ok(())
    }

    /// Check for duplicate header names
    fn check_duplicate_headers(&self, cx: &App) -> std::collections::HashMap<usize, String> {
        let mut name_counts: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
        let mut errors = std::collections::HashMap::new();
        
        // Count occurrences of each header name
        for (index, row) in self.header_rows.iter().enumerate() {
            let key_value = row.key.read(cx).value();
            let key = key_value.trim().to_lowercase();
            
            if !key.is_empty() {
                name_counts.entry(key).or_insert_with(Vec::new).push(index);
            }
        }
        
        // Mark duplicates as errors
        for (name, indices) in name_counts {
            if indices.len() > 1 {
                for &index in &indices {
                    errors.insert(index, format!("Duplicate header name '{}'", name));
                }
            }
        }
        
        errors
    }

    fn get_validation_error(&self, index: usize, cx: &App) -> Option<String> {
        if let Some(row) = self.header_rows.get(index) {
            let key_value = row.key.read(cx).value();
            let value_value = row.value.read(cx).value();
            let key = key_value.trim();
            let value = value_value.trim();
            
            // Check header name
            if let Err(error) = Self::validate_header_name(key) {
                return Some(error);
            }
            
            // Check header value
            if let Err(error) = Self::validate_header_value(value) {
                return Some(error);
            }
            
            // Check for duplicates
            let duplicate_errors = self.check_duplicate_headers(cx);
            if let Some(error) = duplicate_errors.get(&index) {
                return Some(error.clone());
            }
        }
        None
    }
}

impl Render for CommonHeadersPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Initialize header rows from stored headers if needed
        if self.header_rows.is_empty() && !self.stored_headers.is_empty() {
            for header in &self.stored_headers {
                let row = HeaderInputRow::new(Some(header), window, cx);
                self.header_rows.push(row);
            }
        }

        // Handle add button request
        if self.needs_add_button {
            let row = HeaderInputRow::new(None, window, cx);
            self.header_rows.push(row);
            self.needs_add_button = false;
        }

        // Ensure we have at least one empty row if no headers exist
        if self.header_rows.is_empty() {
            let row = HeaderInputRow::new(None, window, cx);
            self.header_rows.push(row);
        }

        div()
            .v_flex()
            .gap_4()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_lg()
                            .font_semibold()
                            .child("Common Headers")
                    )
                    .child(
                        Button::new("add_header")
                            .primary()
                            .label("Add Header")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.needs_add_button = true;
                                cx.notify();
                            }))
                    )
            )
            .child(
                div()
                    .v_flex()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .text_color(gpui::rgb(0x666666))
                            .child("Headers defined here will be automatically included in all HTTP requests within this folder.")
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap_2()
                            .children(
                                self.header_rows.iter().enumerate().map(|(index, row)| {
                                    let validation_error = self.get_validation_error(index, cx);
                                    let has_error = validation_error.is_some();
                                    
                                    div()
                                        .v_flex()
                                        .gap_1()
                                        .child(
                                            div()
                                                .h_flex()
                                                .items_center()
                                                .gap_3()
                                                .p_3()
                                                .bg(gpui::rgb(0xffffff))
                                                .border_1()
                                                .when(has_error, |div| {
                                                    div.border_color(gpui::rgb(0xdc3545))
                                                })
                                                .when(!has_error, |div| {
                                                    div.border_color(gpui::rgb(0xe0e0e0))
                                                })
                                                .rounded_md()
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .h_flex()
                                                        .gap_3()
                                                        .child(
                                                            div()
                                                                .w(px(200.0))
                                                                .child(Input::new(&row.key))
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .child(Input::new(&row.value))
                                                        )
                                                )
                                                .child(
                                                    Button::new(format!("delete_header_{}", index))
                                                        .ghost()
                                                        .compact()
                                                        .icon(gpui_component::IconName::Delete)
                                                        .on_click(cx.listener(move |this, _, _, cx| {
                                                            this.remove_header(index, cx);
                                                        }))
                                                )
                                        )
                                        .when(has_error, |container| {
                                            container.child(
                                                div()
                                                    .text_xs()
                                                    .text_color(gpui::rgb(0xdc3545))
                                                    .pl_3()
                                                    .child(validation_error.unwrap())
                                            )
                                        })
                                })
                            )
                    )
            )
            .child(
                div()
                    .h_flex()
                    .justify_end()
                    .gap_2()
                    .when(self.save_error.is_some(), |container| {
                        container.child(
                            div()
                                .text_xs()
                                .text_color(gpui::rgb(0xdc3545))
                                .child(self.save_error.as_ref().unwrap().clone())
                        )
                    })
                    .child(
                        Button::new("save_headers")
                            .primary()
                            .when(self.is_saving, |btn| btn.disabled(true))
                            .label(if self.is_saving { "Saving..." } else { "Save Changes" })
                            .on_click(cx.listener(|this, _, _, cx| {
                                // Validate all headers before saving
                                let mut has_errors = false;
                                for i in 0..this.header_rows.len() {
                                    if this.get_validation_error(i, cx).is_some() {
                                        has_errors = true;
                                        break;
                                    }
                                }
                                
                                if !has_errors {
                                    this.set_saving(true, cx);
                                    this.emit_headers_changed(cx);
                                } else {
                                    this.set_save_error("Please fix validation errors before saving".to_string(), cx);
                                }
                            }))
                    )
            )
            .into_any_element()
    }
}

/// Input row for search parameter key-value pairs
struct ParamInputRow {
    key: Entity<InputState>,
    value: Entity<InputState>,
}

impl ParamInputRow {
    fn new(param: Option<&SearchParameter>, window: &mut Window, cx: &mut Context<SearchParamsPanel>) -> Self {
        let (key_value, value_value) = if let Some(param) = param {
            (param.key.to_string(), param.value.to_string())
        } else {
            (String::new(), String::new())
        };

        Self {
            key: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(&key_value)
                    .placeholder("Parameter name")
            }),
            value: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(&value_value)
                    .placeholder("Parameter value")
            }),
        }
    }

    fn to_param(&self, cx: &App) -> SearchParameter {
        SearchParameter {
            key: self.key.read(cx).value().to_string().into(),
            value: self.value.read(cx).value().to_string().into(),
        }
    }

    fn is_valid(&self, cx: &App) -> bool {
        let key_value = self.key.read(cx).value();
        let value_value = self.value.read(cx).value();
        let key = key_value.trim();
        let value = value_value.trim();
        !key.is_empty() && !value.is_empty()
    }
}

/// Panel for managing search parameters
pub struct SearchParamsPanel {
    param_rows: Vec<ParamInputRow>,
    stored_params: Vec<SearchParameter>,
    needs_add_button: bool,
    
    // Error handling and validation
    is_saving: bool,
    save_error: Option<String>,
    validation_errors: std::collections::HashMap<usize, String>,
}

impl EventEmitter<SearchParamsPanelEvent> for SearchParamsPanel {}

impl SearchParamsPanel {
    pub fn new() -> Self {
        Self {
            param_rows: Vec::new(),
            stored_params: Vec::new(),
            needs_add_button: true,
            is_saving: false,
            save_error: None,
            validation_errors: std::collections::HashMap::new(),
        }
    }

    pub fn set_params(&mut self, params: Vec<SearchParameter>) {
        self.stored_params = params;
        self.param_rows.clear(); // Will be recreated in render
        self.validation_errors.clear();
        self.save_error = None;
    }

    /// Set saving state
    pub fn set_saving(&mut self, saving: bool, cx: &mut Context<Self>) {
        self.is_saving = saving;
        if saving {
            self.save_error = None;
        }
        cx.notify();
    }

    /// Set save error
    pub fn set_save_error(&mut self, error: String, cx: &mut Context<Self>) {
        self.save_error = Some(error);
        self.is_saving = false;
        cx.notify();
    }

    #[allow(unused)]
    fn add_param(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let new_row = ParamInputRow::new(None, window, cx);
        self.param_rows.push(new_row);
        cx.notify();
    }

    fn remove_param(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.param_rows.len() {
            self.param_rows.remove(index);
            self.emit_params_changed(cx);
            cx.notify();
        }
    }

    fn emit_params_changed(&self, cx: &mut Context<Self>) {
        // Validate all parameters before emitting
        let mut valid_params = Vec::new();
        let mut has_errors = false;
        
        for (index, row) in self.param_rows.iter().enumerate() {
            if let Some(_error) = self.get_validation_error(index, cx) {
                has_errors = true;
                // Don't emit if there are validation errors
                continue;
            }
            
            if row.is_valid(cx) {
                valid_params.push(row.to_param(cx));
            }
        }
        
        // Only emit if no validation errors
        if !has_errors {
            cx.emit(SearchParamsPanelEvent::ParamsChanged(valid_params));
        }
    }

    /// Enhanced parameter name validation
    fn validate_param_name(name: &str) -> Result<(), String> {
        let trimmed = name.trim();
        
        if trimmed.is_empty() {
            return Err("Parameter name cannot be empty".to_string());
        }
        
        if trimmed.len() > 100 {
            return Err("Parameter name too long (max 100 characters)".to_string());
        }
        
        // URL parameter name validation
        if !trimmed.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.'
        }) {
            return Err("Invalid parameter name format (use only letters, numbers, hyphens, underscores, and dots)".to_string());
        }
        
        // Check for reserved parameter names that might conflict with common URL patterns
        if matches!(trimmed.to_lowercase().as_str(), 
            "callback" | "jsonp" | "_" | "__proto__" | "constructor"
        ) {
            return Err(format!("'{}' is a reserved parameter name", trimmed));
        }
        
        Ok(())
    }

    /// Enhanced parameter value validation
    fn validate_param_value(value: &str) -> Result<(), String> {
        let trimmed = value.trim();
        
        if trimmed.is_empty() {
            return Err("Parameter value cannot be empty".to_string());
        }
        
        if trimmed.len() > 2048 {
            return Err("Parameter value too long (max 2048 characters)".to_string());
        }
        
        // Check for potentially problematic characters
        if trimmed.contains('\n') || trimmed.contains('\r') {
            return Err("Parameter value cannot contain newline characters".to_string());
        }
        
        Ok(())
    }

    /// Check for duplicate parameter names
    fn check_duplicate_params(&self, cx: &App) -> std::collections::HashMap<usize, String> {
        let mut name_counts: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
        let mut errors = std::collections::HashMap::new();
        
        // Count occurrences of each parameter name
        for (index, row) in self.param_rows.iter().enumerate() {
            let key_value = row.key.read(cx).value();
            let key = key_value.trim().to_lowercase();
            
            if !key.is_empty() {
                name_counts.entry(key).or_insert_with(Vec::new).push(index);
            }
        }
        
        // Mark duplicates as errors
        for (name, indices) in name_counts {
            if indices.len() > 1 {
                for &index in &indices {
                    errors.insert(index, format!("Duplicate parameter name '{}'", name));
                }
            }
        }
        
        errors
    }

    fn get_validation_error(&self, index: usize, cx: &App) -> Option<String> {
        if let Some(row) = self.param_rows.get(index) {
            let key_value = row.key.read(cx).value();
            let value_value = row.value.read(cx).value();
            let key = key_value.trim();
            let value = value_value.trim();
            
            // Check parameter name
            if let Err(error) = Self::validate_param_name(key) {
                return Some(error);
            }
            
            // Check parameter value
            if let Err(error) = Self::validate_param_value(value) {
                return Some(error);
            }
            
            // Check for duplicates
            let duplicate_errors = self.check_duplicate_params(cx);
            if let Some(error) = duplicate_errors.get(&index) {
                return Some(error.clone());
            }
        }
        None
    }
}

impl Render for SearchParamsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Initialize param rows from stored params if needed
        if self.param_rows.is_empty() && !self.stored_params.is_empty() {
            for param in &self.stored_params {
                let row = ParamInputRow::new(Some(param), window, cx);
                self.param_rows.push(row);
            }
        }

        // Handle add button request
        if self.needs_add_button {
            let row = ParamInputRow::new(None, window, cx);
            self.param_rows.push(row);
            self.needs_add_button = false;
        }

        // Ensure we have at least one empty row if no params exist
        if self.param_rows.is_empty() {
            let row = ParamInputRow::new(None, window, cx);
            self.param_rows.push(row);
        }

        div()
            .v_flex()
            .gap_4()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_lg()
                            .font_semibold()
                            .child("Search Parameters")
                    )
                    .child(
                        Button::new("add_param")
                            .primary()
                            .label("Add Parameter")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.needs_add_button = true;
                                cx.notify();
                            }))
                    )
            )
            .child(
                div()
                    .v_flex()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .text_color(gpui::rgb(0x666666))
                            .child("Parameters defined here will be automatically included as query parameters in all HTTP requests within this folder.")
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap_2()
                            .children(
                                self.param_rows.iter().enumerate().map(|(index, row)| {
                                    let validation_error = self.get_validation_error(index, cx);
                                    let has_error = validation_error.is_some();
                                    
                                    div()
                                        .v_flex()
                                        .gap_1()
                                        .child(
                                            div()
                                                .h_flex()
                                                .items_center()
                                                .gap_3()
                                                .p_3()
                                                .bg(gpui::rgb(0xffffff))
                                                .border_1()
                                                .when(has_error, |div| {
                                                    div.border_color(gpui::rgb(0xdc3545))
                                                })
                                                .when(!has_error, |div| {
                                                    div.border_color(gpui::rgb(0xe0e0e0))
                                                })
                                                .rounded_md()
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .h_flex()
                                                        .gap_3()
                                                        .child(
                                                            div()
                                                                .w(px(200.0))
                                                                .child(Input::new(&row.key))
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .child(Input::new(&row.value))
                                                        )
                                                )
                                                .child(
                                                    Button::new(format!("delete_param_{}", index))
                                                        .ghost()
                                                        .compact()
                                                        .icon(gpui_component::IconName::Delete)
                                                        .on_click(cx.listener(move |this, _, _, cx| {
                                                            this.remove_param(index, cx);
                                                        }))
                                                )
                                        )
                                        .when(has_error, |container| {
                                            container.child(
                                                div()
                                                    .text_xs()
                                                    .text_color(gpui::rgb(0xdc3545))
                                                    .pl_3()
                                                    .child(validation_error.unwrap())
                                            )
                                        })
                                })
                            )
                    )
            )
            .child(
                div()
                    .h_flex()
                    .justify_end()
                    .gap_2()
                    .when(self.save_error.is_some(), |container| {
                        container.child(
                            div()
                                .text_xs()
                                .text_color(gpui::rgb(0xdc3545))
                                .child(self.save_error.as_ref().unwrap().clone())
                        )
                    })
                    .child(
                        Button::new("save_params")
                            .primary()
                            .when(self.is_saving, |btn| btn.disabled(true))
                            .label(if self.is_saving { "Saving..." } else { "Save Changes" })
                            .on_click(cx.listener(|this, _, _, cx| {
                                // Validate all parameters before saving
                                let mut has_errors = false;
                                for i in 0..this.param_rows.len() {
                                    if this.get_validation_error(i, cx).is_some() {
                                        has_errors = true;
                                        break;
                                    }
                                }
                                
                                if !has_errors {
                                    this.set_saving(true, cx);
                                    this.emit_params_changed(cx);
                                } else {
                                    this.set_save_error("Please fix validation errors before saving".to_string(), cx);
                                }
                            }))
                    )
            )
            .into_any_element()
    }
}

