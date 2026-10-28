use super::*;
use crate::structure::{RequestStructure, SchemaNode, Section};

#[derive(Default)]
pub(super) struct StructureUi {
    pub(super) request: Option<Uuid>,
    pub(super) data: RequestStructure,
    section: Section,
    manual_section: bool,
    loading: bool,
    generation: u64,
    nodes: Vec<SchemaNode>,
    expansion: HashMap<String, bool>,
}
impl Workbench {
    pub(super) fn update_http_structure(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        let request = self.current().clone();
        if self.structure_ui.request != Some(request.id) {
            self.structure_ui.request = Some(request.id);
            self.structure_ui.manual_section = false;
            self.structure_ui.expansion.clear();
        }
        self.structure_ui.generation = self.structure_ui.generation.wrapping_add(1);
        self.structure_ui.loading = false;
        if let Some((source, binding)) =
            crate::apifox::binding_for_request(&self.project, request.id)
        {
            self.structure_ui.data = RequestStructure::from_apifox(binding, &source.config.name);
        } else {
            self.structure_ui.data = RequestStructure::local(&request);
            if request.openapi.is_some() {
                self.structure_ui.loading = true;
                let generation = self.structure_ui.generation;
                let store = self.store.clone();
                let tx = self.tx.clone();
                self.runtime.spawn_blocking(move || {
                    let id = request.id;
                    let result = store
                        .request_structure(&request)
                        .map(Box::new)
                        .map_err(|e| format!("无法读取保存的接口定义：{e}"));
                    let _ = tx.blocking_send(WorkerEvent::RequestStructure(id, generation, result));
                });
            }
        }
        self.refresh_structure_section();
    }
    pub(super) fn received_structure(
        &mut self,
        id: Uuid,
        generation: u64,
        result: Result<Box<RequestStructure>, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.current().id != id
            || self.current().protocol != Protocol::Http
            || self.structure_ui.generation != generation
        {
            return;
        }
        self.structure_ui.loading = false;
        match result {
            Ok(data) => self.structure_ui.data = *data,
            Err(error) => self.structure_ui.data.diagnostics.push(error),
        }
        self.refresh_structure_section();
        self.refresh_focused_value_options(window, cx);
        cx.notify();
    }
    fn refresh_structure_section(&mut self) {
        if !self.structure_ui.manual_section {
            self.structure_ui.section = self.structure_ui.data.default_section();
        }
        self.structure_ui.nodes = self
            .structure_ui
            .data
            .documentation(self.structure_ui.section);
    }
    pub(super) fn structure_panel(
        &self,
        protocol: &Protocol,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut panel = v_flex()
            .size_full()
            .min_h_0()
            .min_w_0()
            .border_l_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .px_3()
                    .py_3()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("请求结构"),
            );
        if *protocol != Protocol::Http {
            return panel
                .child(
                    Editor::new(&self.schema)
                        .readonly(true)
                        .appearance(false)
                        .flex_1()
                        .min_h_0(),
                )
                .into_any_element();
        }
        let ui = &self.structure_ui;
        panel = panel.child(
            div()
                .px_3()
                .pb_2()
                .text_size(px(11.))
                .text_color(cx.theme().muted_foreground)
                .truncate()
                .child(if ui.loading {
                    "正在读取保存的接口定义…".into()
                } else {
                    ui.data.source.clone()
                }),
        );
        let mut tabs = h_flex()
            .px_2()
            .gap(px(1.))
            .border_b_1()
            .border_color(cx.theme().border);
        for section in Section::ALL {
            tabs = tabs.child(
                div()
                    .border_b_2()
                    .border_color(if ui.section == section {
                        cx.theme().primary
                    } else {
                        transparent_black()
                    })
                    .child(
                        Button::new(SharedString::from(format!("structure-{}", section.label())))
                            .small()
                            .ghost()
                            .h(px(34.))
                            .px(px(6.))
                            .label(section.label())
                            .accessibility_label(format!(
                                "结构 · {}，{} 项",
                                section.label(),
                                ui.data.count(section)
                            ))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.structure_ui.section = section;
                                this.structure_ui.manual_section = true;
                                this.refresh_structure_section();
                                cx.notify();
                            })),
                    ),
            );
        }
        panel = panel.child(tabs);
        if !ui.data.diagnostics.is_empty() {
            panel = panel.child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(ui.data.diagnostics.join("\n")),
            );
        }
        if ui.data.count(ui.section) > 0 {
            let mut docs = v_flex().gap_3();
            for (i, node) in ui.nodes.iter().enumerate() {
                let key = format!("{}:{i}", ui.section.label());
                docs = docs.child(self.structure_node(node, &key, 0, cx));
            }
            panel = panel.child(
                div()
                    .id("structure-documentation-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_3()
                    .child(docs),
            );
        } else {
            let mut empty = v_flex().p_4().gap_3();
            if ui.data.defined {
                empty = empty.child(div().text_sm().child(ui.section.empty_message()));
            } else if !ui.loading {
                empty = empty
                    .child(div().text_sm().child("当前请求未关联接口定义"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("可打开数据源文件夹内已同步的请求，或导入 OpenAPI 文档。"),
                    )
                    .child(
                        Button::new("structure-sources")
                            .small()
                            .ghost()
                            .label("打开数据源")
                            .on_click(cx.listener(|this, _, _, cx| this.open_sync(None, cx))),
                    )
                    .child(
                        Button::new("structure-import")
                            .small()
                            .ghost()
                            .label("导入 OpenAPI")
                            .on_click(cx.listener(|this, _, _, cx| this.import_openapi(cx))),
                    );
            }
            panel = panel.child(empty);
        }
        panel.into_any_element()
    }

    fn structure_node(
        &self,
        node: &SchemaNode,
        key: &str,
        depth: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let expanded = self
            .structure_ui
            .expansion
            .get(key)
            .copied()
            .unwrap_or(depth < 2);
        let has_children = !node.children.is_empty();
        let mut heading = h_flex().gap_2().items_center().min_w_0();
        if has_children {
            let key = key.to_owned();
            heading = heading.child(
                Button::new(SharedString::from(format!("toggle-{key}")))
                    .xsmall()
                    .ghost()
                    .icon(if expanded {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .accessibility_label(format!(
                        "{}结构 {}",
                        if expanded { "收起" } else { "展开" },
                        node.name
                    ))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.structure_ui.expansion.insert(key.clone(), !expanded);
                        cx.notify();
                    })),
            );
        }
        heading = heading.child(
            div()
                .flex_1()
                .min_w_0()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(node.name.clone()),
        );
        if let Some(required) = node.required {
            heading = heading.child(
                div()
                    .flex_shrink_0()
                    .text_size(px(10.))
                    .text_color(if required {
                        cx.theme().primary
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(if required { "必填" } else { "可选" }),
            );
        }
        let mut metadata = h_flex()
            .gap_2()
            .flex_wrap()
            .items_center()
            .text_size(px(10.))
            .text_color(cx.theme().muted_foreground);
        if let Some(location) = &node.location {
            metadata = metadata.child(
                div()
                    .px_2()
                    .py(px(2.))
                    .rounded(px(4.))
                    .bg(cx.theme().muted)
                    .child(location.clone()),
            );
        }
        metadata = metadata.child(div().font_family("Menlo").child(node.kind.clone()));
        let mut content = v_flex().gap_2().min_w_0().child(heading).child(metadata);
        if !node.description.is_empty() {
            content = content.child(
                div()
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(node.description.clone()),
            );
        }
        if !node.values.is_empty() {
            let mut values = h_flex().gap_1().flex_wrap().items_center();
            for value in &node.values {
                values = values.child(
                    div()
                        .px_2()
                        .py(px(3.))
                        .rounded(px(4.))
                        .bg(cx.theme().primary.opacity(0.08))
                        .text_size(px(11.))
                        .text_color(cx.theme().primary)
                        .child(if value.is_empty() {
                            "空字符串".into()
                        } else {
                            value.clone()
                        }),
                );
            }
            content = content.child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(cx.theme().muted_foreground)
                            .child("可选值"),
                    )
                    .child(values),
            );
        }
        for (label, value) in &node.details {
            content = content.child(
                h_flex()
                    .gap_2()
                    .items_start()
                    .text_size(px(11.))
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_color(cx.theme().muted_foreground)
                            .child(label.clone()),
                    )
                    .child(div().min_w_0().child(value.clone())),
            );
        }
        if let Some(reference) = &node.reference {
            content = content.child(
                div()
                    .text_size(px(11.))
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "引用 · {}",
                        reference.rsplit('/').next().unwrap_or(reference)
                    )),
            );
        }
        if has_children {
            if expanded {
                let mut children = v_flex()
                    .gap_3()
                    .mt_1()
                    .pl_3()
                    .border_l_1()
                    .border_color(cx.theme().border);
                for (i, child) in node.children.iter().enumerate() {
                    children = children.child(self.structure_node(
                        child,
                        &format!("{key}/{i}"),
                        depth + 1,
                        cx,
                    ));
                }
                content = content.child(children);
            } else {
                content = content.child(
                    div()
                        .text_size(px(10.))
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{} 个字段 · 展开查看", node.children.len())),
                );
            }
        }
        if depth == 0 {
            div()
                .p_3()
                .rounded(px(8.))
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().background)
                .child(content)
                .into_any_element()
        } else {
            content.into_any_element()
        }
    }
}
