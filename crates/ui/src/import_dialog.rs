use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{IconName, StyledExt, Disableable};
use models::{Folder, FolderType};
use uuid::Uuid;
use workspace::{ImportOptions, ImportPreview, SchemaFolderStrategy, NameConflict, ConflictResolution, ServicePreview, SchemaPreview};
use std::collections::HashMap;

use crate::FolderSelector;

#[derive(Debug, Clone)]
pub enum ImportDialogEvent {
    Import(ImportConfig),
    Cancel,
    RequestPreview(ImportConfig),
    ResolveConflicts(ImportConfig, HashMap<String, ConflictResolution>),
}

#[derive(Debug, Clone)]
pub struct ImportConfig {
    pub content: String,
    pub target_folder: Option<Uuid>,
    pub create_subfolders: bool,
    pub schema_folder_strategy: SchemaFolderStrategy,
}

/// Enhanced error information for better user feedback
/// Implements Requirements 6.1, 6.2 for error handling
#[derive(Debug, Clone)]
pub enum ImportError {
    ValidationError(String),
    ConflictError(Vec<NameConflict>),
    ParseError(String),
    NetworkError(String),
    GeneralError(String),
}

impl ImportError {
    pub fn message(&self) -> String {
        match self {
            ImportError::ValidationError(msg) => format!("验证错误: {}", msg),
            ImportError::ConflictError(conflicts) => {
                let conflict_msgs: Vec<String> = conflicts
                    .iter()
                    .map(|c| format!("• {}", c.name))
                    .collect();
                format!("名称冲突:\n{}", conflict_msgs.join("\n"))
            }
            ImportError::ParseError(msg) => format!("解析错误: {}", msg),
            ImportError::NetworkError(msg) => format!("网络错误: {}", msg),
            ImportError::GeneralError(msg) => msg.clone(),
        }
    }
    
    pub fn is_conflict_error(&self) -> bool {
        matches!(self, ImportError::ConflictError(_))
    }
    
    pub fn get_conflicts(&self) -> Option<&Vec<NameConflict>> {
        match self {
            ImportError::ConflictError(conflicts) => Some(conflicts),
            _ => None,
        }
    }
}

pub struct ImportDialog {
    // OpenAPI content input
    content_input: Entity<InputState>,
    
    // Folder selection
    folder_selector: Entity<FolderSelector>,
    selected_folder: Option<Uuid>,
    available_folders: Vec<Folder>,
    
    // Import options
    create_subfolders: bool,
    schema_folder_strategy: SchemaFolderStrategy,
    
    // Preview data
    preview_data: Option<ImportPreview>,
    
    // Enhanced error handling
    current_error: Option<ImportError>,
    pending_conflicts: Option<Vec<NameConflict>>,
    conflict_resolutions: HashMap<String, ConflictResolution>,
    
    // State
    is_loading: bool,
    show_preview: bool,
    show_conflict_resolution: bool,
}

impl EventEmitter<ImportDialogEvent> for ImportDialog {}

