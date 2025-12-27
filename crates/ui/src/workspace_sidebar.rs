use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use gpui_component::{IconName, StyledExt, WindowExt};
use models::{Folder, FolderType, Schema, Service, ServiceType, Workspace};
use std::collections::{HashMap, HashSet};
use gpui_component::scroll::ScrollableElement;
use uuid::Uuid;
use workspace::ConflictResolution;

use crate::{ImportDialog, ImportDialogEvent, ImportConfig};

// === Error Handling Constants ===

/// Maximum allowed tree depth to prevent stack overflow and UI issues
const MAX_TREE_DEPTH: usize = 10;

/// Minimum sidebar width to maintain usability
const MIN_SIDEBAR_WIDTH: f32 = 180.0;

/// Maximum sidebar width to prevent excessive space usage
const MAX_SIDEBAR_WIDTH: f32 = 600.0;

// === Menu Types and Options ===

/// Menu options for folder items
#[derive(Debug, Clone)]
pub struct FolderMenuOptions {
    pub can_create_service: bool,
    pub can_create_schema: bool,
    pub can_create_folder: bool,
    pub can_import: bool,
    pub can_delete: bool,
}

/// Menu options for service items
#[derive(Debug, Clone)]
pub struct ServiceMenuOptions {
    pub can_delete: bool,
}

/// Menu options for schema items
#[derive(Debug, Clone)]
pub struct SchemaMenuOptions {
    pub can_delete: bool,
}

/// The type of context menu to display
#[derive(Debug, Clone)]
pub enum MenuType {
    Folder(FolderMenuOptions),
    Service(ServiceMenuOptions),
    Schema(SchemaMenuOptions),
}

/// Context menu button component
#[derive(Debug, Clone)]
pub struct ContextMenuButton {
    pub item_id: Uuid,
    pub menu_type: MenuType,
    pub visible: bool,
}

impl ContextMenuButton {
    /// Create a new ContextMenuButton for a folder
    pub fn folder(item_id: Uuid, options: FolderMenuOptions) -> Self {
        Self {
            item_id,
            menu_type: MenuType::Folder(options),
            visible: false,
        }
    }

    /// Create a new ContextMenuButton for a service
    pub fn service(item_id: Uuid, options: ServiceMenuOptions) -> Self {
        Self {
            item_id,
            menu_type: MenuType::Service(options),
            visible: false,
        }
    }

    /// Create a new ContextMenuButton for a schema
    pub fn schema(item_id: Uuid, options: SchemaMenuOptions) -> Self {
        Self {
            item_id,
            menu_type: MenuType::Schema(options),
            visible: false,
        }
    }

    /// Set the visibility of the menu button
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Check if the menu button should be visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Get the menu type
    pub fn menu_type(&self) -> &MenuType {
        &self.menu_type
    }

    /// Render the context menu button
    pub fn render(&self, cx: &mut Context<WorkspaceSidebar>) -> impl IntoElement {
        let item_id = self.item_id;
        let menu_type = self.menu_type.clone();
        
        // Only render if visible
        if !self.visible {
            return div().into_any_element();
        }

        // Create dropdown menu based on menu type
        let view = cx.entity();
        
        Button::new(format!("context_menu_{}", self.item_id))
            .ghost()
            .compact()
            .icon(IconName::Settings)
            .dropdown_menu(move |menu, window, _cx| {
                match &menu_type {
                    MenuType::Folder(options) => {
                        let mut menu = menu;
                        
                        // Create menu items for folder
                        if options.can_create_service {
                            menu = menu.item(
                                PopupMenuItem::new("Create Service").on_click(
                                    window.listener_for(&view, move |this, _, _window, cx| {
                                        this.handle_menu_event(WorkspaceSidebarEvent::CreateService("New Service".to_string(), Some(item_id)), cx);
                                    })
                                )
                            );
                        }
                        
                        if options.can_create_schema {
                            menu = menu.item(
                                PopupMenuItem::new("Create Schema").on_click(
                                    window.listener_for(&view, move |this, _, _window, cx| {
                                        this.handle_menu_event(WorkspaceSidebarEvent::CreateSchema("New Schema".to_string(), Some(item_id)), cx);
                                    })
                                )
                            );
                        }
                        
                        if options.can_create_folder {
                            menu = menu.item(
                                PopupMenuItem::new("Create Folder").on_click(
                                    window.listener_for(&view, move |this, _, _window, cx| {
                                        // Determine folder type based on parent folder
                                        let folder_type = if let Some(workspace) = &this.workspace {
                                            workspace.folders.iter()
                                                .find(|f| f.id == item_id)
                                                .map(|f| f.folder_type.clone())
                                                .unwrap_or(FolderType::Service)
                                        } else {
                                            FolderType::Service
                                        };
                                        this.handle_menu_event(WorkspaceSidebarEvent::CreateFolder("New Folder".to_string(), Some(item_id), folder_type), cx);
                                    })
                                )
                            );
                        }
                        
                        // View Details option
                        menu = menu.item(
                            PopupMenuItem::new("View Details").on_click(
                                window.listener_for(&view, move |this, _, _window, cx| {
                                    this.handle_menu_event(WorkspaceSidebarEvent::OpenFolderDetail(item_id), cx);
                                })
                            )
                        );
                        
                        // Import option
                        if options.can_import {
                            menu = menu.separator().item(
                                PopupMenuItem::new("Import OpenAPI").on_click(
                                    window.listener_for(&view, move |this, _, window, cx| {
                                        this.open_import_openapi_dialog(window, cx);
                                    })
                                )
                            );
                        }
                        
                        // Delete option
                        if options.can_delete {
                            if options.can_create_service || options.can_create_schema || options.can_create_folder || options.can_import {
                                menu = menu.separator();
                            }
                            menu = menu.item(
                                PopupMenuItem::new("Delete").on_click(
                                    window.listener_for(&view, move |this, _, _window, cx| {
                                        this.handle_menu_event(WorkspaceSidebarEvent::DeleteFolder(item_id), cx);
                                    })
                                )
                            );
                        }
                        
                        menu
                    },
                    MenuType::Service(_options) => {
                        menu.item(
                            PopupMenuItem::new("Delete").on_click(
                                window.listener_for(&view, move |this, _, _window, cx| {
                                    this.handle_menu_event(WorkspaceSidebarEvent::DeleteService(item_id), cx);
                                })
                            )
                        )
                    },
                    MenuType::Schema(_options) => {
                        menu.item(
                            PopupMenuItem::new("Delete").on_click(
                                window.listener_for(&view, move |this, _, _window, cx| {
                                    this.handle_menu_event(WorkspaceSidebarEvent::DeleteSchema(item_id), cx);
                                })
                            )
                        )
                    }
                }
            })
            .into_any_element()
    }
}

