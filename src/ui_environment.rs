use super::*;
use gpui_kit::component::{FocusableExt, searchable_list::SearchableListItem};
use std::collections::BTreeMap;

#[derive(Clone, IntoElement)]
pub(super) struct EnvironmentOption(pub(super) String);

impl SearchableListItem for EnvironmentOption {
    type Value = String;

    fn value(&self) -> &String {
        &self.0
    }

    fn title(&self) -> SharedString {
        self.0.clone().into()
    }

    fn display_title(&self) -> Option<AnyElement> {
        Some(self.clone().into_any_element())
    }
}

impl RenderOnce for EnvironmentOption {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let initial = self
            .0
            .trim()
            .chars()
            .next()
            .unwrap_or('·')
            .to_ascii_uppercase();
        let ink = cx.theme().cyan;
        h_flex()
            .min_w_0()
            .items_center()
            .gap(px(6.))
            .child(
                h_flex()
                    .size(px(20.))
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .rounded(px(5.))
                    .bg(ink.opacity(0.10))
                    .text_color(ink)
                    .text_size(px(11.))
                    .line_height(relative(1.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(initial.to_string()),
            )
            .child(div().min_w_0().truncate().child(self.0))
    }
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum EditorMode {
    #[default]
    Table,
    Json,
}
#[derive(Default)]
pub(super) struct EnvironmentUi {
    window: Option<(AnyWindowHandle, WeakEntity<EnvironmentWindow>)>,
    mode: EditorMode,
    selector_needs_refresh: bool,
}

pub(super) enum EnvironmentsChanged {
    SelectionOrList,
    Variables(String),
}
impl EventEmitter<EnvironmentsChanged> for Workbench {}

struct VariableRow {
    key: String,
    value: String,
    inputs: Option<VariableInputs>,
}
struct VariableInputs {
    key: Entity<InputState>,
    value: Entity<InputState>,
    sensitive: Rc<Cell<bool>>,
    _subscriptions: Vec<Subscription>,
}

struct EnvironmentWindow {
    owner: Entity<Workbench>,
    handle: AnyWindowHandle,
    focus: FocusHandle,
    workspace_name: String,
    editing_name: String,
    name: Entity<InputState>,
    json: Entity<EditorState>,
    rows: Vec<VariableRow>,
    scroll: UniformListScrollHandle,
    mode: EditorMode,
    changed: bool,
    error: String,
}

#[derive(Clone)]
pub(super) struct EnvironmentDraft {
    name: String,
    json: String,
    // Preserve unfinished rows (including duplicate or empty keys) across switches.
    rows: Option<Vec<(String, String)>>,
}
impl EnvironmentDraft {
    fn variables(&self) -> Result<BTreeMap<String, String>, String> {
        match &self.rows {
            Some(rows) => variable_map(rows.clone()),
            None => serde_json::from_str(&self.json).map_err(|error| error.to_string()),
        }
    }
}
fn sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    ["token", "secret", "password", "api_key", "apikey"]
        .iter()
        .any(|part| key.contains(part))
}
fn variable_map(rows: Vec<(String, String)>) -> Result<BTreeMap<String, String>, String> {
    let mut values = BTreeMap::new();
    for (index, (key, value)) in rows.into_iter().enumerate() {
        let key = key.trim();
        if key.is_empty() && value.is_empty() {
            continue;
        }
        if key.is_empty() {
            return Err(format!("第 {} 行缺少变量名", index + 1));
        }
        if values.insert(key.to_string(), value).is_some() {
            return Err(format!("变量名「{key}」重复，请使用不同的名称"));
        }
    }
    Ok(values)
}
impl Workbench {
    pub(super) fn environment_switcher(&self, cx: &Context<Self>) -> Div {
        h_flex()
            .h(TITLEBAR_CONTROL_HEIGHT)
            .flex_shrink_0()
            .items_center()
            .child(
                div().w(px(134.)).h_full().child(
                    Select::new(&self.env_select)
                        .small()
                        .focus_ring(false)
                        .w_full()
                        .h(TITLEBAR_CONTROL_HEIGHT)
                        .px_2()
                        .text_size(TITLEBAR_LABEL_SIZE)
                        .bg(cx.theme().background)
                        .rounded(TITLEBAR_CONTROL_RADIUS)
                        .rounded_r_none()
                        .accessibility_label("切换环境")
                        .menu_width(px(180.)),
                ),
            )
            .child(
                Button::new("environment-open")
                    .size(TITLEBAR_CONTROL_HEIGHT)
                    .rounded(TITLEBAR_CONTROL_RADIUS)
                    .rounded_l_none()
                    .border_l_0()
                    .border_color(cx.theme().input)
                    .bg(cx.theme().background)
                    .shadow_none()
                    .icon(IconName::Menu)
                    .accessibility_label("环境设置")
                    .tooltip("环境与变量设置")
                    .on_click(cx.listener(|this, _, _, cx| this.open_environment(cx))),
            )
    }

