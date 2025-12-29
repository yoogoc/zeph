mod utils;

use crate::ws_sidebar::utils::cal_icon;
use gpui::*;
use gpui_component::list::ListItem;
use gpui_component::tree::{tree, TreeItem, TreeState};
use gpui_component::{h_flex, v_flex};
use models::Workspace;
use utils::build_tree;

pub struct WorkspaceSidebar {
    tree: Entity<TreeState>,
    selected_item: Option<TreeItem>,
}

impl WorkspaceSidebar {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let tree = cx.new(|cx| TreeState::new(cx));
        Self {
            tree,
            selected_item: None,
        }
    }

    pub fn set_workspace(&mut self, workspace: Workspace, cx: &mut Context<Self>) {
        self.tree.update(cx, |this, cx| {
            this.set_items(build_tree(workspace), cx);
        });
    }

    fn handle_selection(&mut self, item: TreeItem, cx: &mut Context<Self>) {
        self.selected_item = Some(item.clone());
        println!("Selected: {} ({})", item.label, item.id);
        cx.notify();
    }
}

impl Render for WorkspaceSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .h_full()
            .w_48()
            .border_r_1()
            .border_color(rgb(0xe0e0e0))
            .child(
                div()
                    .p_2()
                    .border_1()
                    .rounded_md()
                    .text_sm()
                    .overflow_hidden()
                    .child("Search"),
            )
            .child(
                v_flex()
                    .justify_start()
                    .h_full()
                    .p_2()
                    .gap_2()
                    // Services Section
                    .child(tree(&self.tree, {
                        let view = cx.entity();
                        move |ix, entry, selected, _window, cx| {
                            let item = entry.item();
                            let icon = cal_icon(entry);
                            view.update(cx, |_this, cx| {
                                ListItem::new(ix)
                                    .selected(selected)
                                    .pl(px(16.) * entry.depth() + px(12.)) // Indent based on depth
                                    .child(h_flex().gap_2().child(icon).child(item.label.clone()))
                                    .on_click(cx.listener({
                                        let item = entry.item().clone();
                                        move |this, _, _, cx| {
                                            this.handle_selection(item.clone(), cx);
                                        }
                                    }))
                            })
                        }
                    })),
            )
    }
}
