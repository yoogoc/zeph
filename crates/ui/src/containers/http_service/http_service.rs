//! HttpService: thin `ContentView` shell around `HttpState`.
//!
//! Owns the request/response views and forwards `ContentView` lifecycle calls
//! to the `HttpState` entity, which holds all request state and logic.

use super::design_panel::DesignPanel;
use super::http_state::HttpState;
use super::request_panel::RequestPanel;
use super::response_panel::ResponsePanel;
use crate::Badge;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::resizable::{resizable_panel, v_resizable};
use gpui_component::spinner::Spinner;
use gpui_component::tab::{Tab, TabBar};
use gpui_component::{h_flex, v_flex, Sizable};
use uuid::Uuid;

/// Top-level mode: edit the API contract (Design) vs. fill values and send (Debug).
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum ViewMode {
    #[default]
    Debug,
    Design,
}

pub struct HttpService {
    /// Snapshot of the service this view renders; used for tab title/icon.
    service: models::Service,
    http_state: Entity<HttpState>,
    request_panel: Entity<RequestPanel>,
    response_panel: Entity<ResponsePanel>,
    design_panel: Entity<DesignPanel>,
    mode: ViewMode,
    /// Inline editor for the service name (shown after the mode tabs). `Some`
    /// while editing; committed on Enter/Blur via the input's subscription.
    name_input: Option<Entity<InputState>>,
    _name_subscription: Option<Subscription>,
}

impl HttpService {
    pub fn new(
        service: models::Service,
        service_state: Entity<crate::state::ServiceState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let http_state = cx.new(|cx| HttpState::new(service.clone(), service_state, window, cx));
        let response_panel = cx.new(|cx| ResponsePanel::new(window, cx));
        http_state.update(cx, |s, _| s.set_response_panel(response_panel.downgrade()));
        let request_panel = cx.new(|cx| RequestPanel::new(http_state.clone(), window, cx));
        http_state.update(cx, |s, _| s.set_request_panel(request_panel.downgrade()));
        let design_panel = cx.new(|cx| DesignPanel::new(http_state.clone(), window, cx));

        Self {
            service,
            http_state,
            request_panel,
            response_panel,
            design_panel,
            mode: ViewMode::default(),
            name_input: None,
            _name_subscription: None,
        }
    }

    /// Begins inline editing of the service name.
    fn start_name_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.service.name.clone();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("请输入新名称"));
        input.update(cx, |state, cx| state.set_value(name, window, cx));
        self._name_subscription =
            Some(
                cx.subscribe(&input, |this, _, event: &InputEvent, cx| match event {
                    InputEvent::PressEnter { .. } | InputEvent::Blur => this.finish_name_edit(cx),
                    _ => {}
                }),
            );
        self.name_input = Some(input);
        cx.notify();
    }

    fn finish_name_edit(&mut self, cx: &mut Context<Self>) {
        if let Some(input) = self.name_input.clone() {
            let new_name = input.read(cx).value().clone();
            let trimmed = new_name.trim();
            if !trimmed.is_empty() && trimmed.len() <= 255 {
                let http_state = self.http_state.clone();
                let new_name = new_name.clone();
                // Defer: this runs inside the input subscription (mid-update of
                // HttpService), and `rename` re-enters HttpService via
                // `set_title`. Schedule it after the current update unwinds.
                cx.defer(move |cx| {
                    http_state.update(cx, |s, cx| s.rename(new_name.clone(), cx));
                });
            }
        }
        self.name_input = None;
        self._name_subscription = None;
        cx.notify();
    }
}

impl Render for HttpService {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.http_state.read(cx);
        let can_undo = state.can_undo(cx);
        let can_redo = state.can_redo(cx);
        let is_loading = state.is_loading;
        let response_panel = self.response_panel.clone();
        let mode = self.mode;
        let service_name = self.service.name.clone();

        // Mode switch: Debug (send) vs Design (contract), as a tab bar.
        let mode_tabs = TabBar::new("http-mode-tabs")
            .selected_index(match mode {
                ViewMode::Debug => 0,
                ViewMode::Design => 1,
            })
            .child(Tab::new().label("Debug"))
            .child(Tab::new().label("Design"))
            .on_click(cx.listener(|this, index: &usize, _, cx| {
                this.mode = match index {
                    1 => ViewMode::Design,
                    _ => ViewMode::Debug,
                };
                cx.notify();
            }));

        // Service name after the tabs; click to edit inline.
        let name_el: AnyElement = if let Some(input) = &self.name_input {
            Input::new(input).w(px(200.)).into_any_element()
        } else {
            div()
                .id("service-name")
                .px_2()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .hover(|s| s.opacity(0.7))
                .child(service_name)
                .on_click(cx.listener(|this, _, window, cx| this.start_name_edit(window, cx)))
                .into_any_element()
        };

        div()
            .size_full()
            .py_2()
            .px_4()
            .gap_4()
            .relative()
            // Keyboard shortcuts for Undo/Redo
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                let modifier =
                    event.keystroke.modifiers.platform || event.keystroke.modifiers.control;
                if !modifier || event.keystroke.key != "z" {
                    return;
                }
                if event.keystroke.modifiers.shift {
                    if can_redo {
                        this.http_state.update(cx, |s, cx| s.redo(window, cx));
                    }
                } else if can_undo {
                    this.http_state.update(cx, |s, cx| s.undo(window, cx));
                }
            }))
            .child(
                v_flex()
                    .size_full()
                    .gap_2()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(mode_tabs)
                            .child(name_el),
                    )
                    .map(|el| match mode {
                        ViewMode::Debug => el.child(
                            v_resizable("http-service")
                                .child(self.request_panel.clone().into_any_element())
                                .child(
                                    resizable_panel()
                                        .child(div().pt_2().size_full().child(response_panel)),
                                ),
                        ),
                        ViewMode::Design => {
                            el.child(div().flex_1().child(self.design_panel.clone()))
                        }
                    }),
            )
            .when(is_loading, |this| {
                this.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(hsla(0., 0., 1., 0.5))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| cx.stop_propagation())
                        .child(
                            v_flex()
                                .items_center()
                                .justify_center()
                                .gap_2()
                                .child(Spinner::new().large())
                                .child(
                                    // TODO: cancel action
                                    Button::new("cancel-http-service").label("cancel").primary(),
                                ),
                        ),
                )
            })
    }
}

impl crate::state::ContentView for HttpService {
    fn id(&self) -> Uuid {
        self.service.id
    }

    fn title(&self) -> SharedString {
        self.service.name.clone()
    }

    fn tab_icon(&self) -> AnyElement {
        match &self.service.service_type {
            models::ServiceType::Http(config) => {
                Badge::new(config.method).compact(true).into_any_element()
            }
            _ => gpui_component::IconName::Globe.into_any_element(),
        }
    }

    fn set_title(&mut self, title: SharedString, cx: &mut Context<Self>) {
        self.service.name = title;
        cx.notify();
    }

    fn is_modified(&self, cx: &App) -> bool {
        self.http_state.read(cx).is_modified(cx)
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        self.http_state.update(cx, |s, cx| s.save(cx));
    }
}