// === Tree Item Types ===

/// Represents a single item in the tree structure
#[derive(Debug, Clone)]
pub struct TreeItem {
    pub item_type: TreeItemType,
    pub depth: usize,
    pub is_expanded: bool,
    pub is_active: bool,
    pub show_menu: bool,
    pub context_menu_button: Option<ContextMenuButton>,
}

/// The type of tree item
#[derive(Debug, Clone)]
pub enum TreeItemType {
    Folder(Folder),
    Service(Service),
    Schema(Schema),
}

/// State for rendering tree items
#[derive(Debug, Clone)]
pub struct TreeItemState {
    pub id: Uuid,
    pub name: String,
    pub item_type: TreeItemType,
    pub depth: usize,
    pub is_expanded: bool,
    pub is_active: bool,
    pub parent_id: Option<Uuid>,
    pub children_count: usize,
}

impl TreeItem {
    /// Create a new TreeItem for a folder
    pub fn folder(folder: Folder, depth: usize, is_expanded: bool) -> Self {
        let folder_options = FolderMenuOptions {
            can_create_service: matches!(folder.folder_type, FolderType::Service),
            can_create_schema: matches!(folder.folder_type, FolderType::Schema),
            can_create_folder: true,
            can_import: matches!(folder.folder_type, FolderType::Service),
            can_delete: true,
        };
        
        let mut context_menu_button = ContextMenuButton::folder(folder.id, folder_options);
        context_menu_button.set_visible(true); // Always set visible, we control visibility at render time
        
        Self {
            item_type: TreeItemType::Folder(folder),
            depth,
            is_expanded,
            is_active: false,
            show_menu: false,
            context_menu_button: Some(context_menu_button),
        }
    }

    /// Create a new TreeItem for a service
    pub fn service(service: Service, depth: usize, is_active: bool) -> Self {
        let service_options = ServiceMenuOptions {
            can_delete: true,
        };
        
        let mut context_menu_button = ContextMenuButton::service(service.id, service_options);
        context_menu_button.set_visible(true); // Always set visible, we control visibility at render time
        
        Self {
            item_type: TreeItemType::Service(service),
            depth,
            is_expanded: false,
            is_active,
            show_menu: false,
            context_menu_button: Some(context_menu_button),
        }
    }

    /// Create a new TreeItem for a schema
    pub fn schema(schema: Schema, depth: usize) -> Self {
        let schema_options = SchemaMenuOptions {
            can_delete: true,
        };
        
        let mut context_menu_button = ContextMenuButton::schema(schema.id, schema_options);
        context_menu_button.set_visible(true); // Always set visible, we control visibility at render time
        
        Self {
            item_type: TreeItemType::Schema(schema),
            depth,
            is_expanded: false,
            is_active: false,
            show_menu: false,
            context_menu_button: Some(context_menu_button),
        }
    }

    /// Get the ID of this tree item
    pub fn id(&self) -> Uuid {
        match &self.item_type {
            TreeItemType::Folder(folder) => folder.id,
            TreeItemType::Service(service) => service.id,
            TreeItemType::Schema(schema) => schema.id,
        }
    }

    /// Get the name of this tree item
    pub fn name(&self) -> &str {
        match &self.item_type {
            TreeItemType::Folder(folder) => &folder.name,
            TreeItemType::Service(service) => &service.name,
            TreeItemType::Schema(schema) => &schema.name,
        }
    }

    /// Calculate the left padding for indentation based on depth
    pub fn calculate_indentation(&self) -> f32 {
        self.depth as f32 * 16.0
    }

    /// Check if this item can be expanded (only folders can be expanded)
    pub fn can_expand(&self) -> bool {
        matches!(self.item_type, TreeItemType::Folder(_))
    }

    /// Get the parent ID of this tree item
    pub fn parent_id(&self) -> Option<Uuid> {
        match &self.item_type {
            TreeItemType::Folder(folder) => folder.parent_id,
            TreeItemType::Service(service) => service.folder_id,
            TreeItemType::Schema(schema) => schema.folder_id,
        }
    }

    /// Set the menu visibility for this item
    pub fn set_menu_visible(&mut self, visible: bool) {
        self.show_menu = visible;
        if let Some(ref mut menu_button) = self.context_menu_button {
            menu_button.set_visible(visible);
        }
    }

    /// Check if the menu should be visible
    pub fn is_menu_visible(&self) -> bool {
        self.show_menu
    }
}

impl TreeItemType {
    /// Get the folder type if this is a folder
    pub fn folder_type(&self) -> Option<FolderType> {
        match self {
            TreeItemType::Folder(folder) => Some(folder.folder_type.clone()),
            _ => None,
        }
    }

    /// Check if this is a folder
    pub fn is_folder(&self) -> bool {
        matches!(self, TreeItemType::Folder(_))
    }

    /// Check if this is a service
    pub fn is_service(&self) -> bool {
        matches!(self, TreeItemType::Service(_))
    }

    /// Check if this is a schema
    pub fn is_schema(&self) -> bool {
        matches!(self, TreeItemType::Schema(_))
    }
}

