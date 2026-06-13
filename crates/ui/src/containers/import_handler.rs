//! Import execution utilities
//!
//! This module provides utility functions for executing OpenAPI imports.
//! The import dialog UI is managed by WorkspaceSidebar.

use crate::simple_import_dialog::{ImportErrorType, SimpleImportConfig};
use crate::state::ServiceState;
use gpui::Context;
use models::FolderType;
use uuid::Uuid;

/// Validates the target folder for import
/// Returns Ok(()) if valid, or an ImportErrorType if invalid
pub fn validate_target_folder(
    folder_id: Uuid,
    service_state: &ServiceState,
) -> Result<(), ImportErrorType> {
    match service_state.get_folder(folder_id) {
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

/// Executes the OpenAPI import with the given configuration.
/// This function should be called within a `service_state.update()` closure.
/// Returns Ok(()) on success, or an error message on failure.
pub fn execute_import(
    config: &SimpleImportConfig,
    service_state: &mut ServiceState,
    cx: &mut Context<ServiceState>,
) -> Result<(), String> {
    // Validate content before import
    if let Err(e) = workspace::validate_openapi_content(&config.content) {
        return Err(e.to_string());
    }

    // Validate target folder if specified
    if let Some(target_folder_id) = config.target_folder {
        if let Err(e) = validate_target_folder(target_folder_id, service_state) {
            return Err(e.message().to_string());
        }
    }

    let options = workspace::ImportOptions {
        target_folder: config.target_folder,
        create_subfolders: config.create_subfolders,
        schema_folder_strategy: workspace::SchemaFolderStrategy::SeparateFolder,
    };

    service_state.import_openapi(&config.content, options, cx)
}
