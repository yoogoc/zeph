use crate::utils::cal_icon;
use gpui::*;
use gpui_component::button::{Button, ButtonRounded};
use gpui_component::input::{Input, InputState};
use gpui_component::tab::{Tab, TabBar, TabVariant};
use gpui_component::{IconName, Sizable};
use models::ContentType;
use workspace::WorkspaceRegistry;

pub enum ServiceTabsEvent {
    Select(ContentType),
    Close(ContentType),
    Rename(ContentType, SharedString),
}

pub struct ServiceTabs {
    editing_tab: Option<ContentType>,
    edit_input: Option<Entity<InputState>>,
    _subscription: Subscription,
}

impl EventEmitter<ServiceTabsEvent> for ServiceTabs {}

impl ServiceTabs {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // Subscribe to WorkspaceRegistry to auto-refresh when workspace changes
        let registry = WorkspaceRegistry::global(cx);
        let subscription = cx.subscribe(
            &registry,
            |_this, _registry, _event: &workspace::WorkspaceEvent, cx| {
                cx.notify();
            },
        );

        Self {
            editing_tab: None,
            edit_input: None,
            _subscription: subscription,
        }
    }

    /// Start editing a tab title
    /// Implements Requirement 1.6: Click focused tab to enter edit mode
    fn start_editing(
        &mut self,
        content_type: ContentType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current_name = content_type.name();

        // Create input field with current name
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("请输入新名称"));

        // Set the initial value
        let name_string = current_name.to_string();
        input.update(cx, |state, cx| {
            state.set_value(name_string, window, cx);
        });

        self.editing_tab = Some(content_type);
        self.edit_input = Some(input);
        cx.notify();
    }

    /// Finish editing and save the new name
    /// Implements Requirement 1.7: Enter key saves new name
    fn finish_editing(&mut self, cx: &mut Context<Self>) {
        if let (Some(content_type), Some(input)) = (self.editing_tab.take(), self.edit_input.take())
        {
            let new_name = input.read(cx).value();

            // Validate the name (not empty, not just whitespace)
            let trimmed = new_name.trim();
            if !trimmed.is_empty() && trimmed.len() <= 255 {
                // Emit rename event
                cx.emit(ServiceTabsEvent::Rename(content_type, new_name.clone()));
            }

            // Clear editing state
            self.editing_tab = None;
            self.edit_input = None;
            cx.notify();
        }
    }

    /// Cancel editing and restore original name
    /// Implements Requirement 1.8: Escape key cancels editing
    fn cancel_editing(&mut self, cx: &mut Context<Self>) {
        self.editing_tab = None;
        self.edit_input = None;
        cx.notify();
    }
}

impl Render for ServiceTabs {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let registry = WorkspaceRegistry::global(cx);
        let Some(workspace) = registry.read(cx).get_active_workspace() else {
            return div().into_any_element();
        };

        if workspace.opened_services.is_empty() {
            return div().into_any_element();
        }

        let service_ids: Vec<_> = workspace.opened_services.clone();
        let active_service = workspace.active_service.clone();
        let selected_index = active_service
            .as_ref()
            .and_then(|active| service_ids.iter().position(|s| s.eq(active)))
            .unwrap_or(0);

        let tabs: Vec<_> = workspace
            .opened_services
            .iter()
            .enumerate()
            .map(|(index, ct)| {
                let ct = ct.clone();
                let _is_selected = index == selected_index;
                let is_editing = self
                    .editing_tab
                    .as_ref()
                    .map_or(false, |editing| editing.eq(&ct));

                // If this tab is being edited, show input instead of label
                if is_editing {
                    if let Some(input) = &self.edit_input {
                        Tab::new()
                            .px_1()
                            .prefix(cal_icon(&ct))
                            .child(
                                div()
                                    .on_key_down(cx.listener(
                                        move |this, event: &KeyDownEvent, _, cx| {
                                            if event.keystroke.key == "enter" {
                                                this.finish_editing(cx);
                                            } else if event.keystroke.key == "escape" {
                                                this.cancel_editing(cx);
                                            }
                                        },
                                    ))
                                    .child(Input::new(input).w(px(150.0))),
                            )
                            .suffix(
                                Button::new(format!("close_{}", ct.id()))
                                    .xsmall()
                                    .rounded(ButtonRounded::Large)
                                    .icon(IconName::Close)
                                    .on_click(cx.listener(move |_this, _, _, cx| {
                                        cx.emit(ServiceTabsEvent::Close(ct.clone()));
                                        cx.stop_propagation();
                                    })),
                            )
                    } else {
                        // Fallback if input is missing
                        Tab::new()
                            .px_1()
                            .label(ct.name())
                            .prefix(cal_icon(&ct))
                            .suffix(
                                Button::new(format!("close_{}", ct.id()))
                                    .xsmall()
                                    .rounded(ButtonRounded::Large)
                                    .icon(IconName::Close)
                                    .on_click(cx.listener(move |_this, _, _, cx| {
                                        cx.emit(ServiceTabsEvent::Close(ct.clone()));
                                        cx.stop_propagation();
                                    })),
                            )
                    }
                } else {
                    // Normal tab display
                    Tab::new()
                        .px_1()
                        .label(ct.name())
                        .prefix(cal_icon(&ct))
                        .suffix(
                            Button::new(format!("close_{}", ct.id()))
                                .xsmall()
                                .rounded(ButtonRounded::Large)
                                .icon(IconName::Close)
                                .on_click(cx.listener(move |_this, _, _, cx| {
                                    cx.emit(ServiceTabsEvent::Close(ct.clone()));
                                    cx.stop_propagation();
                                })),
                        )
                }
            })
            .collect();

        div()
            .size_full()
            .pl_1()
            .child(
                TabBar::new("service_tabs")
                    .selected_index(selected_index)
                    .with_variant(TabVariant::Pill)
                    .on_click(cx.listener(move |this, index: &usize, window, cx| {
                        if let Some(ct) = service_ids.get(*index) {
                            // Check if clicking on already selected tab
                            let registry = WorkspaceRegistry::global(cx);
                            let is_already_selected = registry
                                .read(cx)
                                .get_active_workspace()
                                .and_then(|ws| ws.active_service.as_ref())
                                .map_or(false, |active| active.eq(ct));

                            if is_already_selected && this.editing_tab.is_none() {
                                // Start editing if clicking on already focused tab
                                // Implements Requirement 1.6
                                this.start_editing(ct.clone(), window, cx);
                            } else {
                                // Normal selection
                                cx.emit(ServiceTabsEvent::Select(ct.clone()));
                            }
                        }
                    }))
                    .children(tabs),
            )
            .into_any_element()
    }
}