impl ImportDialog {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let content_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Paste OpenAPI JSON/YAML content or URL...")
                .multi_line(true)
        });
        
        let folder_selector = cx.new(|_cx| FolderSelector::new(FolderType::Service));
        
        Self {
            content_input,
            folder_selector,
            selected_folder: None,
            available_folders: Vec::new(),
            create_subfolders: true,
            schema_folder_strategy: SchemaFolderStrategy::SeparateFolder,
            preview_data: None,
            current_error: None,
            pending_conflicts: None,
            conflict_resolutions: HashMap::new(),
            is_loading: false,
            show_preview: false,
            show_conflict_resolution: false,
        }
    }
    
    pub fn set_folders(&mut self, folders: Vec<Folder>, cx: &mut Context<Self>) {
        self.available_folders = folders.clone();
        self.folder_selector.update(cx, |selector, _cx| {
            selector.set_folders(folders);
        });
    }
    
    pub fn set_preview_data(&mut self, preview: Option<ImportPreview>) {
        self.show_preview = preview.is_some();
        self.preview_data = preview;
    }
    
    /// Enhanced error handling with support for different error types
    /// Implements Requirements 6.1, 6.2 for error message display
    pub fn set_error(&mut self, error: Option<ImportError>) {
        self.current_error = error.clone();
        
        // Handle conflict errors specially
        if let Some(ImportError::ConflictError(conflicts)) = error {
            self.pending_conflicts = Some(conflicts);
            self.show_conflict_resolution = true;
            self.show_preview = false;
        } else {
            self.pending_conflicts = None;
            self.show_conflict_resolution = false;
        }
    }
    
    pub fn set_loading(&mut self, loading: bool) {
        self.is_loading = loading;
    }
    
    /// Clear all errors and reset dialog state
    pub fn clear_error(&mut self) {
        self.current_error = None;
        self.pending_conflicts = None;
        self.conflict_resolutions.clear();
        self.show_conflict_resolution = false;
    }
    
    #[allow(unused)]
    fn get_import_options(&self) -> ImportOptions {
        ImportOptions {
            target_folder: self.selected_folder,
            create_subfolders: self.create_subfolders,
            schema_folder_strategy: self.schema_folder_strategy.clone(),
        }
    }
    
    /// Validate input before processing
    /// Implements Requirements 6.1, 6.2 for input validation
    fn validate_input(&self, cx: &Context<Self>) -> Result<(), ImportError> {
        let content = self.content_input.read(cx).value().to_string();
        
        if content.trim().is_empty() {
            return Err(ImportError::ValidationError("请输入 OpenAPI 内容".to_string()));
        }
        
        if content.len() > 10_000_000 {
            return Err(ImportError::ValidationError("OpenAPI 内容过大，请使用较小的规范文件".to_string()));
        }
        
        // Additional validation can be added here
        Ok(())
    }
    
    fn handle_folder_selection(&mut self, folder_id: Option<Uuid>, cx: &mut Context<Self>) {
        self.selected_folder = folder_id;
        self.clear_error(); // Clear errors when selection changes
        self.update_preview(cx);
    }
    
    fn update_preview(&mut self, cx: &mut Context<Self>) {
        // Validate input first
        if let Err(error) = self.validate_input(cx) {
            self.set_error(Some(error));
            return;
        }
        
        let content = self.content_input.read(cx).value().to_string();
        if content.trim().is_empty() {
            self.set_preview_data(None);
            return;
        }
        
        // Request preview generation from parent
        cx.emit(ImportDialogEvent::RequestPreview(ImportConfig {
            content: content.clone(),
            target_folder: self.selected_folder,
            create_subfolders: self.create_subfolders,
            schema_folder_strategy: self.schema_folder_strategy.clone(),
        }));
    }
    
    /// Handle conflict resolution selection
    /// Implements Requirement 6.4 for conflict resolution
    fn handle_conflict_resolution(&mut self, conflict_name: String, resolution: ConflictResolution) {
        self.conflict_resolutions.insert(conflict_name, resolution);
    }
    
    /// Render enhanced error display with different error types
    /// Implements Requirements 6.1, 6.2 for error message display
    fn render_error_display(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(error) = &self.current_error {
            div()
                .v_flex()
                .gap_2()
                .child(
                    div()
                        .p_3()
                        .bg(rgb(0xfff3cd))
                        .border_1()
                        .border_color(rgb(0xffeaa7))
                        .rounded(px(4.0))
                        .child(
                            div()
                                .h_flex()
                                .items_start()
                                .gap_2()
                                .child(
                                    gpui_component::Icon::new(IconName::Settings)
                                        .size_4()
                                        .text_color(rgb(0x856404))
                                        .mt(px(2.0))
                                )
                                .child(
                                    div()
                                        .v_flex()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_medium()
                                                .text_color(rgb(0x856404))
                                                .child(match error {
                                                    ImportError::ValidationError(_) => "输入验证错误",
                                                    ImportError::ConflictError(_) => "名称冲突",
                                                    ImportError::ParseError(_) => "解析错误",
                                                    ImportError::NetworkError(_) => "网络错误",
                                                    ImportError::GeneralError(_) => "导入错误",
                                                })
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(rgb(0x856404))
                                                .child(error.message())
                                        )
                                )
                        )
                )
        } else {
            div() // Empty div when no error
        }
    }
    
    /// Render conflict resolution interface
    /// Implements Requirement 6.4 for conflict resolution UI
    fn render_conflict_resolution(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(conflicts) = &self.pending_conflicts {
            div()
                .v_flex()
                .gap_3()
                .child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .text_color(rgb(0x495057))
                        .child("解决名称冲突")
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x6c757d))
                        .child("以下项目与现有内容存在名称冲突，请选择处理方式：")
                )
                .child(
                    div()
                        .v_flex()
                        .gap_3()
                        .children(
                            conflicts.iter().map(|conflict| {
                                self.render_conflict_item(conflict, cx)
                            })
                        )
                )
        } else {
            div() // Empty div when no conflicts
        }
    }
    
    /// Render individual conflict resolution item
    fn render_conflict_item(&self, conflict: &NameConflict, cx: &mut Context<Self>) -> impl IntoElement {
        let current_resolution = self.conflict_resolutions.get(&conflict.name);
        
        div()
            .p_3()
            .border_1()
            .border_color(rgb(0xe9ecef))
            .rounded(px(4.0))
            .v_flex()
            .gap_2()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        gpui_component::Icon::new(match conflict.conflict_type {
                            workspace::ConflictType::Service => IconName::Globe,
                            workspace::ConflictType::Schema => IconName::File,
                        })
                        .size_4()
                        .text_color(rgb(0x6f42c1))
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .child(conflict.name.clone())
                    )
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x6c757d))
                    .child(format!("建议名称: {}", conflict.suggested_name))
            )
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .child(self.render_resolution_option(conflict, ConflictResolution::Skip, "跳过导入", current_resolution, cx))
                    .child(self.render_resolution_option(conflict, ConflictResolution::Rename(conflict.suggested_name.clone()), "使用建议名称", current_resolution, cx))
                    .child(self.render_resolution_option(conflict, ConflictResolution::Replace, "替换现有项目", current_resolution, cx))
            )
    }
    
    /// Render conflict resolution option button
    fn render_resolution_option(
        &self,
        conflict: &NameConflict,
        resolution: ConflictResolution,
        label: &str,
        current_resolution: Option<&ConflictResolution>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_selected = current_resolution
            .map(|r| std::mem::discriminant(r) == std::mem::discriminant(&resolution))
            .unwrap_or(false);
        
        let conflict_name = conflict.name.clone();
        let resolution_clone = resolution.clone();
        
        Button::new(format!("resolution_{}_{}", conflict.name, label.replace(" ", "_")))
            .ghost()
            .w_full()
            .justify_start()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .w(px(16.0))
                            .h(px(16.0))
                            .border_1()
                            .border_color(rgb(0xdee2e6))
                            .rounded_full()
                            .h_flex()
                            .items_center()
                            .justify_center()
                            .when(is_selected, |el| {
                                el.border_color(rgb(0x007bff))
                                    .child(
                                        div()
                                            .w(px(8.0))
                                            .h(px(8.0))
                                            .bg(rgb(0x007bff))
                                            .rounded_full()
                                    )
                            })
                    )
                    .child(
                        div()
                            .text_sm()
                            .child(label.to_string())
                    )
            )
            .on_click(cx.listener(move |this, _, _, _cx| {
                this.handle_conflict_resolution(conflict_name.clone(), resolution_clone.clone());
            }))
    }
    
    fn render_content_input(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .text_color(rgb(0x495057))
                    .child("OpenAPI Content")
            )
            .child(
                Input::new(&self.content_input)
                    .w_full()
                    .h(px(120.0))
            )
            .child(self.render_error_display(_cx))
    }
    
    fn render_folder_selection(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .text_color(rgb(0x495057))
                    .child("Target Folder (Optional)")
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x6c757d))
                    .child("Select a folder to import services into. Leave empty to use default tag-based folders.")
            )
            .child(
                div()
                    .max_h(px(200.0))
                    .child(self.folder_selector.clone())
            )
    }
    
    fn render_import_options(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .text_color(rgb(0x495057))
                    .child("Import Options")
            )
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("create_subfolders_checkbox")
                            .ghost()
                            .compact()
                            .w(px(16.0))
                            .h(px(16.0))
                            .border_1()
                            .border_color(rgb(0xdee2e6))
                            .rounded(px(2.0))
                            .when(self.create_subfolders, |btn| {
                                btn.bg(rgb(0x007bff))
                                    .border_color(rgb(0x007bff))
                                    .child(
                                        gpui_component::Icon::new(IconName::Check)
                                            .size_3()
                                            .text_color(rgb(0xffffff))
                                    )
                            })
                            .on_click(cx.listener(|this, _, _, _cx| {
                                this.create_subfolders = !this.create_subfolders;
                            }))
                    )
                    .child(
                        div()
                            .v_flex()
                            .child(
                                div()
                                    .text_sm()
                                    .child("Create subfolders based on tags")
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x6c757d))
                                    .child("When enabled, creates subfolders under the target folder based on OpenAPI tags")
                            )
                    )
            )
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .child("Schema Import Strategy")
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap_1()
                            .child(self.render_schema_option("Same as services", SchemaFolderStrategy::SameAsServices, cx))
                            .child(self.render_schema_option("Separate schema folder", SchemaFolderStrategy::SeparateFolder, cx))
                    )
            )
    }
    
    fn render_schema_option(&self, label: &str, strategy: SchemaFolderStrategy, cx: &mut Context<Self>) -> impl IntoElement {
        let is_selected = std::mem::discriminant(&self.schema_folder_strategy) == std::mem::discriminant(&strategy);
        let strategy_clone = strategy.clone();
        
        Button::new(format!("schema_option_{}", label.replace(" ", "_")))
            .ghost()
            .w_full()
            .justify_start()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .w(px(16.0))
                            .h(px(16.0))
                            .border_1()
                            .border_color(rgb(0xdee2e6))
                            .rounded_full()
                            .h_flex()
                            .items_center()
                            .justify_center()
                            .when(is_selected, |el| {
                                el.border_color(rgb(0x007bff))
                                    .child(
                                        div()
                                            .w(px(8.0))
                                            .h(px(8.0))
                                            .bg(rgb(0x007bff))
                                            .rounded_full()
                                    )
                            })
                    )
                    .child(
                        div()
                            .text_sm()
                            .child(label.to_string())
                    )
            )
            .on_click(cx.listener(move |this, _, _, _cx| {
                this.schema_folder_strategy = strategy_clone.clone();
            }))
    }
    
    /// Enhanced preview rendering with detailed folder path display and comprehensive information
    /// Implements Requirements 5.2, 5.3, 5.4 for preview functionality
    fn render_preview(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(preview) = &self.preview_data {
            div()
                .v_flex()
                .gap_4()
                .child(
                    div()
                        .h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            gpui_component::Icon::new(IconName::Eye)
                                .size_4()
                                .text_color(rgb(0x007bff))
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .text_color(rgb(0x495057))
                                .child("Import Preview")
                        )
                )
                .child(self.render_import_summary(preview))
                .child(self.render_target_folder_structure(preview))
                .child(self.render_services_preview(preview))
                .when(!preview.schemas.is_empty(), |el| {
                    el.child(self.render_schemas_preview(preview))
                })
                .when(!preview.folders_to_create.is_empty(), |el| {
                    el.child(self.render_folders_to_create(preview))
                })
        } else {
            div()
                .p_4()
                .text_center()
                .text_sm()
                .text_color(rgb(0x6c757d))
                .child("Enter OpenAPI content to see preview")
        }
    }
    
    /// Render import summary with counts and overview
    /// Implements Requirements 5.2, 5.3 for accurate service and schema counts
    fn render_import_summary(&self, preview: &ImportPreview) -> impl IntoElement {
        div()
            .p_3()
            .bg(rgb(0xf8f9fa))
            .border_1()
            .border_color(rgb(0xe9ecef))
            .rounded(px(6.0))
            .v_flex()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_medium()
                    .text_color(rgb(0x495057))
                    .child("Import Summary")
            )
            .child(
                div()
                    .h_flex()
                    .gap_4()
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                gpui_component::Icon::new(IconName::Globe)
                                    .size_4()
                                    .text_color(rgb(0x28a745))
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .child(format!("{} Services", preview.services.len()))
                            )
                    )
                    .when(!preview.schemas.is_empty(), |el| {
                        el.child(
                            div()
                                .h_flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    gpui_component::Icon::new(IconName::File)
                                        .size_4()
                                        .text_color(rgb(0x6f42c1))
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .child(format!("{} Schemas", preview.schemas.len()))
                                )
                        )
                    })
                    .when(!preview.folders_to_create.is_empty(), |el| {
                        el.child(
                            div()
                                .h_flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    gpui_component::Icon::new(IconName::Folder)
                                        .size_4()
                                        .text_color(rgb(0xffc107))
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .child(format!("{} New Folders", preview.folders_to_create.len()))
                                )
                        )
                    })
            )
    }
    
    /// Render target folder structure and path information
    /// Implements Requirement 5.4 for folder path and structure display
    fn render_target_folder_structure(&self, preview: &ImportPreview) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_medium()
                    .text_color(rgb(0x495057))
                    .child("Target Structure")
            )
            .child(
                div()
                    .p_3()
                    .border_1()
                    .border_color(rgb(0xe9ecef))
                    .rounded(px(4.0))
                    .bg(rgb(0xfafbfc))
                    .v_flex()
                    .gap_2()
                    .child(self.render_folder_hierarchy(preview))
            )
    }
    
    /// Render folder hierarchy visualization
    /// Implements Requirement 5.4 for detailed folder structure display
    fn render_folder_hierarchy(&self, preview: &ImportPreview) -> impl IntoElement {
        div()
            .v_flex()
            .gap_1()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        gpui_component::Icon::new(IconName::Folder)
                            .size_4()
                            .text_color(rgb(0x6c757d))
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .text_color(rgb(0x495057))
                            .child(self.get_root_folder_display_name())
                    )
            )
            .children(
                self.get_unique_target_folders(preview).into_iter().map(|folder_path| {
                    self.render_folder_path_item(&folder_path)
                })
            )
    }
    
    /// Get display name for root folder based on current selection
    fn get_root_folder_display_name(&self) -> String {
        if let Some(folder_id) = self.selected_folder {
            self.available_folders
                .iter()
                .find(|f| f.id == folder_id)
                .map(|f| f.name.as_ref().to_string())
                .unwrap_or_else(|| "Selected Folder".to_string())
        } else {
            "Workspace Root".to_string()
        }
    }
    
    /// Get unique target folders from preview data
    fn get_unique_target_folders(&self, preview: &ImportPreview) -> Vec<String> {
        let mut folders = std::collections::HashSet::new();
        
        // Add service target folders
        for service in &preview.services {
            folders.insert(service.target_folder.clone());
        }
        
        // Add schema target folders
        for schema in &preview.schemas {
            folders.insert(schema.target_folder.clone());
        }
        
        let mut folder_list: Vec<String> = folders.into_iter().collect();
        folder_list.sort();
        folder_list
    }
    
    /// Render individual folder path item with proper indentation
    fn render_folder_path_item(&self, folder_path: &str) -> impl IntoElement {
        let depth = folder_path.matches('/').count();
        let indent = px((depth * 16) as f32 + 16.0);
        
        div()
            .ml(indent)
            .h_flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x6c757d))
                    .child("└─")
            )
            .child(
                gpui_component::Icon::new(IconName::Folder)
                    .size_3()
                    .text_color(rgb(0x007bff))
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x495057))
                    .child(folder_path.split('/').last().unwrap_or(folder_path).to_string())
            )
    }
    
    /// Enhanced services preview with detailed information
    /// Implements Requirement 5.2 for accurate service information display
    fn render_services_preview(&self, preview: &ImportPreview) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .text_color(rgb(0x495057))
                            .child("Services to Import")
                    )
                    .child(
                        div()
                            .text_xs()
                            .px_2()
                            .py_1()
                            .bg(rgb(0x28a745))
                            .text_color(rgb(0xffffff))
                            .rounded(px(12.0))
                            .child(format!("{} items", preview.services.len()))
                    )
            )
            .when(!preview.services.is_empty(), |el| {
                el.child(
                    div()
                        .max_h(px(150.0))
                        .overflow_y_hidden()
                        .border_1()
                        .border_color(rgb(0xe9ecef))
                        .rounded(px(4.0))
                        .v_flex()
                        .children(
                            preview.services.iter().enumerate().map(|(index, service)| {
                                self.render_service_preview_item(service, index)
                            })
                        )
                )
            })
            .when(preview.services.is_empty(), |el| {
                el.child(
                    div()
                        .p_3()
                        .text_center()
                        .text_sm()
                        .text_color(rgb(0x6c757d))
                        .child("No services found in OpenAPI specification")
                )
            })
    }
    
    /// Render individual service preview item with enhanced details
    fn render_service_preview_item(&self, service: &ServicePreview, index: usize) -> impl IntoElement {
        div()
            .p_3()
            .when(index % 2 == 0, |el| el.bg(rgb(0xf8f9fa)))
            .border_b_1()
            .border_color(rgb(0xe9ecef))
            .v_flex()
            .gap_2()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .px_2()
                            .py_1()
                            .rounded(px(3.0))
                            .bg(self.get_method_color(&service.method))
                            .text_color(rgb(0xffffff))
                            .font_medium()
                            .child(service.method.clone())
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .text_color(rgb(0x212529))
                            .child(service.name.clone())
                    )
            )
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x6c757d))
                            .child("Path:")
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_family("monospace")
                            .text_color(rgb(0x495057))
                            .child(service.path.clone())
                    )
            )
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x6c757d))
                            .child("Target:")
                    )
                    .child(
                        gpui_component::Icon::new(IconName::ArrowRight)
                            .size_3()
                            .text_color(rgb(0x007bff))
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x007bff))
                            .child(service.target_folder.clone())
                    )
                    .when(service.tag.is_some(), |el| {
                        el.child(
                            div()
                                .text_xs()
                                .px_1()
                                .rounded(px(2.0))
                                .bg(rgb(0xe9ecef))
                                .text_color(rgb(0x495057))
                                .child(format!("tag: {}", service.tag.as_ref().unwrap()))
                        )
                    })
            )
    }
    
    /// Get color for HTTP method badge
    fn get_method_color(&self, method: &str) -> gpui::Rgba {
        match method.to_uppercase().as_str() {
            "GET" => rgb(0x28a745),      // Green
            "POST" => rgb(0x007bff),     // Blue
            "PUT" => rgb(0xffc107),      // Yellow
            "DELETE" => rgb(0xdc3545),   // Red
            "PATCH" => rgb(0x6f42c1),    // Purple
            _ => rgb(0x6c757d),          // Gray
        }
    }
    
    /// Enhanced schemas preview with detailed information
    /// Implements Requirement 5.3 for accurate schema information display
    fn render_schemas_preview(&self, preview: &ImportPreview) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .text_color(rgb(0x495057))
                            .child("Schemas to Import")
                    )
                    .child(
                        div()
                            .text_xs()
                            .px_2()
                            .py_1()
                            .bg(rgb(0x6f42c1))
                            .text_color(rgb(0xffffff))
                            .rounded(px(12.0))
                            .child(format!("{} items", preview.schemas.len()))
                    )
            )
            .child(
                div()
                    .max_h(px(120.0))
                    .overflow_y_hidden()
                    .border_1()
                    .border_color(rgb(0xe9ecef))
                    .rounded(px(4.0))
                    .v_flex()
                    .children(
                        preview.schemas.iter().enumerate().map(|(index, schema)| {
                            self.render_schema_preview_item(schema, index)
                        })
                    )
            )
    }
    
    /// Render individual schema preview item with enhanced details
    fn render_schema_preview_item(&self, schema: &SchemaPreview, index: usize) -> impl IntoElement {
        div()
            .p_3()
            .when(index % 2 == 0, |el| el.bg(rgb(0xf8f9fa)))
            .border_b_1()
            .border_color(rgb(0xe9ecef))
            .h_flex()
            .items_center()
            .gap_3()
            .child(
                gpui_component::Icon::new(IconName::File)
                    .size_4()
                    .text_color(rgb(0x6f42c1))
            )
            .child(
                div()
                    .flex_1()
                    .v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .text_color(rgb(0x212529))
                            .child(schema.name.clone())
                    )
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .px_1()
                                    .rounded(px(2.0))
                                    .bg(rgb(0xe9ecef))
                                    .text_color(rgb(0x495057))
                                    .child(schema.schema_type.clone())
                            )
                            .child(
                                gpui_component::Icon::new(IconName::ArrowRight)
                                    .size_3()
                                    .text_color(rgb(0x007bff))
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x007bff))
                                    .child(schema.target_folder.clone())
                            )
                    )
            )
    }
    
    /// Render folders to be created section
    fn render_folders_to_create(&self, preview: &ImportPreview) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .text_color(rgb(0x495057))
                            .child("New Folders to Create")
                    )
                    .child(
                        div()
                            .text_xs()
                            .px_2()
                            .py_1()
                            .bg(rgb(0xffc107))
                            .text_color(rgb(0x212529))
                            .rounded(px(12.0))
                            .child(format!("{} folders", preview.folders_to_create.len()))
                    )
            )
            .child(
                div()
                    .p_3()
                    .border_1()
                    .border_color(rgb(0xe9ecef))
                    .rounded(px(4.0))
                    .bg(rgb(0xfff3cd))
                    .v_flex()
                    .gap_2()
                    .children(
                        preview.folders_to_create.iter().map(|folder| {
                            div()
                                .h_flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    gpui_component::Icon::new(IconName::Plus)
                                        .size_3()
                                        .text_color(rgb(0x856404))
                                )
                                .child(
                                    gpui_component::Icon::new(IconName::Folder)
                                        .size_4()
                                        .text_color(rgb(0x856404))
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(0x856404))
                                        .child(folder.clone())
                                )
                        })
                    )
            )
    }
    
    fn render_actions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h_flex()
            .justify_end()
            .gap_2()
            .child(
                Button::new("cancel_import")
                    .ghost()
                    .child("Cancel")
                    .on_click(cx.listener(|_this, _, _, cx| {
                        cx.emit(ImportDialogEvent::Cancel);
                    }))
            )
            .when(self.show_conflict_resolution, |el| {
                el.child(
                    Button::new("resolve_conflicts")
                        .primary()
                        .when(self.is_loading, |btn| btn.disabled(true))
                        .when(self.pending_conflicts.as_ref().map_or(true, |conflicts| {
                            conflicts.iter().any(|c| !self.conflict_resolutions.contains_key(&c.name))
                        }), |btn| btn.disabled(true))
                        .child(if self.is_loading { "处理中..." } else { "解决冲突并导入" })
                        .on_click(cx.listener(|this, _, _, cx| {
                            let content = this.content_input.read(cx).value().to_string();
                            if !content.trim().is_empty() {
                                cx.emit(ImportDialogEvent::ResolveConflicts(
                                    ImportConfig {
                                        content,
                                        target_folder: this.selected_folder,
                                        create_subfolders: this.create_subfolders,
                                        schema_folder_strategy: this.schema_folder_strategy.clone(),
                                    },
                                    this.conflict_resolutions.clone(),
                                ));
                            }
                        }))
                )
            })
            .when(!self.show_conflict_resolution, |el| {
                el.child(
                    Button::new("confirm_import")
                        .primary()
                        .when(self.is_loading, |btn| btn.disabled(true))
                        .child(if self.is_loading { "Importing..." } else { "Import" })
                        .on_click(cx.listener(|this, _, _, cx| {
                            // Validate input first
                            if let Err(_) = this.validate_input(cx) {
                                return; // Error will be displayed by validation
                            }
                            
                            let content = this.content_input.read(cx).value().to_string();
                            if !content.trim().is_empty() {
                                cx.emit(ImportDialogEvent::Import(ImportConfig {
                                    content,
                                    target_folder: this.selected_folder,
                                    create_subfolders: this.create_subfolders,
                                    schema_folder_strategy: this.schema_folder_strategy.clone(),
                                }));
                            }
                        }))
                )
            })
    }
}