#[derive(Debug)]
pub enum WorkspaceSidebarEvent {
    OpenService(Uuid),
    OpenSchema(Uuid),
    OpenFolderDetail(Uuid), // New event for opening folder details
    CreateService(String, Option<Uuid>),
    CreateSchema(String, Option<Uuid>),
    CreateFolder(String, Option<Uuid>, FolderType),
    ToggleFolder(Uuid),
    ImportOpenApi(ImportConfig),
    PreviewImport(ImportConfig),
    RequestImportPreview(ImportConfig),
    ResolveImportConflicts(ImportConfig, HashMap<String, ConflictResolution>),
    // New menu events
    DeleteService(Uuid),
    DeleteSchema(Uuid),
    DeleteFolder(Uuid),
    ShowContextMenu(Uuid, MenuType),
}

pub struct WorkspaceSidebar {
    workspace: Option<Workspace>,
    pub expanded_folders: HashSet<Uuid>,
    expanded_sections: (bool, bool), // (services, schemas)
    hovered_item: Option<Uuid>, // Track which item is being hovered
    active_item: Option<Uuid>, // Track which item is currently active/selected
    pressed_item: Option<Uuid>, // Track which item is being pressed/clicked
    sidebar_width: f32, // Track current sidebar width for responsive layout
}

impl EventEmitter<WorkspaceSidebarEvent> for WorkspaceSidebar {}

impl WorkspaceSidebar {
    pub fn new() -> Self {
        Self {
            workspace: None,
            expanded_folders: HashSet::new(),
            expanded_sections: (true, true),
            hovered_item: None,
            active_item: None,
            pressed_item: None,
            sidebar_width: 300.0, // Default sidebar width
        }
    }

    pub fn set_workspace(&mut self, workspace: Workspace) {
        if self.workspace.is_none() {
            for folder in &workspace.folders {
                self.expanded_folders.insert(folder.id);
            }
        }
        self.workspace = Some(workspace);
    }

    // === Error Handling and Validation Methods ===

    /// Validate if a project ID exists in the current workspace
    fn validate_project_id(&self, project_id: Uuid) -> Result<(), String> {
        let Some(workspace) = &self.workspace else {
            return Err("No workspace available".to_string());
        };

        // Check if it's a valid folder
        if workspace.folders.iter().any(|f| f.id == project_id) {
            return Ok(());
        }

        // Check if it's a valid service
        if workspace.services.iter().any(|s| s.id == project_id) {
            return Ok(());
        }

        // Check if it's a valid schema
        if workspace.schemas.iter().any(|s| s.id == project_id) {
            return Ok(());
        }

        Err(format!("Project ID {} not found in workspace", project_id))
    }

    /// Validate tree depth to prevent overflow
    fn validate_tree_depth(&self, depth: usize) -> Result<usize, String> {
        if depth > MAX_TREE_DEPTH {
            eprintln!("Warning: Tree depth {} exceeds maximum allowed depth {}. Limiting to maximum.", depth, MAX_TREE_DEPTH);
            Ok(MAX_TREE_DEPTH)
        } else {
            Ok(depth)
        }
    }