    fn open_environment(&self, cx: &mut Context<Self>) {
        let owner = cx.weak_entity();
        cx.defer(move |cx| {
            let Some(owner) = owner.upgrade() else { return };
            if let Some((handle, _)) = &owner.read(cx).environment_ui.window {
                let handle = *handle;
                if handle
                    .update(cx, |_, window, _| window.activate_window())
                    .is_ok()
                {
                    return;
                }
            }
            let options = WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(format!("环境设置 · {}", owner.read(cx).project.name).into()),
                    ..TitleBar::title_bar_options()
                }),
                window_bounds: Some(WindowBounds::centered(size(px(860.), px(580.)), cx)),
                window_min_size: Some(size(px(640.), px(420.))),
                ..TitleBar::window_options()
            };
            let result = gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| EnvironmentWindow::new(owner.clone(), window, cx))
            });
            owner.update(cx, |workbench, cx| match result {
                Ok((handle, view)) => {
                    workbench.environment_ui.window = Some((handle, view.downgrade()))
                }
                Err(error) => {
                    workbench.notice = format!("无法打开环境设置：{error}");
                    cx.notify();
                }
            });
        });
    }
    pub(crate) fn close_environment_window(&self, cx: &mut Context<Self>) {
        if let Some((handle, _)) = &self.environment_ui.window {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
    }
    pub(super) fn environment_has_unapplied_edits(&self, name: &str, cx: &App) -> bool {
        let original = self
            .project
            .environments
            .iter()
            .find(|env| env.name == name);
        if let Some(editor) = self
            .environment_ui
            .window
            .as_ref()
            .and_then(|(_, view)| view.upgrade())
        {
            let editor = editor.read(cx);
            if editor.editing_name == name {
                return editor.changed
                    && (editor.name.read(cx).value().trim() != name
                        || editor.environment_variables(cx).ok().as_ref()
                            != original.map(|env| &env.variables));
            }
        }
        self.environment_drafts.get(name).is_some_and(|draft| {
            draft.name.trim() != name
                || draft.variables().ok().as_ref() != original.map(|env| &env.variables)
        })
    }
    pub(super) fn sync_environment_selector(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !std::mem::take(&mut self.environment_ui.selector_needs_refresh) {
            return;
        }
        self.env_select.update(cx, |selector, cx| {
            selector.set_items(
                self.project
                    .environments
                    .iter()
                    .map(|env| EnvironmentOption(env.name.clone()))
                    .collect(),
                window,
                cx,
            );
            selector.set_selected_index(
                Some(IndexPath::new(self.project.active_environment)),
                window,
                cx,
            );
            cx.notify();
        });
    }
    pub(super) fn select_environment(&mut self, index: usize, cx: &mut Context<Self>) {
        if index != self.project.active_environment && index < self.project.environments.len() {
            self.project.active_environment = index;
            self.environments_changed(cx);
        }
    }
    fn environments_changed(&mut self, cx: &mut Context<Self>) {
        self.environment_ui.selector_needs_refresh = true;
        cx.emit(EnvironmentsChanged::SelectionOrList);
        self.dirty(cx);
    }
}

