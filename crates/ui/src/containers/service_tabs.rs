use crate::state::ServiceState;
use gpui::*;
use gpui_component::button::{Button, ButtonRounded};
use gpui_component::menu::DropdownMenu;
use gpui_component::tab::{Tab, TabBar, TabVariant};
use gpui_component::{IconName, Sizable};
use uuid::Uuid;

pub struct ServiceTabs {
    service_state: Entity<ServiceState>,
}

impl ServiceTabs {
    // Re-renders automatically when `service_state` changes: GPUI tracks the
    // entities `read` during `render` and invalidates this view on their `notify`.
    pub fn new(service_state: Entity<ServiceState>) -> Self {
        Self { service_state }
    }
}

impl Render for ServiceTabs {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ss = self.service_state.read(cx);

        // "+" button to open a new untitled request; always available. The
        // dropdown lets the user pick the request type (HTTP / gRPC).
        let new_request_button = Button::new("new_untitled_request")
            .xsmall()
            .rounded(ButtonRounded::Large)
            .icon(IconName::Plus)
            .dropdown_menu({
                let service_state = self.service_state.clone();
                move |menu, _window, _cx| {
                    use gpui_component::menu::PopupMenuItem;
                    let ss_http = service_state.clone();
                    let ss_grpc = service_state.clone();
                    menu.item(
                        PopupMenuItem::new("HTTP Request").on_click(move |_, window, cx| {
                            ss_http.update(cx, |s, cx| {
                                s.create_untitled_request(
                                    models::ServiceType::Http(models::HttpConfig::default()),
                                    None,
                                    window,
                                    cx,
                                );
                            });
                        }),
                    )
                    .item(
                        PopupMenuItem::new("gRPC Request").on_click(move |_, window, cx| {
                            ss_grpc.update(cx, |s, cx| {
                                s.create_untitled_request(
                                    models::ServiceType::Grpc(models::GrpcConfig::default()),
                                    None,
                                    window,
                                    cx,
                                );
                            });
                        }),
                    )
                }
            });

        if ss.open_views().is_empty() {
            return div().pl_1().child(new_request_button).into_any_element();
        }

        let current_active_view_id = ss.active_view_ix;

        // (id, title, icon) for each open tab, sourced from the cached view.
        let tab_data: Vec<(Uuid, SharedString, AnyElement)> = ss
            .open_views()
            .iter()
            .map(|entry| {
                (
                    entry.view.id(cx),
                    entry.view.title(cx),
                    entry.view.tab_icon(cx),
                )
            })
            .collect();

        let close_button = |ix: usize, id: Uuid| {
            Button::new(format!("close_{}", id))
                .xsmall()
                .rounded(ButtonRounded::Large)
                .icon(IconName::Close)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.service_state.update(cx, |s, cx| {
                        s.close_content(ix, cx);
                    });
                    cx.stop_propagation();
                }))
        };

        let tabs: Vec<_> = tab_data
            .into_iter()
            .enumerate()
            .map(|(ix, (id, title, icon))| {
                Tab::new()
                    .px_1()
                    .label(title)
                    .prefix(icon)
                    .suffix(close_button(ix, id))
            })
            .collect();

        div()
            .size_full()
            .pl_1()
            .flex()
            .items_center()
            .child(
                TabBar::new("service_tabs")
                    .selected_index(current_active_view_id.unwrap_or_default())
                    .with_variant(TabVariant::Pill)
                    .on_click(cx.listener({
                        let ss = self.service_state.clone();
                        move |_this, index: &usize, _window, cx| {
                            ss.update(cx, |s, cx| {
                                s.set_active_content(*index, cx);
                            });
                        }
                    }))
                    .children(tabs),
            )
            .child(new_request_button)
            .into_any_element()
    }
}
