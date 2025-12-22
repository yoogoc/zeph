use gpui::*;
use gpui_component::button::Button;
use gpui_component::Sizable;
use gpui_component::tab::{Tab, TabBar};
use models::Collection;
use uuid::Uuid;

pub enum ServiceTabsEvent {
    Select(Uuid),
    Close(Uuid),
}

pub struct ServiceTabs {
    collection: Option<Collection>,
}

impl EventEmitter<ServiceTabsEvent> for ServiceTabs {}

impl ServiceTabs {
    pub fn new() -> Self {
        Self { collection: None }
    }

    pub fn set_collection(&mut self, collection: Collection) {
        self.collection = Some(collection);
    }
}

impl Render for ServiceTabs {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(collection) = &self.collection else {
            return div().into_any_element();
        };

        if collection.opened_services.is_empty() {
            return div().into_any_element();
        }

        let service_ids: Vec<_> = collection.opened_services.clone();
        let selected_index = collection
            .opened_services
            .iter()
            .position(|s| collection.active_service.eq(&Some(*s)))
            .unwrap_or(0);

        let tabs: Vec<_> = collection
            .opened_services
            .iter()
            .filter_map(|&service_id| {
                collection
                    .services
                    .iter()
                    .find(|s| s.id == service_id)
                    .map(|service| {
                        Tab::new().label(service.name.clone()).px_1().suffix(
                            Button::new(format!("close_{}", service_id))
                                .small()
                                .flex_shrink_0()
                                .label("×")
                                .on_click(cx.listener(move |_this, _, _, cx| {
                                    cx.emit(ServiceTabsEvent::Close(service_id));
                                })),
                        )
                    })
            })
            .collect();

        TabBar::new("service_tabs")
            .selected_index(selected_index)
            .on_click(cx.listener(move |_this, index: &usize, _, cx| {
                if selected_index.ne(index) {
                    if let Some(&service_id) = service_ids.get(*index) {
                        cx.emit(ServiceTabsEvent::Select(service_id));
                    }
                }
            }))
            .children(tabs)
            .into_any_element()
    }
}
