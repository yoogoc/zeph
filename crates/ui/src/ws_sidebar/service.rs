use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use gpui_component::tree::{TreeItem, TreeState};
use gpui_component::{h_flex, v_flex, Icon, IconName, StyledExt};

pub struct ServiceHeader {

}

impl ServiceHeader {
    pub fn new() -> Self {
        Self {}
    }
}

impl Render for ServiceHeader {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        v_flex()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        h_flex()
                            .id("services_header")
                            .items_center()
                            .gap_1()
                            .cursor_pointer()
                            .child(Icon::new(IconName::ChevronDown).size_4())
                            .child(
                                div().text_sm().font_semibold().child("Services"),
                            )
                            .on_click(cx.listener(|this, _, _, _| {
                                // this.expanded_sections.0 =
                                //     !this.expanded_sections.0;
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
                                    menu.item(
                                        PopupMenuItem::new("Add Service").on_click(
                                            window.listener_for(
                                                &view,
                                                |this, _, window, cx| {
                                                    //     this.open_create_service_dialog(
                                                    //     window, cx,
                                                    // );
                                                },
                                            ),
                                        ),
                                    )
                                        .item(
                                            PopupMenuItem::new("Add Folder").on_click(
                                                window.listener_for(
                                                    &view,
                                                    |this, _, window, cx| {
                                                        // this.open_create_folder_dialog(
                                                        //     FolderType::Service,
                                                        //     window,
                                                        //     cx,
                                                        // );
                                                    },
                                                ),
                                            ),
                                        )
                                        .separator()
                                        .item(
                                            PopupMenuItem::new("Import OpenAPI")
                                                .on_click(window.listener_for(
                                                    &view,
                                                    |this, _, window, cx| {
                                                        //     this.open_import_openapi_dialog(
                                                        //     window, cx,
                                                        // );
                                                    },
                                                )),
                                        )
                                }
                            }),
                    ),
            ).into_any_element()
    }
}