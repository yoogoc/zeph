use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::checkbox::Checkbox;
use gpui_component::input::{Input, InputState};
use gpui_component::{Disableable, IconName, StyledExt};
use gpui_tokio::Tokio;
use std::time::Duration;
use uuid::Uuid;
use workspace::parse_openapi;

/// Configuration for the simple import dialog
#[derive(Debug, Clone)]
pub struct SimpleImportConfig {
    /// OpenAPI content (JSON/YAML) or downloaded content from URL
    pub content: String,
    /// Target folder ID (None means use default tag behavior)
    pub target_folder: Option<Uuid>,
    /// Whether to create subfolders based on tags
    pub create_subfolders: bool,
}

/// Error types for the import dialog
/// Implements Requirements 5.1, 5.3, 5.4 for error categorization
#[derive(Debug, Clone)]
pub enum ImportErrorType {
    /// Parse error - invalid JSON/YAML format
    ParseError(String),
    /// Validation error - empty content, content too large, etc.
    ValidationError(String),
    /// Network error - URL download failed
    NetworkError(String),
    /// Import error - target folder not found, database error, etc.
    ImportError(String),
}

impl ImportErrorType {
    /// Get the error title based on error type
    pub fn title(&self) -> &'static str {
        match self {
            ImportErrorType::ParseError(_) => "解析错误",
            ImportErrorType::ValidationError(_) => "验证错误",
            ImportErrorType::NetworkError(_) => "网络错误",
            ImportErrorType::ImportError(_) => "导入错误",
        }
    }

    /// Get the error message
    pub fn message(&self) -> &str {
        match self {
            ImportErrorType::ParseError(msg) => msg,
            ImportErrorType::ValidationError(msg) => msg,
            ImportErrorType::NetworkError(msg) => msg,
            ImportErrorType::ImportError(msg) => msg,
        }
    }
}

/// Events emitted by the SimpleImportDialog
/// Implements Requirements 3.1, 3.2, 3.3 - only Import and Cancel events
#[derive(Debug, Clone)]
pub enum SimpleImportDialogEvent {
    Import(SimpleImportConfig),
    Cancel,
}

/// Simplified import dialog component
/// Implements Requirements 2.1, 2.2, 2.3, 2.5 for dialog UI
pub struct SimpleImportDialog {
    // Input
    content_input: Entity<InputState>,

    // Target folder (preset, read-only display)
    target_folder_id: Option<Uuid>,
    target_folder_name: Option<SharedString>,

    // Options
    create_subfolders: bool,

    // State
    is_loading: bool,
    /// Error information with type categorization
    /// Implements Requirements 5.1, 5.3, 5.4 for error display
    error: Option<ImportErrorType>,
    service_count: Option<usize>,

    // Downloaded content from URL (if applicable)
    /// Implements Requirement 6.3 - stores downloaded content for URL imports
    downloaded_content: Option<String>,
}

impl EventEmitter<SimpleImportDialogEvent> for SimpleImportDialog {}