impl EnvironmentWindow {
    fn new(owner: Entity<Workbench>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let mut editor = Self {
            workspace_name: owner.read(cx).project.name.clone(),
            mode: owner.read(cx).environment_ui.mode,
            owner: owner.clone(),
            handle: window.window_handle(),
            focus,
            editing_name: String::new(),
            name: cx.new(|cx| InputState::new(window, cx).placeholder("环境名称")),
            json: cx.new(|cx| EditorState::new(window, cx).language("json")),
            rows: Vec::new(),
            scroll: Default::default(),
            changed: false,
            error: String::new(),
        };
        editor.load_environment(owner.read(cx).project.active_environment, window, cx);
        cx.subscribe(&editor.name, |this, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                this.edited(cx);
            }
        })
        .detach();
        cx.subscribe(&editor.json, |this, _, event, cx| {
            if matches!(event, InputEvent::Change) && this.mode == EditorMode::Json {
                this.edited(cx);
            }
        })
        .detach();
        cx.subscribe_in(
            &owner,
            window,
            |this, owner, change: &EnvironmentsChanged, window, cx| {
                if let EnvironmentsChanged::Variables(name) = change
                    && name == &this.editing_name
                    && let Some(index) = owner
                        .read(cx)
                        .project
                        .environments
                        .iter()
                        .position(|env| &env.name == name)
                {
                    this.load_environment(index, window, cx);
                } else {
                    cx.notify();
                }
            },
        )
        .detach();
        cx.observe_in(&owner, window, |this, owner, window, cx| {
            let name = &owner.read(cx).project.name;
            if this.workspace_name != *name {
                this.workspace_name.clone_from(name);
                window.set_window_title(&format!("环境设置 · {name}"));
                cx.notify();
            }
        })
        .detach();
        cx.on_release(|this, cx| {
            if this
                .owner
                .read(cx)
                .environment_ui
                .window
                .as_ref()
                .is_some_and(|(handle, _)| *handle == this.handle)
            {
                this.stash_draft(cx);
                this.owner.update(cx, |owner, _| {
                    owner.environment_ui.window = None;
                    owner.environment_ui.mode = this.mode;
                });
            }
        })
        .detach();
        editor
    }
    fn edited(&mut self, cx: &mut Context<Self>) {
        self.changed = true;
        self.error.clear();
        cx.notify();
    }
    fn environment_rows(&self, cx: &App) -> Vec<(String, String)> {
        self.rows
            .iter()
            .map(|row| match &row.inputs {
                Some(inputs) => (
                    inputs.key.read(cx).value().to_string(),
                    inputs.value.read(cx).value().to_string(),
                ),
                None => (row.key.clone(), row.value.clone()),
            })
            .collect()
    }
    fn set_rows(&mut self, rows: impl IntoIterator<Item = (String, String)>) {
        self.rows = rows
            .into_iter()
            .map(|(key, value)| VariableRow {
                key,
                value,
                inputs: None,
            })
            .collect();
        self.scroll = Default::default();
    }
    fn row_inputs(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> &VariableInputs {
        let row = &mut self.rows[index];
        row.inputs.get_or_insert_with(|| {
            let sensitive = Rc::new(Cell::new(sensitive_key(&row.key)));
            let key = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("variable_name")
                    .default_value(std::mem::take(&mut row.key))
            });
            let value = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("变量值")
                    .masked(sensitive.get())
                    .default_value(std::mem::take(&mut row.value))
            });
            let mask = sensitive.clone();
            let value_for_key = value.clone();
            let on_key = cx.subscribe_in(&key, window, move |this, key, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    let next = sensitive_key(key.read(cx).value().as_ref());
                    if mask.replace(next) != next {
                        value_for_key.update(cx, |state, cx| state.set_masked(next, window, cx));
                    }
                    this.edited(cx);
                }
            });
            let on_value = cx.subscribe(&value, |this, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    this.edited(cx);
                }
            });
            VariableInputs {
                key,
                value,
                sensitive,
                _subscriptions: vec![on_key, on_value],
            }
        })
    }
    fn load_environment(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let owner = self.owner.read(cx);
        let env = &owner.project.environments[index];
        self.editing_name.clone_from(&env.name);
        let draft = owner.environment_drafts.get(&env.name);
        self.changed = draft.is_some();
        let name = draft.map_or_else(|| env.name.clone(), |draft| draft.name.clone());
        if let Some(draft) = draft {
            self.mode = if draft.rows.is_some() {
                EditorMode::Table
            } else {
                EditorMode::Json
            };
        }
        match self.mode {
            EditorMode::Table => {
                let rows = draft
                    .and_then(|draft| draft.rows.clone())
                    .unwrap_or_else(|| env.variables.clone().into_iter().collect());
                self.set_rows(rows);
            }
            EditorMode::Json => {
                let text = draft.map(|draft| draft.json.clone()).unwrap_or_else(|| {
                    serde_json::to_string_pretty(&env.variables).unwrap_or_default()
                });
                self.json
                    .update(cx, |editor, cx| editor.set_value(text, window, cx));
                self.rows.clear();
            }
        }
        self.name
            .update(cx, |input, cx| input.set_value(name, window, cx));
        self.error.clear();
        cx.notify();
    }
    fn stash_draft(&self, cx: &mut App) {
        if self.changed {
            let draft = EnvironmentDraft {
                name: self.name.read(cx).value().to_string(),
                json: if self.mode == EditorMode::Json {
                    self.json.read(cx).value().to_string()
                } else {
                    String::new()
                },
                rows: (self.mode == EditorMode::Table).then(|| self.environment_rows(cx)),
            };
            self.owner.update(cx, |owner, _| {
                owner
                    .environment_drafts
                    .insert(self.editing_name.clone(), draft);
            });
        }
    }
    fn edit_environment(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing_name != self.owner.read(cx).project.environments[index].name {
            self.stash_draft(cx);
            self.load_environment(index, window, cx);
        }
    }
    fn new_environment(&mut self, duplicate: bool, window: &mut Window, cx: &mut Context<Self>) {
        let (base, variables) = if duplicate {
            match self.environment_variables(cx) {
                Ok(vars) => (format!("{} 副本", self.name.read(cx).value().trim()), vars),
                Err(error) => {
                    self.error = error;
                    cx.notify();
                    return;
                }
            }
        } else {
            ("新环境".into(), BTreeMap::new())
        };
        let index = self.owner.update(cx, |owner, cx| {
            let mut name = base.clone();
            let mut suffix = 2;
            while owner
                .project
                .environments
                .iter()
                .any(|env| env.name == name)
                || owner
                    .deleted_environment
                    .as_ref()
                    .is_some_and(|(_, env)| env.name == name)
            {
                name = format!("{base} {suffix}");
                suffix += 1;
            }
            owner
                .project
                .environments
                .push(Environment { name, variables });
            owner.environments_changed(cx);
            owner.project.environments.len() - 1
        });
        self.edit_environment(index, window, cx);
    }
    fn delete_environment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let project = &self.owner.read(cx).project;
        let Some(index) = project
            .environments
            .iter()
            .position(|env| env.name == self.editing_name)
        else {
            return;
        };
        if index == project.active_environment {
            return;
        }
        self.stash_draft(cx);
        self.owner.update(cx, |owner, cx| {
            if let Some((_, previous)) = owner.deleted_environment.take() {
                owner.environment_drafts.remove(&previous.name);
            }
            owner.deleted_environment = Some((index, owner.project.environments.remove(index)));
            if index < owner.project.active_environment {
                owner.project.active_environment -= 1;
            }
            owner.notice = "环境已删除，可恢复上次删除的环境".into();
            owner.environments_changed(cx);
        });
        self.load_environment(self.owner.read(cx).project.active_environment, window, cx);
    }
    fn restore_environment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let restored = self.owner.update(cx, |owner, cx| {
            let (index, mut env) = owner.deleted_environment.take()?;
            let previous_name = env.name.clone();
            while owner
                .project
                .environments
                .iter()
                .any(|item| item.name == env.name)
            {
                env.name.push_str(" 恢复");
            }
            if env.name != previous_name
                && let Some(mut draft) = owner.environment_drafts.remove(&previous_name)
            {
                draft.name.clone_from(&env.name);
                owner.environment_drafts.insert(env.name.clone(), draft);
            }
            let index = index.min(owner.project.environments.len());
            owner.project.environments.insert(index, env);
            if index <= owner.project.active_environment {
                owner.project.active_environment += 1;
            }
            owner.environments_changed(cx);
            Some(index)
        });
        if let Some(index) = restored {
            self.edit_environment(index, window, cx);
        }
    }
    fn environment_variables(&self, cx: &App) -> Result<BTreeMap<String, String>, String> {
        match self.mode {
            EditorMode::Table => variable_map(self.environment_rows(cx)),
            EditorMode::Json => serde_json::from_str(self.json.read(cx).value().as_ref())
                .map_err(|error| format!("请输入 JSON 字符串键值表：{error}")),
        }
    }
    fn switch_environment_editor(
        &mut self,
        mode: EditorMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.mode == mode {
            return;
        }
        match self.environment_variables(cx) {
            Ok(vars) => {
                match mode {
                    EditorMode::Table => self.set_rows(vars),
                    EditorMode::Json => self.json.update(cx, |editor, cx| {
                        editor.set_value(
                            serde_json::to_string_pretty(&vars).unwrap_or_default(),
                            window,
                            cx,
                        )
                    }),
                }
                self.mode = mode;
                self.error.clear();
            }
            Err(error) => self.error = error,
        }
        cx.notify();
    }
    fn save_environment(&mut self, apply: bool, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.name.read(cx).value().trim().to_string();
        let owner = self.owner.read(cx);
        let project = &owner.project;
        let Some(index) = project
            .environments
            .iter()
            .position(|env| env.name == self.editing_name)
        else {
            self.error = "当前编辑的环境已删除".into();
            cx.notify();
            return;
        };
        let result = if owner
            .deleted_environment
            .as_ref()
            .is_some_and(|(_, env)| env.name == name)
        {
            Err("环境名称与待恢复的环境重复，请使用不同的名称".into())
        } else if name.is_empty()
            || project
                .environments
                .iter()
                .any(|env| env.name != self.editing_name && env.name == name)
        {
            Err("环境名称不能为空或重复".into())
        } else {
            self.environment_variables(cx)
        };
        match result {
            Ok(vars) => {
                self.owner.update(cx, |owner, cx| {
                    let env = &mut owner.project.environments[index];
                    owner.environment_drafts.remove(&env.name);
                    env.name.clone_from(&name);
                    env.variables = vars;
                    owner.notice = if apply {
                        owner.project.active_environment = index;
                        format!("已应用环境 {name}")
                    } else {
                        format!("已保存环境 {name}")
                    };
                    owner.environments_changed(cx);
                });
                self.editing_name = name;
                self.changed = false;
                self.error.clear();
                if apply {
                    window.remove_window();
                } else {
                    cx.notify();
                }
            }
            Err(error) => {
                self.error = error;
                cx.notify();
            }
        }
    }
    fn environment_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let owner = self.owner.read(cx);
        let project = &owner.project;
        let theme = cx.theme();
        let border = theme.border;
        let muted = theme.muted_foreground;
        let mut sidebar = v_flex()
            .w(px(164.))
            .flex_shrink_0()
            .h_full()
            .min_h_0()
            .bg(theme.sidebar)
            .border_r_1()
            .border_color(border)
            .p_3()
            .gap_1()
            .child(
                h_flex()
                    .px_2()
                    .pb_2()
                    .justify_between()
                    .text_xs()
                    .text_color(muted)
                    .child("环境")
                    .child(project.environments.len().to_string()),
            );
        let mut environments = v_flex()
            .id("environment-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .gap_1();
        for (index, environment) in project.environments.iter().enumerate() {
            let selected = environment.name == self.editing_name;
            let applied = index == project.active_environment;
            environments = environments.child(
                h_flex()
                    .id(("environment-item", index))
                    .h(px(36.))
                    .px_2()
                    .gap_2()
                    .rounded_md()
                    .items_center()
                    .cursor_pointer()
                    .when(selected, |row| row.bg(cx.theme().accent))
                    .hover(|row| row.bg(cx.theme().accent))
                    .text_color(if selected {
                        theme.primary
                    } else {
                        theme.foreground
                    })
                    .role(Role::Button)
                    .aria_label(format!(
                        "编辑环境 {}{}",
                        environment.name,
                        if applied { "，使用中" } else { "" }
                    ))
                    .child(Icon::new(gpui_kit::assets::IconName::Layers2).size(px(14.)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(environment.name.clone()),
                    )
                    .when(applied, |row| {
                        row.child(Icon::new(IconName::Check).size(px(13.)))
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit_environment(index, window, cx)
                    })),
            );
        }
        sidebar = sidebar
            .child(environments)
            .when(owner.deleted_environment.is_some(), |sidebar| {
                sidebar.child(
                    Button::new("environment-restore")
                        .small()
                        .ghost()
                        .label("恢复上次删除")
                        .on_click(
                            cx.listener(|this, _, window, cx| this.restore_environment(window, cx)),
                        ),
                )
            })
            .child(
                Button::new("environment-new")
                    .small()
                    .ghost()
                    .icon(IconName::Plus)
                    .label("新建环境")
                    .on_click(
                        cx.listener(|this, _, window, cx| this.new_environment(false, window, cx)),
                    ),
            );
        let mut modes = h_flex()
            .rounded_md()
            .p_0p5()
            .border_1()
            .border_color(border);
        for (mode, label) in [(EditorMode::Table, "表格"), (EditorMode::Json, "JSON")] {
            modes = modes.child(
                Button::new(("environment-mode", mode as usize))
                    .xsmall()
                    .ghost()
                    .label(label)
                    .when(self.mode == mode, |button| {
                        button.bg(cx.theme().accent).text_color(cx.theme().primary)
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.switch_environment_editor(mode, window, cx)
                    })),
            );
        }
        let editing_applied =
            project.environments[project.active_environment].name == self.editing_name;
        let mut editor = v_flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .p_5()
            .gap_3()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Input::new(&self.name)
                            .aria_label("环境名称")
                            .flex_1()
                            .min_w_0(),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .text_xs()
                            .text_color(theme.primary)
                            .bg(theme.accent)
                            .child(if editing_applied {
                                "使用中"
                            } else {
                                "未应用"
                            }),
                    )
                    .child(
                        Button::new("environment-copy")
                            .small()
                            .ghost()
                            .icon(IconName::Copy)
                            .tooltip("复制环境")
                            .accessibility_label("复制环境")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.new_environment(true, window, cx)
                            })),
                    )
                    .child(
                        Button::new("environment-delete")
                            .small()
                            .ghost()
                            .icon(Icon::new(gpui_kit::assets::IconName::Trash))
                            .tooltip(if editing_applied {
                                "先应用其他环境，再删除此环境"
                            } else {
                                "删除环境"
                            })
                            .accessibility_label("删除环境")
                            .disabled(editing_applied)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.delete_environment(window, cx)
                            })),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .pt_2()
                    .child(h_flex().gap_2().child("环境变量").when(
                        self.mode == EditorMode::Table,
                        |label| {
                            label.child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(self.rows.len().to_string()),
                            )
                        },
                    ))
                    .child(modes),
            );
        editor = editor.child(if self.mode == EditorMode::Table {
            self.environment_table(cx)
        } else {
            Editor::new(&self.json)
                .h(relative(1.))
                .flex_1()
                .min_h_0()
                .bordered(true)
                .into_any_element()
        });
        editor = editor
            .when(!self.error.is_empty(), |editor| {
                editor.child(
                    div()
                        .text_sm()
                        .text_color(method_color("DELETE"))
                        .child(self.error.clone()),
                )
            })
            .child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .text_xs()
                    .text_color(muted)
                    .child("在请求中使用")
                    .child(
                        div()
                            .px_1()
                            .rounded_sm()
                            .bg(theme.accent)
                            .text_color(theme.primary)
                            .child("{{base_url}}"),
                    )
                    .child("引用变量"),
            );
        v_flex()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_stretch()
                    .child(sidebar)
                    .child(editor),
            )
            .child(
                h_flex()
                    .h(px(62.))
                    .px_5()
                    .flex_shrink_0()
                    .items_center()
                    .justify_between()
                    .border_t_1()
                    .border_color(border)
                    .child(div().text_xs().text_color(muted).child(if self.changed {
                        "有未应用的更改 · 关闭保留草稿"
                    } else {
                        "所有数据仅保存在本地"
                    }))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("close-environment")
                                    .label("关闭")
                                    .on_click(|_, window, _| window.remove_window()),
                            )
                            .child(
                                Button::new("save-environment")
                                    .primary()
                                    .label("保存并应用")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_environment(true, window, cx)
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }
    fn environment_variable_row(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let row = self.row_inputs(index, window, cx);
        let border = cx.theme().border;
        h_flex()
            .w_full()
            .h(px(44.))
            .flex_shrink_0()
            .px_2()
            .gap_2()
            .items_center()
            .border_b_1()
            .border_color(border)
            .child(
                Input::new(&row.key)
                    .small()
                    .appearance(false)
                    .w(relative(0.36))
                    .aria_label(format!("变量名 {}", index + 1)),
            )
            .child(
                Input::new(&row.value)
                    .small()
                    .appearance(false)
                    .flex_1()
                    .min_w_0()
                    .when(row.sensitive.get(), |input| input.mask_toggle())
                    .aria_label(format!("变量 {} 的值", row.key.read(cx).value())),
            )
            .child(
                Button::new(("environment-remove-variable", index))
                    .xsmall()
                    .w(px(24.))
                    .ghost()
                    .icon(IconName::Close)
                    .accessibility_label(format!("删除变量 {}", row.key.read(cx).value()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.rows.remove(index);
                        this.edited(cx);
                    })),
            )
            .into_any_element()
    }
    fn environment_table(&self, cx: &Context<Self>) -> AnyElement {
        let border = cx.theme().border;
        let rows = uniform_list(
            "environment-variable-rows",
            self.rows.len(),
            cx.processor(|this, range: std::ops::Range<usize>, window, cx| {
                range
                    .map(|index| this.environment_variable_row(index, window, cx))
                    .collect::<Vec<_>>()
            }),
        )
        .track_scroll(&self.scroll)
        .w_full()
        .flex_1()
        .min_h_0();
        v_flex()
            .flex_1()
            .min_h_0()
            .gap_2()
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .rounded_lg()
                    .border_1()
                    .border_color(border)
                    .overflow_hidden()
                    .child(
                        h_flex()
                            .h(px(30.))
                            .px_2()
                            .gap_2()
                            .flex_shrink_0()
                            .items_center()
                            .bg(cx.theme().muted)
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(div().w(relative(0.36)).pl_2().child("变量名"))
                            .child(div().flex_1().pl_2().child("变量值"))
                            .child(div().w(px(24.))),
                    )
                    .child(rows),
            )
            .child(
                h_flex().child(
                    Button::new("environment-add-variable")
                        .small()
                        .ghost()
                        .icon(IconName::Plus)
                        .label("添加变量")
                        .text_color(cx.theme().primary)
                        .on_click(cx.listener(|this, _, window, cx| {
                            let index = this.rows.len();
                            this.rows.push(VariableRow {
                                key: String::new(),
                                value: String::new(),
                                inputs: None,
                            });
                            this.scroll.scroll_to_item(index, ScrollStrategy::Bottom);
                            this.row_inputs(index, window, cx)
                                .key
                                .read(cx)
                                .focus_handle(cx)
                                .focus(window, cx);
                            this.edited(cx);
                        })),
                ),
            )
            .into_any_element()
    }
}
impl Render for EnvironmentWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .min_h_0()
            .key_context("Zeph")
            .track_focus(&self.focus)
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(|this, _: &SaveProject, window, cx| {
                this.save_environment(false, window, cx)
            }))
            .on_action(|_: &CloseRequest, window, _| window.remove_window())
            .on_action(|_: &crate::ui_settings::OpenSettings, _, cx| {
                crate::ui_settings::open(None, cx)
            })
            .on_action(|_: &crate::ui_settings::CheckUpdates, _, cx| {
                crate::ui_settings::open_updates(cx)
            })
            .child(
                TitleBar::new().child(
                    div()
                        .flex_1()
                        .truncate()
                        .child(format!("环境设置 · {}", self.workspace_name)),
                ),
            )
            .child(self.environment_panel(cx))
    }
}

