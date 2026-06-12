//! Service content renderer for WorkspaceContent.
//!
//! Renders the view for the currently active content. Each opened content has a
//! cached view in `ServiceState` (see `ContentView` / `ContentViewHandle`), so
//! this component just renders the active one — mirroring how `WorkspaceArea`
//! renders `tabs[active_idx].view`.

use crate::state::ServiceState;
use gpui::*;
use gpui_component::h_flex;

/// Renders the main content area for the currently active service/content.
pub struct ServiceContent {
    service_state: Entity<ServiceState>,
}

impl ServiceContent {
    pub fn new(service_state: Entity<ServiceState>) -> Self {
        Self { service_state }
    }
}

impl Render for ServiceContent {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.service_state.read(cx).active_view() {
            Some(view) => view.into_any_element(),
            None => h_flex()
                .size_full()
                .p_4()
                .items_center()
                .justify_center()
                .child("Select a service to get started")
                .into_any_element(),
        }
    }
}
