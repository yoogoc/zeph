use super::*;
use crate::auth::AuthOrigin;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Scope {
    Workspace,
    Folder(Uuid),
}
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum ScopeSection {
    #[default]
    General,
    PreOperations,
    PostOperations,
}
pub(super) struct ScopeEditor {
    pub(super) scope: Scope,
    pub(super) section: ScopeSection,
    pub(super) operations: crate::operations::OperationSet,
    name: Entity<InputState>,
    kind: Entity<SelectState<Vec<String>>>,
    token: Entity<InputState>,
    user: Entity<InputState>,
    password: Entity<InputState>,
    _subscription: Subscription,
}
impl Workbench {
    pub(super) fn ensure_scope_editor(
        &mut self,
        scope: Scope,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        if self
            .scope_editors
            .iter()
            .any(|editor| editor.scope == scope)
        {
            return Ok(());
        }
        let (name, auth) = match scope {
            Scope::Workspace => (self.project.name.clone(), Some(self.project.auth.clone())),
            Scope::Folder(id) => {
                let folder = self.project.folder(id)?;
                (folder.name.clone(), folder.auth.clone())
            }
        };
        let modes: Vec<String> = if scope == Scope::Workspace {
            vec!["无认证", "Bearer Token", "Basic Auth"]
        } else {
            vec!["继承上级", "无认证", "Bearer Token", "Basic Auth"]
        }
        .into_iter()
        .map(str::to_owned)
        .collect();
        let selected = auth.as_ref().map(Auth::label).unwrap_or("继承上级");
        let index = modes.iter().position(|m| m == selected).unwrap_or(0);
        let kind = cx.new(|cx| SelectState::new(modes, Some(IndexPath::new(index)), window, cx));
        let name = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let (token, user, password) = match auth {
            Some(Auth::Bearer(t)) => (t, String::new(), String::new()),
            Some(Auth::Basic { username, password }) => (String::new(), username, password),
            _ => (String::new(), String::new(), String::new()),
        };
        let token = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("Token 或 {{token}}")
                .default_value(token)
        });
        let user = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Username 或 {{username}}")
                .default_value(user)
        });
        let password = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("Password 或 {{password}}")
                .default_value(password)
        });
        let subscription = cx.subscribe_in(
            &kind,
            window,
            |_, _, _: &SelectEvent<Vec<String>>, _, cx| cx.notify(),
        );
        self.scope_editors.push(ScopeEditor {
            scope,
            section: ScopeSection::General,
            operations: match scope {
                Scope::Folder(id) => self
                    .project
                    .folder(id)
                    .map(|f| f.operations.clone())
                    .unwrap_or_default(),
                Scope::Workspace => Default::default(),
            },
            name,
            kind,
            token,
            user,
            password,
            _subscription: subscription,
        });
        Ok(())
    }

    pub(super) fn active_scope(&self) -> Option<Scope> {
        match self.active_page {
            Some(PageTab::Settings(scope)) => Some(scope),
            _ => None,
        }
    }
    pub(super) fn active_scope_editor(&self) -> Option<&ScopeEditor> {
        let scope = self.active_scope()?;
        self.scope_editors
            .iter()
            .find(|editor| editor.scope == scope)
    }
    pub(super) fn save_scope_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.active_scope_editor() else {
            return;
        };
        let scope = editor.scope;
        let operations = editor.operations.clone();
        if operations.pre.len() > 64 || operations.post.len() > 64 {
            self.notice = "每层每阶段最多 64 项操作".into();
            cx.notify();
            return;
        }
        let name = editor.name.read(cx).value().trim().to_owned();
        if name.is_empty() {
            self.notice = "名称不能为空".into();
            cx.notify();
            return;
        }
        let mode = editor
            .kind
            .read(cx)
            .selected_value()
            .cloned()
            .unwrap_or_default();
        let auth = match mode.as_str() {
            "继承上级" => None,
            "Bearer Token" => Some(Auth::Bearer(editor.token.read(cx).value().to_string())),
            "Basic Auth" => Some(Auth::Basic {
                username: editor.user.read(cx).value().to_string(),
                password: editor.password.read(cx).value().to_string(),
            }),
            _ => Some(Auth::None),
        };
        let result = (|| -> anyhow::Result<()> {
            match scope {
                Scope::Workspace => {
                    self.project.name = name.clone();
                    self.project.auth = auth.unwrap_or(Auth::None);
                    self.project_name
                        .update(cx, |s, cx| s.set_value(name, window, cx));
                }
                Scope::Folder(id) => {
                    self.project.rename_folder(id, &name)?;
                    let folder = self
                        .project
                        .folders
                        .iter_mut()
                        .find(|f| f.id == id)
                        .unwrap();
                    folder.auth = auth;
                    folder.operations = operations;
                }
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.notice = "设置已保存，将在下次发送或连接时生效".into();
                self.dirty(cx);
            }
            Err(e) => {
                self.notice = e.to_string();
                cx.notify();
            }
        }
    }
    pub(super) fn inherited_auth_summary(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.project.effective_auth(self.current()) {
            Ok(effective) => {
                let scope = match effective.origin {
                    AuthOrigin::Folder(id) => Scope::Folder(id),
                    _ => Scope::Workspace,
                };
                v_flex().p_4().gap_3().rounded_lg().border_1().border_color(cx.theme().border)
                    .child(h_flex().gap_2().child(div().font_weight(FontWeight::MEDIUM).child(effective.auth.label()))
                        .child(div().text_sm().text_color(cx.theme().muted_foreground).child("继承")))
                    .child(div().text_sm().child(self.project.auth_origin_label(effective.origin)))
                    .child(div().text_xs().text_color(cx.theme().muted_foreground).child("凭据不会复制到此请求。移动文件夹或更改上级配置后，下次发送自动使用新配置。"))
                    .child(Button::new("edit-inherited-auth").small().ghost().label("编辑来源设置").w(px(140.))
                        .on_click(cx.listener(move|s,_,window,cx|s.open_page_tab(PageTab::Settings(scope),window,cx))))
                    .into_any_element()
            }
            Err(e) => div().text_sm().child(e.to_string()).into_any_element(),
        }
    }
    pub(super) fn scope_settings_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(editor) = self.active_scope_editor() else {
            return div().into_any_element();
        };
        let (title, path) = match editor.scope {
            Scope::Workspace => ("工作区设置", self.project.name.clone()),
            Scope::Folder(id) => ("文件夹设置", self.project.folder_path(id)),
        };
        let mode = editor
            .kind
            .read(cx)
            .selected_value()
            .map(String::as_str)
            .unwrap_or("无认证");
        let mut auth=v_flex().gap_4()
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("认证 · Auth"))
            .child(div().text_xs().text_color(cx.theme().muted_foreground)
                .child("下级选择继承时，优先使用最近文件夹的配置，再使用工作区配置。无认证会停止继续继承。"))
            .child(div().w(px(240.)).child(Select::new(&editor.kind).w_full()));
        match mode {
            "继承上级" => {
                let parent = match editor.scope {
                    Scope::Folder(id) => self.project.folder(id).ok().and_then(|f| f.parent_id),
                    Scope::Workspace => None,
                };
                auth = auth.child(match self.project.folder_auth(parent) {
                    Ok(value) => div()
                        .text_sm()
                        .child(format!(
                            "{} · {}",
                            self.project.auth_origin_label(value.origin),
                            value.auth.label()
                        ))
                        .into_any_element(),
                    Err(error) => div().text_sm().child(error.to_string()).into_any_element(),
                });
            }
            "Bearer Token" => {
                auth = auth.child(labelled("Token", Input::new(&editor.token).w_full()))
            }
            "Basic Auth" => {
                auth = auth
                    .child(labelled("用户名", Input::new(&editor.user).w_full()))
                    .child(labelled("密码", Input::new(&editor.password).w_full()))
            }
            _ => {
                auth = auth.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("此级别不生成 Authorization。下级仍可单独配置自己的认证。"),
                )
            }
        }
        let form=v_flex().max_w(px(820.)).w_full().gap_5()
            .child(v_flex().p_5().gap_4().rounded_lg().border_1().border_color(cx.theme().border)
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("基本信息"))
                .child(labelled("名称",Input::new(&editor.name).w_full())))
            .child(v_flex().p_5().rounded_lg().border_1().border_color(cx.theme().border).child(auth))
            .child(div().text_xs().text_color(cx.theme().muted_foreground)
                .child("支持 {{variable}}，发送时解析当前环境。凭据保存在当前工作区的本地数据中，建议使用环境变量引用。已运行的连接保持原配置。"))
            .child(Button::new("save-scope-settings").primary().label("保存设置").w(px(130.))
                .on_click(cx.listener(|s,_,window,cx|s.save_scope_settings(window,cx))));
        let scope = editor.scope;
        let section = editor.section;
        let mut tabs = h_flex()
            .gap_2()
            .px_6()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border);
        if matches!(scope, Scope::Folder(_)) {
            for (index, label, next) in [
                (0, "基本信息与认证", ScopeSection::General),
                (1, "前置操作", ScopeSection::PreOperations),
                (2, "后置操作", ScopeSection::PostOperations),
            ] {
                tabs = tabs.child(
                    Button::new(("folder-settings-section", index as usize))
                        .small()
                        .ghost()
                        .label(label)
                        .text_color(if section == next {
                            cx.theme().primary
                        } else {
                            cx.theme().muted_foreground
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if let Some(editor) =
                                this.scope_editors.iter_mut().find(|e| e.scope == scope)
                            {
                                editor.section = next;
                            }
                            this.reload_operation_inputs(window, cx);
                            cx.notify();
                        })),
                );
            }
        }
        let content = match section {
            ScopeSection::General => div()
                .id("scope-settings-scroll")
                .overflow_y_scroll()
                .flex_1()
                .min_h_0()
                .p_6()
                .child(form)
                .into_any_element(),
            ScopeSection::PreOperations => {
                self.operations_editor(crate::operations::Stage::Pre, cx)
            }
            ScopeSection::PostOperations => {
                self.operations_editor(crate::operations::Stage::Post, cx)
            }
        };
        v_flex()
            .flex_1()
            .min_h_0()
            .h_full()
            .child(
                h_flex()
                    .px_6()
                    .py_4()
                    .gap_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(21.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(title),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(path),
                            ),
                    )
                    .child(
                        Button::new("save-scope-settings-header")
                            .primary()
                            .label("保存设置")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.save_scope_settings(window, cx)
                            })),
                    ),
            )
            .when(matches!(scope, Scope::Folder(_)), |panel| panel.child(tabs))
            .child(content)
            .into_any_element()
    }
}
fn labelled(label: &'static str, input: impl IntoElement) -> AnyElement {
    v_flex()
        .gap_2()
        .child(div().text_sm().child(label))
        .child(input)
        .into_any_element()
}