impl Render for ImportDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Subscribe to folder selector events
        cx.subscribe(&self.folder_selector, |this, _selector, event, cx| {
            match event {
                crate::FolderSelectorEvent::SelectFolder(folder_id) => {
                    this.handle_folder_selection(*folder_id, cx);
                }
                crate::FolderSelectorEvent::ToggleFolder(_) => {
                    // Handle folder toggle if needed
                }
            }
        }).detach();
        
        div()
            .w(px(650.0))
            .max_h(px(900.0))
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(0xdee2e6))
            .rounded_lg()
            .shadow_lg()
            .v_flex()
            .child(
                // Header
                div()
                    .p_4()
                    .border_b_1()
                    .border_color(rgb(0xe9ecef))
                    .child(
                        div()
                            .text_lg()
                            .font_semibold()
                            .text_color(rgb(0x212529))
                            .child("Import OpenAPI")
                    )
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
                    .when(!self.show_conflict_resolution, |el| {
                        el.child(self.render_folder_selection(cx))
                            .child(self.render_import_options(cx))
                            .when(self.show_preview, |content| {
                                content.child(self.render_preview(cx))
                            })
                    })
                    .when(self.show_conflict_resolution, |el| {
                        el.child(self.render_conflict_resolution(cx))
                    })
            )
            .child(
                // Footer
                div()
                    .p_4()
                    .border_t_1()
                    .border_color(rgb(0xe9ecef))
                    .child(self.render_actions(cx))
            )
    }
}