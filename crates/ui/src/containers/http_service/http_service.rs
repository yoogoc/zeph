//! HttpService: thin `ContentView` shell around `HttpState`.
//!
//! Owns the request/response views and forwards `ContentView` lifecycle calls
//! to the `HttpState` entity, which holds all request state and logic.

use super::http_state::HttpState;
use super::request_panel::RequestPanel;
use crate::Badge;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::resizable::{resizable_panel, v_resizable};
use gpui_component::spinner::Spinner;
use gpui_component::{v_flex, Sizable};
use uuid::Uuid;

pub struct HttpService {
    /// Snapshot of the service this view renders; used for tab title/icon.
    service: models::Service,
    http_state: Entity<HttpState>,
    request_panel: Entity<RequestPanel>,
}

impl HttpService {
    pub fn new(service: models::Service, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let http_state = cx.new(|cx| HttpState::new(service.clone(), window, cx));
        let request_panel = cx.new(|_| RequestPanel::new(http_state.clone()));

        Self {
            service,
            http_state,
            request_panel,
        }
    }
}

impl Render for HttpService {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.http_state.read(cx);
        let can_undo = state.can_undo(cx);
        let can_redo = state.can_redo(cx);
        let is_loading = state.is_loading;
        let response_panel = state.response_panel.clone();

        div()
            .size_full()
            .py_2()
            .px_4()
            .gap_4()
            .relative()
            // Keyboard shortcuts for Undo/Redo
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                let modifier =
                    event.keystroke.modifiers.platform || event.keystroke.modifiers.control;
                if !modifier || event.keystroke.key != "z" {
                    return;
                }
                if event.keystroke.modifiers.shift {
                    if can_redo {
                        this.http_state.update(cx, |s, cx| s.redo(cx));
                    }
                } else if can_undo {
                    this.http_state.update(cx, |s, cx| s.undo(cx));
                }
            }))
            .child(
                v_flex().size_full().gap_2().child(
                    v_resizable("http-service")
                        .child(self.request_panel.clone().into_any_element())
                        .child(
                            resizable_panel().child(div().pt_2().size_full().child(response_panel)),
                        ),
                ),
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