#[cfg(test)]
mod tests {
    use super::{EnvironmentDraft, sensitive_key, variable_map};
    #[test]
    fn table_conversion_rejects_duplicate_keys_before_overwriting_values() {
        let error = variable_map(vec![
            ("token".into(), "first".into()),
            (" token ".into(), "second".into()),
        ])
        .unwrap_err();
        assert!(error.contains("重复"));
    }
    #[test]
    fn table_conversion_keeps_empty_values_and_ignores_only_empty_rows() {
        let vars = variable_map(vec![
            (String::new(), String::new()),
            (" token ".into(), String::new()),
            ("base_url".into(), " http://localhost:8080 ".into()),
        ])
        .unwrap();
        assert_eq!(vars.len(), 2);
        assert_eq!(vars["token"], "");
        assert_eq!(vars["base_url"], " http://localhost:8080 ");
        assert!(variable_map(vec![(String::new(), "value".into())]).is_err());
    }
    #[test]
    fn script_writeback_reads_table_drafts_instead_of_stale_json() {
        let mut draft = EnvironmentDraft {
            name: "Local".into(),
            json: r#"{"base_url":"old"}"#.into(),
            rows: Some(vec![("base_url".into(), "edited".into())]),
        };
        assert_eq!(draft.variables().unwrap()["base_url"], "edited");
        draft
            .rows
            .as_mut()
            .unwrap()
            .push(("base_url".into(), "duplicate".into()));
        assert!(draft.variables().is_err());
        draft.rows = None;
        draft.json = "{invalid".into();
        assert!(draft.variables().is_err());
    }
    #[test]
    fn credential_variable_names_are_masked() {
        for key in [
            "access_token",
            "API_KEY",
            "clientSecret",
            "password",
            "apiKey",
        ] {
            assert!(sensitive_key(key));
        }
        assert!(!sensitive_key("base_url"));
    }
}