    /// Validate and clamp sidebar width to acceptable bounds
    fn validate_sidebar_width(&self, width: f32) -> f32 {
        width.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH)
    }

    /// Check if a folder can be deleted (not empty)
    fn can_delete_folder(&self, folder_id: Uuid) -> Result<bool, String> {
        let Some(workspace) = &self.workspace else {
            return Err("No workspace available".to_string());
        };

        // Check if folder has any child folders
        let has_child_folders = workspace.folders.iter().any(|f| f.parent_id == Some(folder_id));
        
        // Check if folder has any services
        let has_services = workspace.services.iter().any(|s| s.folder_id == Some(folder_id));
        
        // Check if folder has any schemas
        let has_schemas = workspace.schemas.iter().any(|s| s.folder_id == Some(folder_id));

        if has_child_folders || has_services || has_schemas {
            Ok(false) // Cannot delete non-empty folder
        } else {
            Ok(true) // Can delete empty folder
        }
    }

    /// Safe menu rendering with fallback for errors
    fn safe_render_context_menu(&self, item: &TreeItem, cx: &mut Context<Self>) -> AnyElement {
        // Don't check item.is_menu_visible() since we now control visibility at render time
        match self.try_render_context_menu(item, cx) {
            Ok(menu_element) => menu_element,
            Err(error) => {
                eprintln!("Warning: Failed to render context menu for item {}: {}. Using fallback.", item.id(), error);
                self.render_fallback_menu(item.id(), &item.item_type, cx)
            }
        }
    }

    /// Try to render context menu, returning error if it fails
    fn try_render_context_menu(&self, item: &TreeItem, cx: &mut Context<Self>) -> Result<AnyElement, String> {
        let Some(ref menu_button) = item.context_menu_button else {
            return Err("No context menu button available".to_string());
        };

        // Don't check menu_button.is_visible() since we control visibility at render time

        // Validate the item ID exists
        self.validate_project_id(item.id())?;

        // Render the menu button
        Ok(menu_button.render(cx).into_any_element())
    }

    /// Render a simple fallback menu when the main menu fails
    fn render_fallback_menu(&self, item_id: Uuid, item_type: &TreeItemType, cx: &mut Context<Self>) -> AnyElement {
        // Simple fallback button that only shows a basic menu
        let view = cx.entity();
        let item_type_clone = item_type.clone();
        
        Button::new(format!("fallback_menu_{}", item_id))
            .ghost()
            .compact()
            .icon(IconName::Settings)
            .dropdown_menu(move |menu, window, _cx| {
                let item_type_for_closure = item_type_clone.clone();
                menu.item(
                    PopupMenuItem::new("Delete").on_click(
                        window.listener_for(&view, move |this, _, _window, cx| {
                            // Emit appropriate delete event based on item type
                            match &item_type_for_closure {
                                TreeItemType::Folder(_) => {
                                    this.handle_menu_event(WorkspaceSidebarEvent::DeleteFolder(item_id), cx);
                                }
                                TreeItemType::Service(_) => {
                                    this.handle_menu_event(WorkspaceSidebarEvent::DeleteService(item_id), cx);
                                }
                                TreeItemType::Schema(_) => {
                                    this.handle_menu_event(WorkspaceSidebarEvent::DeleteSchema(item_id), cx);
                                }
                            }
                        })
                    )
                )
            })
            .into_any_element()
    }

    /// Set which item is currently hovered (for testing menu visibility)
    pub fn set_hovered_item(&mut self, item_id: Option<Uuid>) {
        self.hovered_item = item_id;
    }

    /// Set which item is currently active/selected
    pub fn set_active_item(&mut self, item_id: Option<Uuid>) {
        self.active_item = item_id;
    }

    /// Set which item is currently being pressed/clicked
    pub fn set_pressed_item(&mut self, item_id: Option<Uuid>) {
        self.pressed_item = item_id;
    }

    /// Set the sidebar width for responsive layout calculations
    pub fn set_sidebar_width(&mut self, width: f32) {
        self.sidebar_width = self.validate_sidebar_width(width);
    }

    /// Get the current sidebar width
    pub fn get_sidebar_width(&self) -> f32 {
        self.sidebar_width
    }

    /// Calculate the maximum width available for item names based on indentation and menu button space
    fn calculate_max_item_name_width(&self, depth: usize) -> f32 {
        let indentation = depth as f32 * 16.0;
        let menu_button_space = 32.0; // Space reserved for menu button
        let padding_and_margins = 32.0; // Additional padding and margins
        let icon_space = 24.0; // Space for folder/service icons
        
        // Ensure minimum usable width
        let available_width = self.sidebar_width - indentation - menu_button_space - padding_and_margins - icon_space;
        available_width.max(60.0) // Minimum 60px for item names
    }

    /// Truncate text with ellipsis if it exceeds the maximum width
    fn truncate_text_with_ellipsis(&self, text: &str, max_width: f32) -> String {
        // Simple character-based truncation (in a real implementation, you might want to measure actual text width)
        let chars_per_pixel = 0.15; // Approximate characters per pixel for typical font
        let max_chars = (max_width * chars_per_pixel) as usize;
        
        if text.len() <= max_chars {
            text.to_string()
        } else if max_chars <= 3 {
            "...".to_string()
        } else {
            format!("{}...", &text[..max_chars.saturating_sub(3)])
        }
    }

    /// Check if the sidebar is at minimum width
    fn is_at_minimum_width(&self) -> bool {
        self.sidebar_width <= MIN_SIDEBAR_WIDTH
    }

    /// Get the minimum required width for deep indentation levels
    fn get_minimum_width_for_depth(&self, depth: usize) -> f32 {
        let indentation = depth as f32 * 16.0;
        let minimum_content_width = 120.0; // Minimum space for content and menu button
        indentation + minimum_content_width
    }

    /// Check if an item is in hover state
    pub fn is_item_hovered(&self, item_id: Uuid) -> bool {
        self.hovered_item == Some(item_id)
    }

    /// Check if an item is in active state
    pub fn is_item_active(&self, item_id: Uuid) -> bool {
        self.active_item == Some(item_id)
    }

    /// Check if an item is in pressed state
    pub fn is_item_pressed(&self, item_id: Uuid) -> bool {
        self.pressed_item == Some(item_id)
    }

    /// Get the background color for an item based on its interaction state
    fn get_item_background_color(&self, item_id: Uuid, item_type: &TreeItemType) -> Hsla {
        if self.is_item_pressed(item_id) {
            // Pressed state - darker background
            match item_type {
                TreeItemType::Folder(_) => rgb(0xe3f2fd).into(), // Light blue for folders
                TreeItemType::Service(_) => rgb(0xe8f5e8).into(), // Light green for services
                TreeItemType::Schema(_) => rgb(0xf3e5f5).into(), // Light purple for schemas
            }
        } else if self.is_item_active(item_id) {
            // Active state - medium background
            match item_type {
                TreeItemType::Folder(_) => rgb(0xbbdefb).into(), // Medium blue for folders
                TreeItemType::Service(_) => rgb(0xc8e6c9).into(), // Medium green for services
                TreeItemType::Schema(_) => rgb(0xe1bee7).into(), // Medium purple for schemas
            }
        } else if self.is_item_hovered(item_id) {
            // Hover state - light background
            rgb(0xf5f5f5).into() // Light gray for all items on hover
        } else {
            // Default state - transparent
            hsla(0.0, 0.0, 0.0, 0.0)
        }
    }

    /// Get the border color for an item based on its interaction state
    fn get_item_border_color(&self, item_id: Uuid, item_type: &TreeItemType) -> Option<Hsla> {
        if self.is_item_active(item_id) {
            // Active items get a colored border
            match item_type {
                TreeItemType::Folder(_) => Some(rgb(0x2196f3).into()), // Blue border for folders
                TreeItemType::Service(_) => Some(rgb(0x4caf50).into()), // Green border for services
                TreeItemType::Schema(_) => Some(rgb(0x9c27b0).into()), // Purple border for schemas
            }
        } else {
            None
        }
    }

    /// Get menu button styling based on item type
    fn get_menu_button_style(&self, item_type: &TreeItemType) -> (&'static str, Hsla) {
        match item_type {
            TreeItemType::Folder(_) => ("folder_menu", rgb(0x2196f3).into()), // Blue for folders
            TreeItemType::Service(_) => ("service_menu", rgb(0x4caf50).into()), // Green for services
            TreeItemType::Schema(_) => ("schema_menu", rgb(0x9c27b0).into()), // Purple for schemas
        }
    }

    /// Get context information for menu events
    pub fn get_menu_context(&self, item_id: Uuid) -> Option<(String, Option<Uuid>, Option<FolderType>)> {
        let workspace = self.workspace.as_ref()?;
        
        // Check if it's a folder
        if let Some(folder) = workspace.folders.iter().find(|f| f.id == item_id) {
            return Some((folder.name.to_string(), folder.parent_id, Some(folder.folder_type.clone())));
        }
        
        // Check if it's a service
        if let Some(service) = workspace.services.iter().find(|s| s.id == item_id) {
            return Some((service.name.to_string(), service.folder_id, None));
        }
        
        // Check if it's a schema
        if let Some(schema) = workspace.schemas.iter().find(|s| s.id == item_id) {
            return Some((schema.name.to_string(), schema.folder_id, None));
        }
        
        None
    }
    pub fn handle_menu_event(&mut self, event: WorkspaceSidebarEvent, cx: &mut Context<Self>) {
        // Enhanced validation with detailed error handling
        let validation_result = match &event {
            WorkspaceSidebarEvent::DeleteService(service_id) => {
                match self.validate_project_id(*service_id) {
                    Ok(_) => {
                        // Additional check: ensure it's actually a service
                        if let Some(workspace) = &self.workspace {
                            if workspace.services.iter().any(|s| s.id == *service_id) {
                                Ok(())
                            } else {
                                Err(format!("ID {} is not a service", service_id))
                            }
                        } else {
                            Err("No workspace available".to_string())
                        }
                    }
                    Err(e) => Err(e)
                }
            }
            WorkspaceSidebarEvent::DeleteSchema(schema_id) => {
                match self.validate_project_id(*schema_id) {
                    Ok(_) => {
                        // Additional check: ensure it's actually a schema
                        if let Some(workspace) = &self.workspace {
                            if workspace.schemas.iter().any(|s| s.id == *schema_id) {
                                Ok(())
                            } else {
                                Err(format!("ID {} is not a schema", schema_id))
                            }
                        } else {
                            Err("No workspace available".to_string())
                        }
                    }
                    Err(e) => Err(e)
                }
            }
            WorkspaceSidebarEvent::DeleteFolder(folder_id) => {
                match self.validate_project_id(*folder_id) {
                    Ok(_) => {
                        // Additional checks: ensure it's a folder and can be deleted
                        if let Some(workspace) = &self.workspace {
                            if workspace.folders.iter().any(|f| f.id == *folder_id) {
                                match self.can_delete_folder(*folder_id) {
                                    Ok(true) => Ok(()),
                                    Ok(false) => Err(format!("Folder {} cannot be deleted: it contains items", folder_id)),
                                    Err(e) => Err(e)
                                }
                            } else {
                                Err(format!("ID {} is not a folder", folder_id))
                            }
                        } else {
                            Err("No workspace available".to_string())
                        }
                    }
                    Err(e) => Err(e)
                }
            }
            WorkspaceSidebarEvent::CreateService(name, parent_id) => {
                if name.trim().is_empty() {
                    Err("Service name cannot be empty".to_string())
                } else if let Some(parent_id) = parent_id {
                    match self.validate_project_id(*parent_id) {
                        Ok(_) => {
                            // Ensure parent is a service folder
                            if let Some(workspace) = &self.workspace {
                                if let Some(folder) = workspace.folders.iter().find(|f| f.id == *parent_id) {
                                    if matches!(folder.folder_type, FolderType::Service) {
                                        Ok(())
                                    } else {
                                        Err(format!("Parent folder {} is not a service folder", parent_id))
                                    }
                                } else {
                                    Err(format!("Parent ID {} is not a folder", parent_id))
                                }
                            } else {
                                Err("No workspace available".to_string())
                            }
                        }
                        Err(e) => Err(e)
                    }
                } else {
                    Ok(()) // No parent is valid
                }
            }
            WorkspaceSidebarEvent::CreateSchema(name, parent_id) => {
                if name.trim().is_empty() {
                    Err("Schema name cannot be empty".to_string())
                } else if let Some(parent_id) = parent_id {
                    match self.validate_project_id(*parent_id) {
                        Ok(_) => {
                            // Ensure parent is a schema folder
                            if let Some(workspace) = &self.workspace {
                                if let Some(folder) = workspace.folders.iter().find(|f| f.id == *parent_id) {
                                    if matches!(folder.folder_type, FolderType::Schema) {
                                        Ok(())
                                    } else {
                                        Err(format!("Parent folder {} is not a schema folder", parent_id))
                                    }
                                } else {
                                    Err(format!("Parent ID {} is not a folder", parent_id))
                                }
                            } else {
                                Err("No workspace available".to_string())
                            }
                        }
                        Err(e) => Err(e)
                    }
                } else {
                    Ok(()) // No parent is valid
                }
            }
            WorkspaceSidebarEvent::CreateFolder(name, parent_id, _folder_type) => {
                if name.trim().is_empty() {
                    Err("Folder name cannot be empty".to_string())
                } else if let Some(parent_id) = parent_id {
                    self.validate_project_id(*parent_id)
                } else {
                    Ok(()) // No parent is valid
                }
            }
            _ => Ok(()), // Other events are always valid
        };

        match validation_result {
            Ok(_) => {
                cx.emit(event);
            }
            Err(error) => {
                eprintln!("Warning: Invalid menu operation attempted: {:?}. Error: {}", event, error);
                // Optionally, you could emit an error event or show a user notification here
            }
        }
    }

    /// Build tree items for services section
    fn build_service_tree_items(&self, parent_id: Option<Uuid>, depth: usize, active_service: Option<Uuid>) -> Vec<TreeItem> {
        let Some(workspace) = &self.workspace else { return vec![] };
        
        // Validate and limit depth to prevent overflow
        let safe_depth = match self.validate_tree_depth(depth) {
            Ok(d) => d,
            Err(_) => return vec![], // Stop recursion if depth is too deep
        };

        let mut items = Vec::new();

        // Add folders first
        for folder in workspace.folders.iter()
            .filter(|f| f.parent_id == parent_id && matches!(f.folder_type, FolderType::Service)) {
            
            // Skip if we've reached maximum depth
            if safe_depth >= MAX_TREE_DEPTH {
                eprintln!("Warning: Skipping folder {} due to maximum depth limit", folder.name);
                continue;
            }

            let is_expanded = self.expanded_folders.contains(&folder.id);
            let item = TreeItem::folder(folder.clone(), safe_depth, is_expanded);
            
            items.push(item);
            
            // Add children if expanded and not at max depth
            if is_expanded && safe_depth < MAX_TREE_DEPTH {
                items.extend(self.build_service_tree_items(Some(folder.id), safe_depth + 1, active_service));
            }
        }

        // Add services
        for service in workspace.services.iter()
            .filter(|s| s.folder_id == parent_id) {
            let is_active = active_service == Some(service.id);
            let item = TreeItem::service(service.clone(), safe_depth, is_active);
            
            items.push(item);
        }

        items
    }

    /// Build tree items for schemas section
    fn build_schema_tree_items(&self, parent_id: Option<Uuid>, depth: usize) -> Vec<TreeItem> {
        let Some(workspace) = &self.workspace else { return vec![] };
        
        // Validate and limit depth to prevent overflow
        let safe_depth = match self.validate_tree_depth(depth) {
            Ok(d) => d,
            Err(_) => return vec![], // Stop recursion if depth is too deep
        };

        let mut items = Vec::new();

        // Add folders first
        for folder in workspace.folders.iter()
            .filter(|f| f.parent_id == parent_id && matches!(f.folder_type, FolderType::Schema)) {
            
            // Skip if we've reached maximum depth
            if safe_depth >= MAX_TREE_DEPTH {
                eprintln!("Warning: Skipping folder {} due to maximum depth limit", folder.name);
                continue;
            }

            let is_expanded = self.expanded_folders.contains(&folder.id);
            let item = TreeItem::folder(folder.clone(), safe_depth, is_expanded);
            
            items.push(item);
            
            // Add children if expanded and not at max depth
            if is_expanded && safe_depth < MAX_TREE_DEPTH {
                items.extend(self.build_schema_tree_items(Some(folder.id), safe_depth + 1));
            }
        }

        // Add schemas
        for schema in workspace.schemas.iter()
            .filter(|s| s.folder_id == parent_id) {
            let item = TreeItem::schema(schema.clone(), safe_depth);
            
            items.push(item);
        }

        items
    }

    /// Render a tree item with proper indentation and styling
    fn render_tree_item(&self, item: &TreeItem, cx: &mut Context<Self>) -> impl IntoElement {
        let item_id = item.id();
        let indentation = item.calculate_indentation();
        let item_name = item.name().to_string();
        let item_type = &item.item_type;

        // Calculate responsive layout values
        let max_name_width = self.calculate_max_item_name_width(item.depth);
        let truncated_name = self.truncate_text_with_ellipsis(&item_name, max_name_width);
        let is_minimum_width = self.is_at_minimum_width();
        let minimum_width_for_depth = self.get_minimum_width_for_depth(item.depth);

        // Get styling based on interaction state
        let background_color = self.get_item_background_color(item_id, item_type);
        let border_color = self.get_item_border_color(item_id, item_type);
        let (_menu_class, _menu_color) = self.get_menu_button_style(item_type);

        // Adjust layout for deep indentation or narrow sidebar
        let should_compact_layout = is_minimum_width || self.sidebar_width < minimum_width_for_depth;
        let adjusted_indentation = if should_compact_layout {
            // Reduce indentation for deep levels when space is limited
            (item.depth.min(3) as f32 * 12.0).min(indentation)
        } else {
            indentation
        };

        // Create the main content container with enhanced interaction handling
        let mut content_container = div()
            .h_flex()
            .items_center()
            .justify_between()
            .w_full()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(background_color)
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                this.set_hovered_item(Some(item_id));
                cx.notify(); // Trigger re-render to show menu
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, _cx| {
                this.set_pressed_item(Some(item_id));
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(move |this, _, _, _cx| {
                this.set_pressed_item(None);
            }));

        // Add border if item is active
        if let Some(border) = border_color {
            content_container = content_container.border_1().border_color(border);
        }

        match &item.item_type {
            TreeItemType::Folder(folder) => {
                let folder_id = folder.id;
                let icon = if item.is_expanded { IconName::ChevronDown } else { IconName::ChevronRight };

                content_container
                    .child(
                        Button::new(format!("folder_{}", folder_id))
                            .w_full()
                            .justify_start()
                            .ghost() // Use ghost style for consistent appearance
                            .child(
                                div()
                                    .h_flex()
                                    .items_center()
                                    .gap_1()
                                    .pl(px(adjusted_indentation))
                                    .child(
                                        gpui_component::Icon::new(icon)
                                            .size_4()
                                            .when(self.is_item_active(item_id), |icon| {
                                                icon.text_color(rgb(0x2196f3))
                                            })
                                    )
                                    .child(
                                        gpui_component::Icon::new(IconName::Folder)
                                            .size_4()
                                            .text_color(rgb(0xffc107)) // Folder icon always yellow
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .overflow_hidden()
                                            .flex_shrink()
                                            .max_w(px(max_name_width))
                                            .when(self.is_item_active(item_id), |div| {
                                                div.font_semibold().text_color(rgb(0x2196f3))
                                            })
                                            .child(truncated_name.clone())
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.set_active_item(Some(folder_id));
                                cx.emit(WorkspaceSidebarEvent::ToggleFolder(folder_id));
                            }))
                    )
                    .child(
                        // Menu button container - always reserve space to ensure visibility
                        div()
                            .w(px(32.0)) // Fixed width for menu button area
                            .h_flex()
                            .justify_end()
                            .items_center()
                            .when(self.is_item_hovered(item_id), |container| {
                                container.child(self.safe_render_context_menu(item, cx))
                            })
                    )
            }
            TreeItemType::Service(service) => {
                let service_id = service.id;
                let method_color = match &service.service_type {
                    ServiceType::Http(config) => match config.method {
                        models::HttpMethod::Get => rgb(0x28a745),
                        models::HttpMethod::Post => rgb(0x007bff),
                        models::HttpMethod::Put => rgb(0xffc107),
                        models::HttpMethod::Delete => rgb(0xdc3545),
                        _ => rgb(0x6c757d),
                    },
                    ServiceType::Grpc(_) => rgb(0x6f42c1),
                    ServiceType::WebSocket(_) => rgb(0x17a2b8),
                };

                let method_label = match &service.service_type {
                    ServiceType::Http(config) => config.method.as_str(),
                    ServiceType::Grpc(_) => "gRPC",
                    ServiceType::WebSocket(_) => "WS",
                };

                content_container
                    .child(
                        Button::new(format!("service_{}", service_id))
                            .w_full()
                            .justify_start()
                            .ghost()
                            .when(item.is_active || self.is_item_active(item_id), |btn: Button| {
                                btn.primary()
                            })
                            .child(
                                div()
                                    .h_flex()
                                    .items_center()
                                    .gap_2()
                                    .pl(px(adjusted_indentation))
                                    .child(
                                        div()
                                            .text_xs()
                                            .px_1()
                                            .rounded(px(2.0))
                                            .bg(method_color)
                                            .text_color(rgb(0xffffff))
                                            .font_medium()
                                            .when(should_compact_layout, |div| {
                                                // Use shorter labels in compact mode
                                                div.min_w(px(20.0))
                                            })
                                            .child(if should_compact_layout && method_label.len() > 3 {
                                                &method_label[..3.min(method_label.len())]
                                            } else {
                                                method_label
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .overflow_hidden()
                                            .flex_shrink()
                                            .max_w(px(max_name_width))
                                            .when(self.is_item_active(item_id), |div| {
                                                div.font_semibold().text_color(rgb(0x4caf50))
                                            })
                                            .child(truncated_name.clone())
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.set_active_item(Some(service_id));
                                cx.emit(WorkspaceSidebarEvent::OpenService(service_id));
                            }))
                    )
                    .child(
                        // Menu button container - always reserve space to ensure visibility
                        div()
                            .w(px(32.0)) // Fixed width for menu button area
                            .h_flex()
                            .justify_end()
                            .items_center()
                            .when(self.is_item_hovered(item_id), |container| {
                                container.child(self.safe_render_context_menu(item, cx))
                            })
                    )
            }
            TreeItemType::Schema(schema) => {
                let schema_id = schema.id;

                content_container
                    .child(
                        Button::new(format!("schema_{}", schema_id))
                            .w_full()
                            .justify_start()
                            .ghost()
                            .when(self.is_item_active(item_id), |btn: Button| {
                                btn.primary()
                            })
                            .child(
                                div()
                                    .h_flex()
                                    .items_center()
                                    .gap_2()
                                    .pl(px(adjusted_indentation))
                                    .child(
                                        gpui_component::Icon::new(IconName::File)
                                            .size_4()
                                            .text_color(rgb(0x6f42c1))
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .overflow_hidden()
                                            .flex_shrink()
                                            .max_w(px(max_name_width))
                                            .when(self.is_item_active(item_id), |div| {
                                                div.font_semibold().text_color(rgb(0x9c27b0))
                                            })
                                            .child(truncated_name.clone())
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.set_active_item(Some(schema_id));
                                cx.emit(WorkspaceSidebarEvent::OpenSchema(schema_id));
                            }))
                    )
                    .child(
                        // Menu button container - always reserve space to ensure visibility
                        div()
                            .w(px(32.0)) // Fixed width for menu button area
                            .h_flex()
                            .justify_end()
                            .items_center()
                            .when(self.is_item_hovered(item_id), |container| {
                                container.child(self.safe_render_context_menu(item, cx))
                            })
                    )
            }
        }
    }

    fn open_create_service_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Service name"));

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input_clone = input.clone();
            let this_clone = this.clone();

            dialog
                .title("Create Service")
                .w(px(400.0))
                .confirm()
                .child(Input::new(&input).w_full())
                .on_ok(move |_, window, cx| {
                    let name = input_clone.read(cx).value().to_string();
                    if !name.trim().is_empty() {
                        if let Some(this) = this_clone.upgrade() {
                            this.update(cx, |_, cx| {
                                cx.emit(WorkspaceSidebarEvent::CreateService(name, None));
                            });
                        }
                    }
                    window.close_dialog(cx);
                    true
                })
        });
    }

    fn open_create_schema_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Schema name"));

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input_clone = input.clone();
            let this_clone = this.clone();

            dialog
                .title("Create Schema")
                .w(px(400.0))
                .confirm()
                .child(Input::new(&input).w_full())
                .on_ok(move |_, window, cx| {
                    let name = input_clone.read(cx).value().to_string();
                    if !name.trim().is_empty() {
                        if let Some(this) = this_clone.upgrade() {
                            this.update(cx, |_, cx| {
                                cx.emit(WorkspaceSidebarEvent::CreateSchema(name, None));
                            });
                        }
                    }
                    window.close_dialog(cx);
                    true
                })
        });
    }

    fn open_create_folder_dialog(&mut self, folder_type: FolderType, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Folder name"));

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input_clone = input.clone();
            let this_clone = this.clone();
            let ft = folder_type.clone();

            dialog
                .title("Create Folder")
                .w(px(400.0))
                .confirm()
                .child(Input::new(&input).w_full())
                .on_ok(move |_, window, cx| {
                    let name = input_clone.read(cx).value().to_string();
                    if !name.trim().is_empty() {
                        if let Some(this) = this_clone.upgrade() {
                            this.update(cx, |_, cx| {
                                cx.emit(WorkspaceSidebarEvent::CreateFolder(name, None, ft.clone()));
                            });
                        }
                    }
                    window.close_dialog(cx);
                    true
                })
        });
    }

    fn open_import_openapi_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _this = cx.entity().downgrade();
        let import_dialog = cx.new(|cx| ImportDialog::new(window, cx));
        
        // Set available folders for the dialog
        if let Some(workspace) = &self.workspace {
            let service_folders: Vec<Folder> = workspace.folders
                .iter()
                .filter(|f| f.folder_type == FolderType::Service)
                .cloned()
                .collect();
            
            import_dialog.update(cx, |dialog, cx| {
                dialog.set_folders(service_folders, cx);
            });
        }

        // Subscribe to import dialog events
        let import_dialog_clone = import_dialog.clone();
        cx.subscribe(&import_dialog, move |_this, _dialog, event, cx| {
            match event {
                ImportDialogEvent::Import(config) => {
                    cx.emit(WorkspaceSidebarEvent::ImportOpenApi(config.clone()));
                }
                ImportDialogEvent::RequestPreview(config) => {
                    cx.emit(WorkspaceSidebarEvent::RequestImportPreview(config.clone()));
                }
                ImportDialogEvent::ResolveConflicts(config, resolutions) => {
                    cx.emit(WorkspaceSidebarEvent::ResolveImportConflicts(config.clone(), resolutions.clone()));
                }
                ImportDialogEvent::Cancel => {
                    // Dialog will be closed automatically
                }
            }
        }).detach();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .title("Import OpenAPI")
                .w(px(700.0))
                .max_h(px(600.0))
                .child(import_dialog_clone.clone())
                .on_ok(move |_, window, cx| {
                    window.close_dialog(cx);
                    true
                })
        });
    }
}

