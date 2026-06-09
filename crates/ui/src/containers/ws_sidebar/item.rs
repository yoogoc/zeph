use crate::state::ServiceState;
use crate::ws_sidebar::popup_menu::popup_menu;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::Button;
use gpui_component::list::ListItem;
use gpui_component::menu::DropdownMenu;
use gpui_component::tree::TreeEntry;
use gpui_component::{h_flex, IconName, Sizable};

pub fn item(
    entry: &TreeEntry,
    ix: usize,
    selected: bool,
    icon: AnyElement,
    hovered: Option<SharedString>,
    service_state: Entity<ServiceState>,
) -> ListItem {
    // let icon = get_icon(entry); cal_icon(entry, &ss.services, &ss.schemas);
    let item = entry.item();
    let item_id = item.id.clone();
    ListItem::new(ix)
        .selected(selected)
        .pl(px(10.) * entry.depth() + px(12.))
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .gap_1()
                .child(
                    h_flex()
                        .gap_2()
                        .child(icon)
                        .child(item.label.clone())
                        .overflow_hidden(),
                )
                .when_some(hovered.clone(), |this, id| {
                    this.when(id == item_id, |this| {
                        this.child(
                            Button::new(format!("action_{}", id))
                                .icon(IconName::Menu)
                                .small()
                                .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                                    cx.stop_propagation();
                                })
                                .dropdown_menu({
                                    let item = entry.item().clone();
                                    let service_state = service_state.clone();

                                    move |menu, window, cx| {
                                        popup_menu(menu, &item, service_state.clone(), window, cx)
                                    }
                                }),
                        )
                    })
                }),
        )
}
