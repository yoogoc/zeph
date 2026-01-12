use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{IconName, StyledExt};
use models::{Folder, FolderType};
use std::collections::HashSet;
use uuid::Uuid;

// Performance optimization: Use faster hasher for folder operations
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum FolderSelectorEvent {
    SelectFolder(Option<Uuid>),
    ToggleFolder(Uuid),
}

pub struct FolderSelector {
    folders: Vec<Folder>,
    selected_folder: Option<Uuid>,
    expanded_folders: HashSet<Uuid>,
    folder_type_filter: FolderType,

    // Performance optimization: Pre-computed folder hierarchy for faster rendering
    folder_hierarchy: HashMap<Option<Uuid>, Vec<Uuid>>,
    folder_lookup: HashMap<Uuid, usize>, // Maps folder ID to index in folders vec
}

impl EventEmitter<FolderSelectorEvent> for FolderSelector {}

impl FolderSelector {
    pub fn new(folder_type_filter: FolderType) -> Self {
        Self {
            folders: Vec::new(),
            selected_folder: None,
            expanded_folders: HashSet::new(),
            folder_type_filter,
            folder_hierarchy: HashMap::new(),
            folder_lookup: HashMap::new(),
        }
    }

    pub fn set_folders(&mut self, folders: Vec<Folder>) {
        // Filter folders by type and expand root folders by default
        self.folders = folders
            .into_iter()
            .filter(|f| f.folder_type == self.folder_type_filter)
            .collect();

        // Performance optimization: Pre-compute folder hierarchy and lookup maps
        self.rebuild_hierarchy();

        // Expand root folders by default
        for folder in &self.folders {
            if folder.parent_id.is_none() {
                self.expanded_folders.insert(folder.id);
            }
        }
    }

    /// Performance optimization: Pre-compute folder hierarchy for faster rendering
    fn rebuild_hierarchy(&mut self) {
        self.folder_hierarchy.clear();
        self.folder_lookup.clear();

        // Build lookup map for O(1) folder access
        for (index, folder) in self.folders.iter().enumerate() {
            self.folder_lookup.insert(folder.id, index);
        }

        // Build hierarchy map for O(1) children lookup
        for folder in &self.folders {
            self.folder_hierarchy
                .entry(folder.parent_id)
                .or_insert_with(Vec::new)
                .push(folder.id);
        }

        // Sort children by name for consistent ordering
        for children in self.folder_hierarchy.values_mut() {
            children.sort_by(|a, b| {
                let folder_a = &self.folders[self.folder_lookup[a]];
                let folder_b = &self.folders[self.folder_lookup[b]];
                folder_a.name.cmp(&folder_b.name)
            });
        }
    }

    pub fn selected_folder(&self) -> Option<Uuid> {
        self.selected_folder
    }

    pub fn set_selected_folder(&mut self, folder_id: Option<Uuid>) {
        self.selected_folder = folder_id;
    }

    fn toggle_folder(&mut self, folder_id: Uuid) {
        if self.expanded_folders.contains(&folder_id) {
            self.expanded_folders.remove(&folder_id);
        } else {
            self.expanded_folders.insert(folder_id);
        }
    }

    /// Performance optimization: Check if folder has children using pre-computed hierarchy
    fn has_children(&self, folder_id: Uuid) -> bool {
        self.folder_hierarchy
            .get(&Some(folder_id))
            .map_or(false, |children| !children.is_empty())
    }

    /// Performance optimization: Get folder by ID using lookup map
    fn get_folder(&self, folder_id: Uuid) -> Option<&Folder> {
        self.folder_lookup
            .get(&folder_id)
            .and_then(|&index| self.folders.get(index))
    }

    fn render_folder_item(
        &self,
        folder_id: Uuid,
        depth: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let folder = match self.get_folder(folder_id) {
            Some(f) => f,
            None => return div().into_any_element(), // Skip invalid folder
        };

        let is_expanded = self.expanded_folders.contains(&folder_id);
        let is_selected = self.selected_folder == Some(folder_id);
        let has_children = self.has_children(folder_id);

        let chevron_icon = if has_children {
            if is_expanded {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            }
        } else {
            IconName::ChevronRight // Placeholder, will be invisible
        };

        div()
            .v_flex()
            .child(
                Button::new(format!("folder_selector_{}", folder_id))
                    .w_full()
                    .justify_start()
                    .when(is_selected, |btn: Button| btn.primary())
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap_1()
                            .pl(px(depth as f32 * 16.0))
                            .child(
                                div()
                                    .w(px(16.0))
                                    .h(px(16.0))
                                    .h_flex()
                                    .items_center()
                                    .justify_center()
                                    .when(has_children, |el| {
                                        el.child(
                                            Button::new(format!("chevron_{}", folder_id))
                                                .ghost()
                                                .compact()
                                                .icon(chevron_icon)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.toggle_folder(folder_id);
                                                    cx.emit(FolderSelectorEvent::ToggleFolder(
                                                        folder_id,
                                                    ));
                                                })),
                                        )
                                    }),
                            )
                            .child(
                                gpui_component::Icon::new(IconName::Folder)
                                    .size_4()
                                    .text_color(rgb(0x6c757d)),
                            )
                            .child(div().text_sm().child(folder.name.clone())),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected_folder = Some(folder_id);
                        cx.emit(FolderSelectorEvent::SelectFolder(Some(folder_id)));
                    })),
            )
            .when(is_expanded && has_children, |el| {
                el.child(div().v_flex().children(self.render_folder_tree_optimized(
                    Some(folder_id),
                    depth + 1,
                    cx,
                )))
            })
            .into_any_element()
    }

    /// Performance optimization: Use pre-computed hierarchy for faster tree rendering
    fn render_folder_tree_optimized(
        &self,
        parent_id: Option<Uuid>,
        depth: usize,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        self.folder_hierarchy
            .get(&parent_id)
            .map(|children| {
                children
                    .iter()
                    .map(|&folder_id| {
                        self.render_folder_item(folder_id, depth, cx)
                            .into_any_element()
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn render_folder_tree(
        &self,
        parent_id: Option<Uuid>,
        depth: usize,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        // Use optimized version
        self.render_folder_tree_optimized(parent_id, depth, cx)
    }
}

impl Render for FolderSelector {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .max_h(px(300.0))
            .overflow_y_hidden()
            .border_1()
            .border_color(rgb(0xdee2e6))
            .rounded_md()
            .bg(rgb(0xffffff))
            .p_2()
            .v_flex()
            .gap_1()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .pb_2()
                    .border_b_1()
                    .border_color(rgb(0xe9ecef))
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .text_color(rgb(0x495057))
                            .child("Select Target Folder"),
                    )
                    .child(
                        Button::new("clear_selection")
                            .ghost()
                            .compact()
                            .child("None")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.selected_folder = None;
                                cx.emit(FolderSelectorEvent::SelectFolder(None));
                            })),
                    ),
            )
            .child(
                div()
                    .v_flex()
                    .gap_px()
                    .when(self.folders.is_empty(), |el| {
                        el.child(
                            div()
                                .p_4()
                                .text_center()
                                .text_sm()
                                .text_color(rgb(0x6c757d))
                                .child("No folders available"),
                        )
                    })
                    .when(!self.folders.is_empty(), |el| {
                        el.children(self.render_folder_tree(None, 0, cx))
                    }),
            )
    }
}