impl Render for WorkspaceSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(workspace) = &self.workspace else {
            return div().into_any_element();
        };

        let active_service = workspace.active_service;
        let (services_expanded, schemas_expanded) = self.expanded_sections;
        let view = cx.entity();

        // Build tree items using the new structure
        let service_tree_items = if services_expanded {
            self.build_service_tree_items(None, 0, active_service)
        } else {
            vec![]
        };

        let schema_tree_items = if schemas_expanded {
            self.build_schema_tree_items(None, 0)
        } else {
            vec![]
        };

        // Convert tree items to renderable elements
        let service_elements: Vec<AnyElement> = service_tree_items
            .iter()
            .map(|item| self.render_tree_item(item, cx).into_any_element())
            .collect();

        let schema_elements: Vec<AnyElement> = schema_tree_items
            .iter()
            .map(|item| self.render_tree_item(item, cx).into_any_element())
            .collect();

        // Determine responsive sidebar width with validation
        let validated_width = self.validate_sidebar_width(self.sidebar_width);
        let is_minimum_width = validated_width <= MIN_SIDEBAR_WIDTH;

        div()
            // .w(sidebar_width)
            // .min_w(px(MIN_SIDEBAR_WIDTH)) // Ensure minimum width
            // .max_w(px(MAX_SIDEBAR_WIDTH)) // Prevent excessive width
            .h_full()
            .bg(rgb(0xf8f9fa))
            .border_r_1()
            .border_color(rgb(0xe0e0e0))
            .v_flex()
            .on_mouse_move(cx.listener(|this, _event, _, cx| {
                // Clear hovered item if mouse is not over any specific item
                // This helps ensure menu disappears when mouse moves to empty areas
                if this.hovered_item.is_some() {
                    // Only clear if we're in an empty area (this is a simple heuristic)
                    // In a more sophisticated implementation, you might track mouse coordinates
                    cx.notify();
                }
            }))
            .child(
                div()
                    .p_2()
                    .child(
                        div()
                            .p_2()
                            .bg(rgb(0xffffff))
                            .border_1()
                            .border_color(rgb(0xdee2e6))
                            .rounded_md()
                            .text_sm()
                            .text_color(rgb(0x999999))
                            .overflow_hidden()
                            .child(if is_minimum_width { "Search" } else { "Search..." }),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .overflow_scrollbar()
                    .p_2()
                    .v_flex()
                    .gap_2()
                    // Services Section
                    .child(
                        div()
                            .v_flex()
                            .child(
                                div()
                                    .h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .id("services_header")
                                            .h_flex()
                                            .items_center()
                                            .gap_1()
                                            .cursor_pointer()
                                            .child(gpui_component::Icon::new(if services_expanded { IconName::ChevronDown } else { IconName::ChevronRight }).size_4())
                                            .child(div().text_sm().font_semibold().child("Services"))
                                            .on_click(cx.listener(|this, _, _, _| {
                                                this.expanded_sections.0 = !this.expanded_sections.0;
                                            })),
                                    )
                                    .child(
                                        Button::new("add_service_btn")
                                            .ghost()
                                            .compact()
                                            .icon(IconName::Plus)
                                            .dropdown_menu({
                                                let view = view.clone();
                                                move |menu, window, _cx| {
                                                    menu.item(PopupMenuItem::new("Add Service").on_click(
                                                        window.listener_for(&view, |this, _, window, cx| {
                                                            this.open_create_service_dialog(window, cx);
                                                        }),
                                                    ))
                                                    .item(PopupMenuItem::new("Add Folder").on_click(
                                                        window.listener_for(&view, |this, _, window, cx| {
                                                            this.open_create_folder_dialog(FolderType::Service, window, cx);
                                                        }),
                                                    ))
                                                    .separator()
                                                    .item(PopupMenuItem::new("Import OpenAPI").on_click(
                                                        window.listener_for(&view, |this, _, window, cx| {
                                                            this.open_import_openapi_dialog(window, cx);
                                                        }),
                                                    ))
                                                }
                                            }),
                                    ),
                            )
                            .when(services_expanded, |el| el.child(div().v_flex().gap_px().children(service_elements))),
                    )
                    // Schemas Section
                    .child(
                        div()
                            .v_flex()
                            .child(
                                div()
                                    .h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .id("schemas_header")
                                            .h_flex()
                                            .items_center()
                                            .gap_1()
                                            .cursor_pointer()
                                            .child(gpui_component::Icon::new(if schemas_expanded { IconName::ChevronDown } else { IconName::ChevronRight }).size_4())
                                            .child(div().text_sm().font_semibold().child("Schemas"))
                                            .on_click(cx.listener(|this, _, _, _| {
                                                this.expanded_sections.1 = !this.expanded_sections.1;
                                            })),
                                    )
                                    .child(
                                        Button::new("add_schema_btn")
                                            .ghost()
                                            .compact()
                                            .icon(IconName::Plus)
                                            .dropdown_menu({
                                                let view = view.clone();
                                                move |menu, window, _cx| {
                                                    menu.item(PopupMenuItem::new("Add Schema").on_click(
                                                        window.listener_for(&view, |this, _, window, cx| {
                                                            this.open_create_schema_dialog(window, cx);
                                                        }),
                                                    ))
                                                    .item(PopupMenuItem::new("Add Folder").on_click(
                                                        window.listener_for(&view, |this, _, window, cx| {
                                                            this.open_create_folder_dialog(FolderType::Schema, window, cx);
                                                        }),
                                                    ))
                                                }
                                            }),
                                    ),
                            )
                            .when(schemas_expanded, |el| el.child(div().v_flex().gap_px().children(schema_elements))),
                    ),
            )
            .into_any_element()
    }
}
