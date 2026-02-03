//! Import handling module for WorkspaceContent
//!
//! This module encapsulates the import dialog state and related logic,
//! reducing the complexity of WorkspaceContent.

use crate::simple_import_dialog::{ImportErrorType, SimpleImportConfig, SimpleImportDialog};
use gpui::*;
use models::FolderType;
use uuid::Uuid;

/// Manages the import dialog state and lifecycle
pub struct ImportHandler {
    /// The import dialog entity
    dialog: Option<Entity<SimpleImportDialog>>,
    /// Whether the import dialog is visible
    visible: bool,
}

impl Default for ImportHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ImportHandler {
    /// Creates a new ImportHandler
    pub fn new() -> Self {
        Self {
            dialog: None,
            visible: false,
        }
    }

    /// Returns whether the import dialog is visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Returns a reference to the dialog entity if it exists
    pub fn dialog(&self) -> Option<&Entity<SimpleImportDialog>> {
        self.dialog.as_ref()
    }

    /// Shows the import dialog with the given entity
    pub fn show(&mut self, dialog: Entity<SimpleImportDialog>) {
        self.dialog = Some(dialog);
        self.visible = true;
    }

    /// Hides and clears the import dialog
    pub fn hide(&mut self) {
        self.dialog = None;
        self.visible = false;
    }

    /// Sets an error on the dialog if it exists
    pub fn set_error(&self, error: ImportErrorType, cx: &mut App) {
        if let Some(dialog) = &self.dialog {
            dialog.update(cx, |d, _cx| {
                d.set_error(Some(error));
            });
        }
    }
}

/// Validates the target folder for import
/// Returns Ok(()) if valid, or an ImportErrorType if invalid
pub fn validate_target_folder(
    folder_id: Uuid,
    registry: &workspace::WorkspaceRegistry,
) -> Result<(), ImportErrorType> {
    match registry.get_folder(folder_id) {
        Some(folder) => {
            // Validate folder type - must be a service folder
            if folder.folder_type != FolderType::Service {
                Err(ImportErrorType::ValidationError(
                    "目标文件夹必须是服务类型文件夹".to_string(),
                ))
            } else {
                Ok(())
            }
        }
        None => Err(ImportErrorType::ImportError("目标文件夹不存在".to_string())),
    }
}

/// Executes the OpenAPI import with the given configuration
/// This function should be called within a registry.update() closure
/// Returns Ok(()) on success, or an error message on failure
pub fn execute_import<'a>(
    config: &SimpleImportConfig,
    registry: &mut workspace::WorkspaceRegistry,
    cx: &mut Context<'a, workspace::WorkspaceRegistry>,
) -> Result<(), String> {
    // Validate content before import
    if let Err(e) = workspace::validate_openapi_content(&config.content) {
        return Err(e.to_string());
    }

    // Validate target folder if specified
    if let Some(target_folder_id) = config.target_folder {
        if let Err(e) = validate_target_folder(target_folder_id, registry) {
            return Err(e.message().to_string());
        }
    }

    let options = workspace::ImportOptions {
        target_folder: config.target_folder,
        create_subfolders: config.create_subfolders,
        schema_folder_strategy: workspace::SchemaFolderStrategy::SeparateFolder,
    };

    registry
        .import_openapi_with_options(&config.content, options, cx)
        .map_err(|e| e.to_string())
}