impl SimpleImportDialog {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let content_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("粘贴 OpenAPI JSON/YAML 内容或 URL...")
                .multi_line(true)
        });

        Self {
            content_input,
            target_folder_id: None,
            target_folder_name: None,
            create_subfolders: true,
            is_loading: false,
            error: None,
            service_count: None,
            downloaded_content: None,
        }
    }

    /// Set the target folder for import
    /// Implements Requirement 2.2 - display preset target folder name
    pub fn set_target_folder(
        &mut self,
        folder_id: Option<Uuid>,
        folder_name: Option<SharedString>,
    ) {
        self.target_folder_id = folder_id;
        self.target_folder_name = folder_name;
    }

    /// Set loading state
    pub fn set_loading(&mut self, loading: bool) {
        self.is_loading = loading;
    }

    /// Set error message
    /// Implements Requirements 5.1, 5.3, 5.4 for error display
    pub fn set_error(&mut self, error: Option<ImportErrorType>) {
        self.error = error;
    }

    /// Set error from a simple string (convenience method for backward compatibility)
    pub fn set_error_message(&mut self, message: Option<String>) {
        self.error = message.map(ImportErrorType::ImportError);
    }

    /// Clear error message
    pub fn clear_error(&mut self) {
        self.error = None;
    }

    /// Set downloaded content from URL
    /// Implements Requirement 6.3 - use downloaded content for import
    pub fn set_downloaded_content(&mut self, content: Option<String>) {
        self.downloaded_content = content;
    }

    /// Get the content to use for import (downloaded content or input content)
    /// Implements Requirement 6.3 - use downloaded content when available
    pub fn get_import_content(&self, cx: &Context<Self>) -> String {
        if let Some(downloaded) = &self.downloaded_content {
            downloaded.clone()
        } else {
            self.content_input.read(cx).value().to_string()
        }
    }

    /// Check if the current input is a URL
    pub fn is_url_input(&self, cx: &Context<Self>) -> bool {
        let content = self.content_input.read(cx).value().to_string();
        is_url(content.trim())
    }

    /// Validate OpenAPI content format
    /// Implements Requirement 5.1 for content validation
    pub fn validate_content(content: &str) -> Result<(), ImportErrorType> {
        let trimmed = content.trim();

        // Check for empty content
        if trimmed.is_empty() {
            return Err(ImportErrorType::ValidationError(
                "OpenAPI 内容不能为空".to_string(),
            ));
        }

        // Check content size limit (10MB)
        if content.len() > 10_000_000 {
            return Err(ImportErrorType::ValidationError(
                "内容过大，请使用较小的规范文件".to_string(),
            ));
        }

        // Try to parse the content
        match parse_openapi(content) {
            Ok(spec) => {
                // Check if spec has any paths
                if spec.paths.is_empty() {
                    return Err(ImportErrorType::ValidationError(
                        "OpenAPI 规范中没有找到任何路径定义".to_string(),
                    ));
                }
                Ok(())
            }
            Err(e) => Err(ImportErrorType::ParseError(e.to_string())),
        }
    }

    /// Handle URL download completion
    /// Implements Requirements 6.2, 6.3 for URL download handling
    pub fn handle_url_download_result(
        &mut self,
        result: Result<String, UrlFetchError>,
        cx: &mut Context<Self>,
    ) {
        self.is_loading = false;

        match result {
            Ok(content) => {
                // Validate the downloaded content
                if let Err(e) = Self::validate_content(&content) {
                    self.error = Some(e);
                    self.downloaded_content = None;
                    self.service_count = None;
                    cx.notify();
                    return;
                }

                self.downloaded_content = Some(content.clone());
                self.error = None;

                // Try to parse and count services from downloaded content
                match parse_openapi(&content) {
                    Ok(spec) => {
                        let mut count = 0;
                        for path_item in spec.paths.values() {
                            if path_item.get.is_some() {
                                count += 1;
                            }
                            if path_item.post.is_some() {
                                count += 1;
                            }
                            if path_item.put.is_some() {
                                count += 1;
                            }
                            if path_item.delete.is_some() {
                                count += 1;
                            }
                            if path_item.patch.is_some() {
                                count += 1;
                            }
                        }
                        self.service_count = Some(count);
                    }
                    Err(e) => {
                        self.error = Some(ImportErrorType::ParseError(e.to_string()));
                        self.service_count = None;
                    }
                }
            }
            Err(e) => {
                self.error = Some(ImportErrorType::NetworkError(e.to_string()));
                self.downloaded_content = None;
                self.service_count = None;
            }
        }

        cx.notify();
    }

    /// Update service count preview based on content
    /// Implements Requirement 2.4 - show service count preview
    fn update_preview(&mut self, cx: &Context<Self>) {
        let content = self.content_input.read(cx).value().to_string();
        let trimmed = content.trim();

        if trimmed.is_empty() {
            self.service_count = None;
            self.error = None;
            return;
        }

        // Skip preview for URLs - will be handled after download
        if is_url(trimmed) {
            self.service_count = None;
            self.error = None;
            return;
        }

        // Try to parse and count services
        match parse_openapi(&content) {
            Ok(spec) => {
                let mut count = 0;
                for path_item in spec.paths.values() {
                    if path_item.get.is_some() {
                        count += 1;
                    }
                    if path_item.post.is_some() {
                        count += 1;
                    }
                    if path_item.put.is_some() {
                        count += 1;
                    }
                    if path_item.delete.is_some() {
                        count += 1;
                    }
                    if path_item.patch.is_some() {
                        count += 1;
                    }
                }
                self.service_count = Some(count);
                self.error = None;
            }
            Err(e) => {
                self.service_count = None;
                self.error = Some(ImportErrorType::ParseError(e.to_string()));
            }
        }
    }

    /// Validate input before import
    /// Implements Requirement 5.1 for input validation
    fn validate_input(&self, cx: &Context<Self>) -> Result<(), ImportErrorType> {
        let content = self.content_input.read(cx).value().to_string();

        // Check for empty content
        if content.trim().is_empty() {
            return Err(ImportErrorType::ValidationError(
                "请输入 OpenAPI 内容或 URL".to_string(),
            ));
        }

        // Check content size limit (10MB)
        if content.len() > 10_000_000 {
            return Err(ImportErrorType::ValidationError(
                "内容过大，请使用较小的规范文件".to_string(),
            ));
        }

        // Skip format validation for URLs - will be validated after download
        let trimmed = content.trim();
        if is_url(trimmed) {
            return Ok(());
        }

        // Validate OpenAPI content format (JSON/YAML)
        match workspace::parse_openapi(&content) {
            Ok(spec) => {
                // Check if spec has any paths
                if spec.paths.is_empty() {
                    return Err(ImportErrorType::ValidationError(
                        "OpenAPI 规范中没有找到任何路径定义".to_string(),
                    ));
                }
                Ok(())
            }
            Err(e) => Err(ImportErrorType::ParseError(e.to_string())),
        }
    }

    /// Render the content input area
    fn render_content_input(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let input_content = self.content_input.read(cx).value().to_string();
        let is_url_input = is_url(input_content.trim());

        div()
            .v_flex()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .text_color(rgb(0x495057))
                    .child("OpenAPI 内容"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x6c757d))
                    .child("支持 JSON/YAML 格式或 URL"),
            )
            .child(Input::new(&self.content_input).w_full().h(px(150.0)))
            .child(self.render_preview_info(cx))
            .when(is_url_input && self.is_loading, |el| {
                el.child(self.render_loading_indicator())
            })
    }

    /// Render loading indicator for URL download
    /// Implements Requirement 6.5 - show loading state while downloading
    fn render_loading_indicator(&self) -> impl IntoElement {
        div().h_flex().items_center().gap_2().child(
            // Simple loading spinner using animated dots
            div()
                .text_sm()
                .text_color(rgb(0x007bff))
                .child("正在下载..."),
        )
    }

    /// Render preview info (service count)
    /// Implements Requirement 2.4
    fn render_preview_info(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let input_content = self.content_input.read(cx).value().to_string();
        let is_url_input = is_url(input_content.trim());

        if let Some(count) = self.service_count {
            div()
                .h_flex()
                .items_center()
                .gap_2()
                .child(
                    gpui_component::Icon::new(IconName::Check)
                        .size_4()
                        .text_color(rgb(0x28a745)),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x28a745))
                        .child(format!("将导入 {} 个服务", count)),
                )
        } else if is_url_input && !self.is_loading && self.error.is_none() {
            // Show URL detected message
            div()
                .h_flex()
                .items_center()
                .gap_2()
                .child(
                    gpui_component::Icon::new(IconName::Globe)
                        .size_4()
                        .text_color(rgb(0x007bff)),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x007bff))
                        .child("检测到 URL，点击导入将下载内容"),
                )
        } else {
            div()
        }
    }

    /// Render target folder display
    /// Implements Requirement 2.2
    fn render_target_folder(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .text_color(rgb(0x495057))
                    .child("目标文件夹"),
            )
            .child(
                div()
                    .p_3()
                    .bg(rgb(0xf8f9fa))
                    .border_1()
                    .border_color(rgb(0xe9ecef))
                    .rounded(px(4.0))
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        gpui_component::Icon::new(IconName::Folder)
                            .size_4()
                            .text_color(rgb(0x6c757d)),
                    )
                    .child(
                        div().text_sm().text_color(rgb(0x495057)).child(
                            self.target_folder_name
                                .clone()
                                .unwrap_or_else(|| "工作区根目录（按 tag 创建文件夹）".into()),
                        ),
                    ),
            )
    }

    /// Render import options
    /// Implements Requirement 2.3
    fn render_options(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .text_color(rgb(0x495057))
                    .child("导入选项"),
            )
            .child(
                Checkbox::new("create_sample_services")
                    .label("根据 tags 创建子文件夹")
                    .checked(self.create_subfolders)
                    .on_click(cx.listener(|this, checked, _, cx| {
                        this.create_subfolders = *checked;
                        cx.notify();
                    })),
            )
    }

    /// Render error display area
    /// Implements Requirements 5.1, 5.3, 5.4 for error display with prominent styling
    fn render_error(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(error) = &self.error {
            // Error container with prominent red styling
            // Implements Requirement 5.4 - use prominent style for error display
            div()
                .v_flex()
                .gap_2()
                .p_3()
                .bg(rgb(0xf8d7da)) // Light red background
                .border_1()
                .border_color(rgb(0xf5c6cb)) // Red border
                .rounded(px(6.0))
                .child(
                    // Error header with icon and title
                    div()
                        .h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            // Error icon - using a styled div as warning indicator
                            div()
                                .w(px(20.0))
                                .h(px(20.0))
                                .rounded_full()
                                .bg(rgb(0xdc3545)) // Red background
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    gpui_component::Icon::new(IconName::Close)
                                        .size_3()
                                        .text_color(rgb(0xffffff)), // White icon
                                ),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .text_color(rgb(0x721c24)) // Dark red text
                                .child(error.title()),
                        ),
                )
                .child(
                    // Error message with proper formatting
                    div()
                        .pl(px(28.0)) // Align with title text
                        .text_sm()
                        .text_color(rgb(0x721c24))
                        .child(error.message().to_string()),
                )
        } else {
            div()
        }
    }

    /// Render action buttons
    /// Implements Requirement 2.5
    fn render_actions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h_flex()
            .justify_end()
            .gap_2()
            .child(
                Button::new("cancel_import")
                    .ghost()
                    .child("取消")
                    .on_click(cx.listener(|_this, _, _, cx| {
                        cx.emit(SimpleImportDialogEvent::Cancel);
                    })),
            )
            .child(
                Button::new("confirm_import")
                    .primary()
                    .when(self.is_loading, |btn| btn.disabled(true))
                    .child(if self.is_loading { "导入中..." } else { "导入" })
                    .on_click(cx.listener(|this, _, window, cx| {
                        // Validate input first
                        if let Err(error) = this.validate_input(cx) {
                            this.error = Some(error);
                            cx.notify();
                            return;
                        }

                        let input_content = this.content_input.read(cx).value().to_string();
                        let trimmed = input_content.trim();

                        // Check if input is a URL
                        if is_url(trimmed) {
                            // If we already have downloaded content, use it
                            if let Some(downloaded) = &this.downloaded_content {
                                cx.emit(SimpleImportDialogEvent::Import(SimpleImportConfig {
                                    content: downloaded.clone(),
                                    target_folder: this.target_folder_id,
                                    create_subfolders: this.create_subfolders,
                                }));
                            } else {
                                // Start URL download
                                // Implements Requirements 6.2, 6.5 for URL download with loading state
                                this.is_loading = true;
                                this.error = None;
                                cx.notify();

                                let url = trimmed.to_string();
                                let target_folder = this.target_folder_id;
                                let create_subfolders = this.create_subfolders;

                                // Spawn async task to download URL content
                                let task = Tokio::spawn(cx, async move {
                                    fetch_openapi_from_url(&url).await
                                });

                                cx.spawn_in(
                                    window,
                                    async move |this: WeakEntity<Self>, cx: &mut AsyncWindowContext| {
                                        let result = task.await.unwrap();

                                        this.update_in(cx, |this, _, cx| {
                                            this.is_loading = false;

                                            match result {
                                                Ok(content) => {
                                                    // Emit import event with downloaded content
                                                    cx.emit(SimpleImportDialogEvent::Import(SimpleImportConfig {
                                                        content,
                                                        target_folder,
                                                        create_subfolders,
                                                    }));
                                                }
                                                Err(e) => {
                                                    this.error = Some(ImportErrorType::NetworkError(e.to_string()));
                                                    cx.notify();
                                                }
                                            }
                                        }).ok();
                                    },
                                ).detach();
                            }
                        } else {
                            // Direct content import
                            cx.emit(SimpleImportDialogEvent::Import(SimpleImportConfig {
                                content: input_content,
                                target_folder: this.target_folder_id,
                                create_subfolders: this.create_subfolders,
                            }));
                        }
                    })),
            )
    }
}

