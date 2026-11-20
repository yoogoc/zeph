use super::*;
use crate::operations::{Kind, Operation, Outcome, Scope, Stage};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum OperationTarget {
    Request(Uuid),
    Folder(Uuid),
}

#[derive(Default)]
pub(super) struct OperationUi {
    rows: Vec<OperationInputs>,
    help: std::collections::HashSet<Uuid>,
    expanded: std::collections::HashSet<Uuid>,
    checks: HashMap<(OperationTarget, Uuid), String>,
}
impl OperationUi {
    pub(super) fn checked(
        &mut self,
        target: OperationTarget,
        id: Uuid,
        result: Result<crate::scripting::ScriptOutput, String>,
    ) {
        self.checks.insert(
            (target, id),
            match result {
                Ok(output) => output
                    .error
                    .unwrap_or_else(|| "语法检查通过（未执行脚本）".into()),
                Err(error) => error,
            },
        );
    }
}
struct OperationInputs {
    id: Uuid,
    stage: Stage,
    key: Entity<InputState>,
    value: Entity<InputState>,
    source: Entity<InputState>,
    kind: Entity<SelectState<Vec<String>>>,
    scope: Entity<SelectState<Vec<String>>>,
    name: Entity<InputState>,
    script: Entity<EditorState>,
    _subscriptions: Vec<Subscription>,
}
impl Workbench {
    pub(super) fn script_check_matches(
        &self,
        target: OperationTarget,
        id: Uuid,
        source: &str,
    ) -> bool {
        let lists = match target {
            OperationTarget::Request(request) => self
                .project
                .requests
                .iter()
                .find(|r| r.id == request)
                .map(|r| (r.pre_operations.as_slice(), r.post_operations.as_slice())),
            OperationTarget::Folder(folder) => self
                .scope_editors
                .iter()
                .find(|e| e.scope == scope_settings::Scope::Folder(folder))
                .map(|e| (e.operations.pre.as_slice(), e.operations.post.as_slice())),
        };
        lists.is_some_and(|(pre, post)| {
            pre.iter()
                .chain(post)
                .any(|op| op.id == id && op.kind == Kind::JavaScript && op.script == source)
        })
    }
    fn operation_inherits(&self, stage: Stage) -> bool {
        if let Some(editor) = self.active_scope_editor()
            && matches!(editor.scope, scope_settings::Scope::Folder(_))
        {
            return editor.operations.inherits(stage);
        }
        match stage {
            Stage::Pre => self.current().inherit_pre_operations,
            Stage::Post => self.current().inherit_post_operations,
        }
    }
    fn set_operation_inherits(&mut self, stage: Stage, enabled: bool, cx: &mut Context<Self>) {
        if let Some(scope @ scope_settings::Scope::Folder(_)) = self.active_scope() {
            if let Some(editor) = self.scope_editors.iter_mut().find(|e| e.scope == scope) {
                editor.operations.set_inherits(stage, enabled);
            }
        } else {
            match stage {
                Stage::Pre => self.current_mut().inherit_pre_operations = enabled,
                Stage::Post => self.current_mut().inherit_post_operations = enabled,
            }
        }
        self.operations_changed(cx);
    }
    fn open_operation_folder(
        &mut self,
        id: Uuid,
        stage: Stage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let scope = scope_settings::Scope::Folder(id);
        self.open_page_tab(PageTab::Settings(scope), window, cx);
        if let Some(editor) = self.scope_editors.iter_mut().find(|e| e.scope == scope) {
            editor.section = match stage {
                Stage::Pre => scope_settings::ScopeSection::PreOperations,
                Stage::Post => scope_settings::ScopeSection::PostOperations,
            };
        }
        self.reload_operation_inputs(window, cx);
        cx.notify();
    }
    fn inherited_operation_panel(&self, stage: Stage, cx: &mut Context<Self>) -> AnyElement {
        let is_folder = matches!(self.operation_target(), OperationTarget::Folder(_));
        let parent = match self.operation_target() {
            OperationTarget::Folder(id) => self.project.folder(id).ok().and_then(|f| f.parent_id),
            OperationTarget::Request(_) => self.current().folder_id,
        };
        let enabled = self.operation_inherits(stage);
        let mut panel = v_flex()
            .gap_2()
            .p_3()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted)
            .child(
                Checkbox::new("inherit-folder-operations")
                    .checked(enabled)
                    .label("继承上级文件夹操作")
                    .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                        this.set_operation_inherits(stage, *checked, cx)
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(match stage {
                        Stage::Pre => {
                            "执行顺序：外层文件夹 → 内层文件夹 → 当前层；同一层按列表顺序。"
                        }
                        Stage::Post => {
                            "执行顺序：当前层 → 内层文件夹 → 外层文件夹；同一层按列表顺序。"
                        }
                    }),
            );
        if is_folder {
            panel=panel.child(div().text_xs().text_color(cx.theme().muted_foreground).child("当前文件夹操作为草稿，点击“保存设置”后对下级生效。继承项需到来源文件夹修改。"));
        }
        if !enabled {
            return panel
                .child(
                    div()
                        .text_xs()
                        .child("已关闭此阶段继承，仅执行当前层及其下级操作。"),
                )
                .into_any_element();
        }
        match self.project.inherited_operations(parent, stage) {
            Ok(inherited) => {
                let total = inherited.len() + self.operation_list(stage).len();
                panel = panel.child(
                    div()
                        .text_xs()
                        .text_color(if total > 64 {
                            method_color("DELETE")
                        } else {
                            cx.theme().muted_foreground
                        })
                        .child(format!(
                            "继承 {} 项 · 本层 {} 项 · 当前链共 {} 项（上限 64）{}",
                            inherited.len(),
                            self.operation_list(stage).len(),
                            total,
                            if stage == Stage::Post {
                                " · 继承项在本层之后执行"
                            } else {
                                " · 继承项在本层之前执行"
                            }
                        )),
                );
                if inherited.is_empty() {
                    panel = panel.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("上级文件夹尚未配置此阶段操作。"),
                    );
                }
                for (index, op) in inherited.iter().enumerate() {
                    let Some(origin) = &op.origin else { continue };
                    let Some(folder) = origin.folder_id else {
                        continue;
                    };
                    panel = panel.child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                v_flex()
                                    .flex_1()
                                    .gap_1()
                                    .child(div().text_sm().child(format!(
                                        "{} · {}{}",
                                        op.kind.label(),
                                        if op.name.is_empty() {
                                            "未命名"
                                        } else {
                                            &op.name
                                        },
                                        if op.enabled { "" } else { " · 已禁用" }
                                    )))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(origin.label.clone()),
                                    ),
                            )
                            .child(
                                Button::new(("edit-operation-origin", index))
                                    .small()
                                    .ghost()
                                    .label("编辑来源")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.open_operation_folder(folder, stage, window, cx)
                                    })),
                            ),
                    );
                }
            }
            Err(error) => {
                panel = panel.child(
                    div()
                        .text_sm()
                        .text_color(method_color("DELETE"))
                        .child(error.to_string()),
                );
            }
        }
        panel.into_any_element()
    }
    pub(super) fn operation_target(&self) -> OperationTarget {
        match self.active_scope() {
            Some(scope_settings::Scope::Folder(id)) => OperationTarget::Folder(id),
            _ => OperationTarget::Request(self.current().id),
        }
    }
    fn operations_changed(&mut self, cx: &mut Context<Self>) {
        if matches!(self.operation_target(), OperationTarget::Folder(_)) {
            self.notice = "文件夹操作有未保存的修改，点击“保存设置”后对下级生效".into();
            cx.notify();
        } else {
            self.dirty(cx);
        }
    }
    fn check_script(&mut self, stage: Stage, id: Uuid, cx: &mut Context<Self>) {
        let Some(op) = self.operation_list(stage).iter().find(|op| op.id == id) else {
            return;
        };
        let input = crate::scripting::ScriptInput {
            source: op.script.clone(),
            stage,
            variables: Default::default(),
            environment: Default::default(),
            request: crate::scripting::ScriptRequest::from(&Request::new(
                Protocol::Http,
                "语法检查",
            )),
            response: None,
            check_only: true,
        };
        let target = self.operation_target();
        let tx = self.tx.clone();
        self.operation_ui
            .checks
            .insert((target, id), "正在检查…".into());
        self.runtime.spawn(async move {
            let source = input.source.clone();
            let result = crate::scripting::execute(
                input,
                None,
                tokio::time::Instant::now() + crate::scripting::TIMEOUT,
                &CancellationToken::new(),
            )
            .await
            .map_err(|e| e.to_string());
            let _ = tx
                .send(WorkerEvent::ScriptChecked(target, id, source, result))
                .await;
        });
        cx.notify();
    }
    pub(super) fn apply_operation_patch(
        &mut self,
        patch: &crate::operations::EnvironmentPatch,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        if self.environment_has_unapplied_edits(&patch.name, cx) {
            anyhow::bail!("目标环境有未应用的编辑，未写回变量");
        }
        patch.apply(&mut self.project)?;
        self.environment_drafts.remove(&patch.name);
        cx.emit(environment_ui::EnvironmentsChanged::Variables(
            patch.name.clone(),
        ));
        self.dirty(cx);
        Ok(())
    }
    pub(super) fn operation_list(&self, stage: Stage) -> &[Operation] {
        if let Some(editor) = self.active_scope_editor()
            && matches!(editor.scope, scope_settings::Scope::Folder(_))
        {
            return editor.operations.list(stage);
        }
        match stage {
            Stage::Pre => &self.current().pre_operations,
            Stage::Post => &self.current().post_operations,
        }
    }
    fn operation_list_mut(&mut self, stage: Stage) -> &mut Vec<Operation> {
        if let Some(scope @ scope_settings::Scope::Folder(_)) = self.active_scope() {
            return self
                .scope_editors
                .iter_mut()
                .find(|editor| editor.scope == scope)
                .expect("active folder editor")
                .operations
                .list_mut(stage);
        }
        match stage {
            Stage::Pre => &mut self.current_mut().pre_operations,
            Stage::Post => &mut self.current_mut().post_operations,
        }
    }
    pub(super) fn reload_operation_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut rows = vec![];
        let target = self.operation_target();
        for stage in [Stage::Pre, Stage::Post] {
            for op in self.operation_list(stage).to_vec() {
                let id = op.id;
                let key = cx.new(|cx| InputState::new(window, cx).default_value(op.key));
                let value = cx.new(|cx| InputState::new(window, cx).default_value(op.value));
                let source = cx.new(|cx| InputState::new(window, cx).default_value(op.source));
                let name = cx.new(|cx| InputState::new(window, cx).default_value(op.name));
                let script = cx.new(|cx| {
                    let mut editor = EditorState::new(window, cx).language("javascript");
                    editor.set_value(op.script, window, cx);
                    editor
                });
                let mut choices = Kind::choices(stage);
                if !choices.contains(&op.kind) {
                    choices.push(op.kind);
                }
                let index = choices.iter().position(|k| *k == op.kind).unwrap_or(0);
                let kind = cx.new(|cx| {
                    SelectState::new(
                        choices
                            .iter()
                            .map(|k| k.label().to_owned())
                            .collect::<Vec<_>>(),
                        Some(IndexPath::new(index)),
                        window,
                        cx,
                    )
                });
                let scope = cx.new(|cx| {
                    SelectState::new(
                        vec!["仅本次运行".into(), "写入当前环境".into()],
                        Some(IndexPath::new(if op.scope == Scope::Run { 0 } else { 1 })),
                        window,
                        cx,
                    )
                });
                let mut subscriptions = vec![];
                for (field, input) in [key.clone(), value.clone(), source.clone(), name.clone()]
                    .into_iter()
                    .enumerate()
                {
                    subscriptions.push(cx.subscribe_in(
                        &input,
                        window,
                        move |this, input, event: &InputEvent, _, cx| {
                            if matches!(event, InputEvent::Change)
                                && this.operation_target() == target
                                && let Some(op) = this
                                    .operation_list_mut(stage)
                                    .iter_mut()
                                    .find(|o| o.id == id)
                            {
                                let text = input.read(cx).value().to_string();
                                match field {
                                    0 => op.key = text,
                                    1 => op.value = text,
                                    2 => op.source = text,
                                    _ => op.name = text,
                                }
                                this.operation_ui.checks.remove(&(target, id));
                                this.operations_changed(cx);
                            }
                        },
                    ));
                }
                subscriptions.push(cx.subscribe_in(
                    &script,
                    window,
                    move |this, editor, event: &InputEvent, _, cx| {
                        if matches!(event, InputEvent::Change)
                            && this.operation_target() == target
                            && let Some(op) = this
                                .operation_list_mut(stage)
                                .iter_mut()
                                .find(|o| o.id == id)
                        {
                            op.script = editor.read(cx).value().to_string();
                            this.operation_ui.checks.remove(&(target, id));
                            this.operations_changed(cx);
                        }
                    },
                ));
                subscriptions.push(cx.subscribe_in(
                    &kind,
                    window,
                    move |this, _, event: &SelectEvent<Vec<String>>, window, cx| {
                        if let SelectEvent::Confirm(Some(label)) = event
                            && this.operation_target() == target
                            && let Some(kind) = choices.iter().find(|k| k.label() == label).copied()
                            && let Some(op) = this
                                .operation_list_mut(stage)
                                .iter_mut()
                                .find(|o| o.id == id)
                        {
                            *op = Operation {
                                id,
                                script: if kind == Kind::JavaScript {
                                    crate::scripting::sample(stage).into()
                                } else {
                                    String::new()
                                },
                                ..Operation::new(kind)
                            };
                            this.operation_ui.checks.remove(&(target, id));
                            this.reload_operation_inputs(window, cx);
                            this.operations_changed(cx);
                        }
                    },
                ));
                subscriptions.push(cx.subscribe_in(
                    &scope,
                    window,
                    move |this, _, event: &SelectEvent<Vec<String>>, _, cx| {
                        if let SelectEvent::Confirm(Some(label)) = event
                            && this.operation_target() == target
                            && let Some(op) = this
                                .operation_list_mut(stage)
                                .iter_mut()
                                .find(|o| o.id == id)
                        {
                            op.scope = if label == "写入当前环境" {
                                Scope::Environment
                            } else {
                                Scope::Run
                            };
                            this.operations_changed(cx);
                        }
                    },
                ));
                rows.push(OperationInputs {
                    id,
                    stage,
                    key,
                    value,
                    source,
                    kind,
                    scope,
                    name,
                    script,
                    _subscriptions: subscriptions,
                });
            }
        }
        self.operation_ui.rows = rows;
    }
    fn mutate_operation(
        &mut self,
        stage: Stage,
        id: Uuid,
        action: i8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ops = self.operation_list_mut(stage);
        if let Some(index) = ops.iter().position(|o| o.id == id) {
            match action {
                -1 if index > 0 => ops.swap(index, index - 1),
                1 if index + 1 < ops.len() => ops.swap(index, index + 1),
                2 => {
                    ops.remove(index);
                }
                3 => {
                    let mut op = ops[index].clone();
                    op.id = Uuid::new_v4();
                    ops.insert(index + 1, op);
                }
                _ => return,
            }
            self.reload_operation_inputs(window, cx);
            self.operations_changed(cx);
        }
    }
    pub(super) fn operations_editor(&self, stage: Stage, cx: &mut Context<Self>) -> AnyElement {
        let target = self.operation_target();
        let operations = self.operation_list(stage);
        let mut content = v_flex().gap_3().p_4();
        content = content.child(
            h_flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .flex_1()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("{} · {} 项", stage.label(), operations.len())),
                )
                .child(
                    Button::new("add-request-operation")
                        .small()
                        .primary()
                        .icon(IconName::Plus)
                        .label("添加操作")
                        .disabled(operations.len() >= 64)
                        .dropdown_menu({
                            let view = cx.entity().downgrade();
                            move |mut menu, _, _| {
                                for kind in Kind::choices(stage) {
                                    let view = view.clone();
                                    menu = menu.item(PopupMenuItem::new(kind.label()).on_click(
                                        move |_, window, cx| {
                                            let _ = view.update(cx, |this, cx| {
                                                if this.operation_list(stage).len() < 64 {
                                                    let mut op = Operation::new(kind);
                                                    if kind == Kind::JavaScript {
                                                        op.script =
                                                            crate::scripting::sample(stage).into();
                                                    }
                                                    this.operation_list_mut(stage).push(op);
                                                    this.reload_operation_inputs(window, cx);
                                                    this.operations_changed(cx);
                                                }
                                            });
                                        },
                                    ));
                                }
                                menu
                            }
                        }),
                ),
        );
        content=content.child(div().text_xs().text_color(cx.theme().muted_foreground).child(if stage==Stage::Pre {
            "在解析请求变量和建立连接前执行，修改仅作用于发送快照。等待以毫秒计，支持取消；变量支持 {{name}}。"
        } else {"HTTP 收到响应后执行；gRPC / WS 会话结束后执行，JSON 取最后一条收到的消息。取消或传输失败时跳过。"}));
        content = content.child(self.inherited_operation_panel(stage, cx));
        content = content.child(div().text_sm().font_weight(FontWeight::MEDIUM).child(
            if matches!(self.operation_target(), OperationTarget::Folder(_)) {
                "本文件夹操作"
            } else {
                "当前请求操作"
            },
        ));
        if operations.is_empty() {
            content = content.child(
                v_flex()
                    .py_6()
                    .gap_2()
                    .items_center()
                    .child(
                        Icon::new(IconName::Plus)
                            .size_6()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(div().text_sm().child(if stage == Stage::Pre {
                        "发送前准备变量和请求头"
                    } else {
                        "提取响应变量，并验证返回结果"
                    }))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("点击“添加操作”开始，列表按从上到下执行。"),
                    ),
            );
        }
        for (index, op) in operations.iter().enumerate() {
            let Some(inputs) = self
                .operation_ui
                .rows
                .iter()
                .find(|r| r.id == op.id && r.stage == stage)
            else {
                continue;
            };
            let id = op.id;
            let mut card = v_flex()
                .gap_3()
                .p_3()
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().background)
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Checkbox::new(("operation-enabled", index))
                                .checked(op.enabled)
                                .label(format!("{}", index + 1))
                                .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                                    if let Some(op) = this
                                        .operation_list_mut(stage)
                                        .iter_mut()
                                        .find(|o| o.id == id)
                                    {
                                        op.enabled = *checked;
                                        this.operations_changed(cx);
                                    }
                                })),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(Select::new(&inputs.kind).w_full()),
                        )
                        .child(
                            Button::new(("operation-up", index))
                                .small()
                                .ghost()
                                .icon(IconName::ArrowUp)
                                .accessibility_label("上移操作")
                                .disabled(index == 0)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.mutate_operation(stage, id, -1, window, cx)
                                })),
                        )
                        .child(
                            Button::new(("operation-down", index))
                                .small()
                                .ghost()
                                .icon(IconName::ArrowDown)
                                .accessibility_label("下移操作")
                                .disabled(index + 1 == operations.len())
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.mutate_operation(stage, id, 1, window, cx)
                                })),
                        )
                        .child(
                            Button::new(("operation-copy", index))
                                .small()
                                .ghost()
                                .icon(IconName::Copy)
                                .accessibility_label("复制操作")
                                .disabled(operations.len() >= 64)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.mutate_operation(stage, id, 3, window, cx)
                                })),
                        )
                        .child(
                            Button::new(("operation-delete", index))
                                .small()
                                .ghost()
                                .icon(IconName::Delete)
                                .accessibility_label("删除操作")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.mutate_operation(stage, id, 2, window, cx)
                                })),
                        ),
                );
            if op.kind.has_variable() || op.kind == Kind::SetHeader {
                card = card.child(operation_field(
                    if op.kind == Kind::SetHeader {
                        "Header / Metadata 名称"
                    } else {
                        "变量名"
                    },
                    Input::new(&inputs.key).w_full(),
                ));
            }
            if op.kind == Kind::JavaScript {
                let expanded = self.operation_ui.expanded.contains(&id);
                card = card
                    .child(operation_field(
                        "脚本名称",
                        Input::new(&inputs.name).w_full(),
                    ))
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("JavaScript · Boa · 最多 3 秒 · 支持取消"),
                            )
                            .child(
                                Button::new(("script-sample", index))
                                    .small()
                                    .ghost()
                                    .label("插入示例")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let sample = crate::scripting::sample(stage);
                                        if let Some(op) = this
                                            .operation_list_mut(stage)
                                            .iter_mut()
                                            .find(|o| o.id == id)
                                        {
                                            if !op.script.is_empty() {
                                                op.script.push('\n');
                                            }
                                            op.script.push_str("{\n");
                                            op.script.push_str(sample);
                                            op.script.push_str("}\n");
                                        }
                                        this.operation_ui.checks.remove(&(target, id));
                                        this.reload_operation_inputs(window, cx);
                                        this.operations_changed(cx);
                                    })),
                            )
                            .child(
                                Button::new(("script-check", index))
                                    .small()
                                    .ghost()
                                    .label("检查语法")
                                    .disabled(
                                        self.operation_ui
                                            .checks
                                            .get(&(self.operation_target(), id))
                                            .is_some_and(|s| s == "正在检查…"),
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.check_script(stage, id, cx)
                                    })),
                            )
                            .child(
                                Button::new(("script-help", index))
                                    .small()
                                    .ghost()
                                    .label("API 帮助")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if !this.operation_ui.help.remove(&id) {
                                            this.operation_ui.help.insert(id);
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new(("script-expand", index))
                                    .small()
                                    .ghost()
                                    .label(if expanded {
                                        "收起编辑器"
                                    } else {
                                        "放大编辑器"
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if !this.operation_ui.expanded.remove(&id) {
                                            this.operation_ui.expanded.insert(id);
                                        }
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        Editor::new(&inputs.script)
                            .bordered(true)
                            .h(px(if expanded { 480. } else { 250. }))
                            .w_full(),
                    );
                if let Some(message) = self.operation_ui.checks.get(&(self.operation_target(), id))
                {
                    card = card.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(message.clone()),
                    );
                }
                if self.operation_ui.help.contains(&id) {
                    let mut help = v_flex()
                        .gap_2()
                        .p_3()
                        .rounded_lg()
                        .bg(cx.theme().muted)
                        .text_xs();
                    for line in [
                        "zeph.variables.get / set(name, value) · 本次运行变量",
                        "zeph.environment.get / set(name, value) · 环境变量，全部成功后写回",
                        "zeph.request.url / method · 前置可修改；headers / query / path.get / set / add / remove",
                        "zeph.request.body.raw / json() / setJson(value) / setText(value)",
                        "zeph.response.code / status / elapsedMs / text() / json() / headers.get(name)",
                        "zeph.response.trailers.get(name) / messages() · gRPC / WS 在会话结束后读取",
                        "zeph.test(name, () => { ... }) · zeph.assert(condition, message)",
                        "zeph.uuid() · console.log / warn / error · 日志随历史保存",
                        "仅同步脚本；不提供 fetch、文件、系统命令、npm 和 pm.* 兼容层。",
                    ] {
                        help = help.child(line);
                    }
                    card = card.child(help);
                }
            }
            if matches!(
                op.kind,
                Kind::ExtractJson | Kind::AssertJson | Kind::ExtractHeader
            ) {
                card = card.child(operation_field(
                    if op.kind == Kind::ExtractHeader {
                        "响应 Header / Metadata / Trailer 名称"
                    } else {
                        "JSON 路径 · $.data.token / $.items[0].id / /data/token"
                    },
                    Input::new(&inputs.source).w_full(),
                ));
            }
            if matches!(
                op.kind,
                Kind::SetVariable
                    | Kind::SetHeader
                    | Kind::Wait
                    | Kind::AssertJson
                    | Kind::AssertStatus
                    | Kind::AssertElapsed
            ) {
                card = card.child(operation_field(
                    match op.kind {
                        Kind::Wait => "等待时间（ms，最多 30000）",
                        Kind::AssertElapsed => "最大响应耗时（ms）",
                        Kind::AssertStatus => "预期状态码（HTTP：200；gRPC：0）",
                        Kind::AssertJson => "预期值 · JSON 或文本，支持 {{variable}}",
                        _ => "值 · 支持 {{variable}}",
                    },
                    Input::new(&inputs.value).w_full(),
                ));
            }
            if op.kind.has_variable() {
                card = card.child(operation_field(
                    "变量作用域",
                    Select::new(&inputs.scope).w_full(),
                ));
            }
            card = card.child(
                Checkbox::new(("operation-continue", index))
                    .checked(op.continue_on_error)
                    .label("失败后继续后续操作")
                    .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                        if let Some(op) = this
                            .operation_list_mut(stage)
                            .iter_mut()
                            .find(|o| o.id == id)
                        {
                            op.continue_on_error = *checked;
                            this.operations_changed(cx);
                        }
                    })),
            );
            content = content.child(card);
        }
        content=content.child(div().text_xs().text_color(cx.theme().muted_foreground).child("环境写回仅在传输及全部操作成功后提交；并发修改冲突会阻止写回。每阶段最多 64 项。脚本主动打印的内容会保存在本地历史中。"));
        v_flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .child(
                div()
                    .id("request-operation-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(content),
            )
            .into_any_element()
    }
    pub(super) fn operation_results(
        &self,
        record: Option<&RunRecord>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut list = v_flex().p_4().gap_2();
        let reports = record.map(|r| r.operations.as_slice()).unwrap_or_default();
        if reports.is_empty() {
            list = list.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("暂无操作结果，发送请求后在此查看。流式协议在会话结束后汇总。"),
            );
        }
        for (index, report) in reports.iter().enumerate() {
            let (label, color) = match report.outcome {
                Outcome::Passed => ("通过", method_color("GET")),
                Outcome::Failed => ("失败", method_color("DELETE")),
                Outcome::Skipped => ("跳过", cx.theme().muted_foreground),
            };
            list = list.child(
                v_flex()
                    .p_3()
                    .gap_2()
                    .rounded_lg()
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(div().text_sm().text_color(color).child(label))
                            .child(div().flex_1().text_sm().child(format!(
                                "{}. {} · {}",
                                index + 1,
                                report.stage.label(),
                                report.label
                            )))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("{} ms", report.elapsed_ms)),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(report.detail.clone()),
                    )
                    .child({
                        let mut details = v_flex().gap_2();
                        if let Some(origin) = &report.origin {
                            details = details.child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(origin.label.clone()),
                            );
                        }
                        for test in &report.tests {
                            details = details.child(
                                div()
                                    .text_sm()
                                    .text_color(if test.passed {
                                        method_color("GET")
                                    } else {
                                        method_color("DELETE")
                                    })
                                    .child(format!(
                                        "{} {}{}",
                                        if test.passed { "✓" } else { "×" },
                                        test.name,
                                        test.error
                                            .as_ref()
                                            .map(|e| format!(" · {e}"))
                                            .unwrap_or_default()
                                    )),
                            );
                        }
                        if !report.logs.is_empty() {
                            let mut console = v_flex()
                                .gap_1()
                                .p_3()
                                .rounded_lg()
                                .bg(cx.theme().muted)
                                .text_xs();
                            console = console.child(format!("控制台 · {} 条", report.logs.len()));
                            for line in &report.logs {
                                console = console.child(line.clone());
                            }
                            details = details.child(console);
                        }
                        let folder = report.origin.as_ref().and_then(|origin| origin.folder_id);
                        if let Some(folder) = folder.filter(|id| self.project.folder(*id).is_ok()) {
                            let stage = report.stage;
                            details = details.child(
                                h_flex().child(
                                    Button::new(("locate-folder-operation", index))
                                        .small()
                                        .ghost()
                                        .label("编辑来源文件夹")
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.open_operation_folder(folder, stage, window, cx)
                                        })),
                                ),
                            );
                        } else if folder.is_none()
                            && self
                                .operation_list(report.stage)
                                .iter()
                                .any(|op| op.id == report.id && op.kind == Kind::JavaScript)
                        {
                            let stage = report.stage;
                            let id = report.id;
                            details = details.child(
                                h_flex().child(
                                    Button::new(("locate-script", index))
                                        .small()
                                        .ghost()
                                        .label("定位脚本")
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.tab = if stage == Stage::Pre {
                                                Tab::PreOperations
                                            } else {
                                                Tab::PostOperations
                                            };
                                            if let Some(row) = this
                                                .operation_ui
                                                .rows
                                                .iter()
                                                .find(|r| r.id == id && r.stage == stage)
                                            {
                                                row.script
                                                    .read(cx)
                                                    .focus_handle(cx)
                                                    .focus(window, cx);
                                            }
                                            cx.notify();
                                        })),
                                ),
                            );
                        }
                        details
                    }),
            );
        }
        v_flex()
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .id("operation-results-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(list),
            )
            .into_any_element()
    }
}
fn operation_field(label: &'static str, field: impl IntoElement) -> AnyElement {
    v_flex()
        .gap_1()
        .child(div().text_xs().child(label))
        .child(field)
        .into_any_element()
}