impl Render for SimpleImportDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Update preview when content changes
        self.update_preview(cx);

        div()
            .w(px(550.0))
            .max_h(px(700.0))
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(0xdee2e6))
            .rounded_lg()
            .shadow_lg()
            .v_flex()
            .child(
                // Header
                div().p_4().border_b_1().border_color(rgb(0xe9ecef)).child(
                    div()
                        .text_lg()
                        .font_semibold()
                        .text_color(rgb(0x212529))
                        .child("导入 OpenAPI"),
                ),
            )
            .child(
                // Content
                div()
                    .flex_1()
                    .overflow_y_hidden()
                    .p_4()
                    .v_flex()
                    .gap_4()
                    .child(self.render_content_input(cx))
                    .child(self.render_target_folder(cx))
                    .child(self.render_options(cx))
                    .child(self.render_error(cx)),
            )
            .child(
                // Footer
                div()
                    .p_4()
                    .border_t_1()
                    .border_color(rgb(0xe9ecef))
                    .child(self.render_actions(cx)),
            )
    }
}

/// Check if input is a URL
/// Implements Requirement 6.1
pub fn is_url(input: &str) -> bool {
    let trimmed = input.trim();
    trimmed.starts_with("http://") || trimmed.starts_with("https://")
}

/// Error type for URL fetch operations
/// Implements Requirement 6.4 for network error handling
#[derive(Debug, Clone)]
pub enum UrlFetchError {
    /// Connection failed
    ConnectionFailed(String),
    /// Request timeout
    Timeout,
    /// Non-200 response
    HttpError(u16, String),
    /// Invalid URL
    InvalidUrl(String),
    /// Other errors
    Other(String),
}

impl std::fmt::Display for UrlFetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UrlFetchError::ConnectionFailed(msg) => write!(f, "无法连接到指定 URL: {}", msg),
            UrlFetchError::Timeout => write!(f, "下载超时，请检查网络连接"),
            UrlFetchError::HttpError(status, msg) => {
                write!(f, "服务器返回错误: {} - {}", status, msg)
            }
            UrlFetchError::InvalidUrl(msg) => write!(f, "无效的 URL: {}", msg),
            UrlFetchError::Other(msg) => write!(f, "下载失败: {}", msg),
        }
    }
}

/// Fetch OpenAPI content from a URL
/// Implements Requirements 6.2, 6.3, 6.4 for URL download functionality
pub async fn fetch_openapi_from_url(url: &str) -> Result<String, UrlFetchError> {
    // Validate URL format
    let url = url.trim();
    if !is_url(url) {
        return Err(UrlFetchError::InvalidUrl(
            "URL 必须以 http:// 或 https:// 开头".to_string(),
        ));
    }

    // Create HTTP client with timeout
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| UrlFetchError::Other(e.to_string()))?;

    // Send GET request
    let response = client
        .get(url)
        .header(
            "Accept",
            "application/json, application/yaml, text/yaml, */*",
        )
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                UrlFetchError::Timeout
            } else if e.is_connect() {
                UrlFetchError::ConnectionFailed(e.to_string())
            } else {
                UrlFetchError::Other(e.to_string())
            }
        })?;

    // Check response status
    let status = response.status();
    if !status.is_success() {
        return Err(UrlFetchError::HttpError(
            status.as_u16(),
            status
                .canonical_reason()
                .unwrap_or("Unknown error")
                .to_string(),
        ));
    }

    // Get response body
    let content = response
        .text()
        .await
        .map_err(|e| UrlFetchError::Other(e.to_string()))?;

    Ok(content)
}
