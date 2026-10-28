//! Shared application settings and updater state; workspace requests stay alive behind this page.
use crate::{
    settings::{self, Appearance, Preferences},
    updater::{self, Downloaded, Installation, PreparedInstall, Release},
};
use gpui_kit::{
    component::{
        ActiveTheme, Disableable, Selectable, Theme, ThemeMode, WindowExt,
        button::{Button, ButtonVariants},
        checkbox::Checkbox,
        dialog::DialogButtonProps,
        h_flex,
        input::{Input, InputState},
        v_flex,
    },
    prelude::FluentBuilder,
    *,
};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};
use tokio::sync::mpsc;

pub const RELEASES_URL: &str = "https://github.com/yoogoc/zeph/releases";
gpui_kit::actions!(zeph, [OpenSettings, CheckUpdates]);
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Appearance,
    Requests,
    Updates,
    About,
}
impl Page {
    fn title(self) -> &'static str {
        match self {
            Self::Appearance => "外观",
            Self::Requests => "请求默认值",
            Self::Updates => "应用更新",
            Self::About => "关于 Zeph",
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Idle,
    Checking,
    Current,
    Available,
    Downloading,
    Ready,
    Preparing,
    Failed,
}
pub struct RestartRequested;
struct Shared(Entity<ApplicationSettings>);
impl Global for Shared {}
pub fn maybe_store(cx: &App) -> Option<Entity<ApplicationSettings>> {
    cx.try_global::<Shared>().map(|s| s.0.clone())
}
pub fn store(cx: &App) -> Entity<ApplicationSettings> {
    cx.global::<Shared>().0.clone()
}
pub fn open(page: Option<Page>, cx: &mut App) {
    store(cx).update(cx, |s, cx| {
        s.open = true;
        if let Some(page) = page {
            s.page = page;
        }
        cx.notify();
    });
}
pub fn open_updates(cx: &mut App) {
    open(Some(Page::Updates), cx);
    store(cx).update(cx, |s, cx| s.check(true, cx));
}

enum Event {
    Receipt(Option<String>),
    Checked(u64, bool, Box<Result<Option<Release>, String>>),
    Progress(u64, u64, u64),
    Downloaded(u64, Result<Arc<Downloaded>, String>),
    Prepared(u64, Result<PreparedInstall, String>),
    Credential(bool, Result<(), String>),
}
pub struct ApplicationSettings {
    pub open: bool,
    pub preferences: Preferences,
    pub notice: String,
    page: Page,
    path: PathBuf,
    runtime: Arc<tokio::runtime::Runtime>,
    installation: Installation,
    status: Status,
    available: Option<Release>,
    ready: Option<Arc<Downloaded>>,
    prepared: Option<PreparedInstall>,
    progress: (u64, u64),
    error: String,
    last_checked: Option<SystemTime>,
    last_attempt: Option<Instant>,
    generation: u64,
    worker: Option<tokio::task::JoinHandle<()>>,
    tx: mpsc::Sender<Event>,
    rx: mpsc::Receiver<Event>,
    timeout: Entity<InputState>,
    proxy: Entity<InputState>,
    token: Entity<InputState>,
    credential_busy: bool,
    _appearance: Subscription,
    _poll: Task<()>,
}
impl EventEmitter<RestartRequested> for ApplicationSettings {}
impl ApplicationSettings {
    pub fn init(
        path: PathBuf,
        preferences: Preferences,
        runtime: Arc<tokio::runtime::Runtime>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let state = cx.new(|cx| Self::new(path, preferences, runtime, window, cx));
        cx.set_global(Shared(state.clone()));
        state.update(cx, |s, cx| s.apply_appearance(window, cx));
    }
    fn new(
        path: PathBuf,
        preferences: Preferences,
        runtime: Arc<tokio::runtime::Runtime>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (tx, rx) = mpsc::channel(64);
        let receipt_tx = tx.clone();
        let cache = path.parent().unwrap().join("updates");
        runtime.spawn_blocking(move || {
            let _ = receipt_tx.blocking_send(Event::Receipt(updater::read_install_result(&cache)));
        });
        let timeout = cx.new(|cx| {
            InputState::new(window, cx).default_value(preferences.requests.timeout_secs.to_string())
        });
        let proxy = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("留空以使用系统代理")
                .default_value(preferences.requests.proxy.clone())
        });
        let token = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("GitHub Token，仅保存到系统钥匙串")
        });
        let observer =
            cx.observe_window_appearance(window, |s, window, cx| s.apply_appearance(window, cx));
        let poll = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(120))
                    .await;
                if this
                    .update_in(cx, |s, window, cx| s.poll(window, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            open: false,
            preferences,
            page: Page::Appearance,
            path,
            runtime,
            installation: Installation::detect(),
            status: Status::Idle,
            available: None,
            ready: None,
            prepared: None,
            progress: (0, 0),
            error: String::new(),
            notice: String::new(),
            last_checked: None,
            last_attempt: None,
            generation: 0,
            worker: None,
            tx,
            rx,
            timeout,
            proxy,
            token,
            credential_busy: false,
            _appearance: observer,
            _poll: poll,
        }
    }
    fn apply_appearance(&self, window: &mut Window, cx: &mut App) {
        match self.preferences.appearance {
            Appearance::System => Theme::sync_system_appearance(Some(window), cx),
            Appearance::Light => Theme::change(ThemeMode::Light, Some(window), cx),
            Appearance::Dark => Theme::change(ThemeMode::Dark, Some(window), cx),
        }
        crate::ui::apply_theme(cx);
    }
    fn persist(&mut self, previous: Preferences, cx: &mut Context<Self>) -> bool {
        match self.preferences.save(&self.path) {
            Ok(()) => {
                self.notice = "配置已保存".into();
                cx.notify();
                true
            }
            Err(e) => {
                self.preferences = previous;
                self.notice = format!("配置未保存：{e}");
                cx.notify();
                false
            }
        }
    }
    pub fn set_appearance(
        &mut self,
        appearance: Appearance,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous = self.preferences.clone();
        self.preferences.appearance = appearance;
        if self.persist(previous, cx) {
            self.apply_appearance(window, cx);
        }
    }
    fn cancel(&mut self, cx: &mut Context<Self>) {
        if let Some(job) = self.worker.take() {
            job.abort();
        }
        self.generation = self.generation.wrapping_add(1);
        self.ready = None;
        self.prepared = None;
        self.status = if self.available.is_some() {
            Status::Available
        } else {
            Status::Idle
        };
        cx.notify();
    }
    fn proxy(&self) -> updater::Proxy {
        if self.preferences.requests.proxy.is_empty() {
            updater::Proxy::System
        } else {
            updater::Proxy::Custom(self.preferences.requests.proxy.clone())
        }
    }
    fn check(&mut self, manual: bool, cx: &mut Context<Self>) {
        if matches!(
            self.status,
            Status::Checking | Status::Downloading | Status::Preparing | Status::Ready
        ) {
            return;
        }
        self.cancel(cx);
        self.available = None;
        self.status = Status::Checking;
        self.error.clear();
        self.last_attempt = Some(Instant::now());
        let (generation, tx, channel, installation, proxy, token) = (
            self.generation,
            self.tx.clone(),
            self.preferences.updates.channel,
            self.installation.clone(),
            self.proxy(),
            self.preferences.github_token_configured,
        );
        self.worker = Some(self.runtime.spawn(async move {
            let result = async {
                let token = if token {
                    Some(tokio::task::spawn_blocking(settings::read_token).await??)
                } else {
                    None
                };
                updater::Client::new(proxy, updater::PUBLIC_KEY)?
                    .with_token(token)
                    .check(env!("CARGO_PKG_VERSION"), channel, &installation)
                    .await
            }
            .await
            .map_err(|e: anyhow::Error| e.to_string());
            let _ = tx
                .send(Event::Checked(generation, manual, Box::new(result)))
                .await;
        }));
        cx.notify();
    }
    fn download(&mut self, cx: &mut Context<Self>) {
        if !self.installation.can_install() {
            cx.open_url(RELEASES_URL);
            return;
        }
        let Some(release) = self.available.clone() else {
            return;
        };
        self.cancel(cx);
        self.status = Status::Downloading;
        self.progress = (0, release.asset.size);
        let (generation, tx, proxy, token, cache) = (
            self.generation,
            self.tx.clone(),
            self.proxy(),
            self.preferences.github_token_configured,
            self.path.parent().unwrap().join("updates"),
        );
        self.worker = Some(self.runtime.spawn(async move {
            let result = async {
                let token = if token {
                    Some(tokio::task::spawn_blocking(settings::read_token).await??)
                } else {
                    None
                };
                let progress = tx.clone();
                updater::Client::new(proxy, updater::PUBLIC_KEY)?
                    .with_token(token)
                    .download(release, cache, move |n, total| {
                        let _ = progress.try_send(Event::Progress(generation, n, total));
                    })
                    .await
                    .map(Arc::new)
            }
            .await
            .map_err(|e: anyhow::Error| e.to_string());
            let _ = tx.send(Event::Downloaded(generation, result)).await;
        }));
        cx.notify();
    }
    fn prepare(&mut self, cx: &mut Context<Self>) {
        let Some(download) = self.ready.clone() else {
            return;
        };
        self.status = Status::Preparing;
        let (tx, generation, installation) =
            (self.tx.clone(), self.generation, self.installation.clone());
        self.worker = Some(self.runtime.spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                updater::prepare_install(download, installation)
            })
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r.map_err(|e| e.to_string()));
            let _ = tx.send(Event::Prepared(generation, result)).await;
        }));
        cx.notify();
    }
    /// Called after every workspace and the catalog have saved successfully.
    pub fn launch_install(&mut self) -> anyhow::Result<()> {
        self.prepared
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("更新尚未准备完成"))?
            .launch()
    }
    pub fn installation_error(&mut self, message: String, cx: &mut Context<Self>) {
        self.prepared = None;
        self.status = if self.ready.is_some() {
            Status::Ready
        } else {
            Status::Failed
        };
        self.notice = message;
        cx.notify();
    }
    fn credentials(&mut self, remove: bool, cx: &mut Context<Self>) {
        if self.credential_busy {
            return;
        }
        let token = zeroize::Zeroizing::new(self.token.read(cx).value().to_string());
        if !remove && token.trim().is_empty() {
            self.notice = "请输入 GitHub Token".into();
            cx.notify();
            return;
        }
        self.credential_busy = true;
        let tx = self.tx.clone();
        self.runtime.spawn_blocking(move || {
            let result = settings::update_token(if remove { None } else { Some(token.as_str()) })
                .map_err(|e| e.to_string());
            let _ = tx.blocking_send(Event::Credential(!remove, result));
        });
        cx.notify();
    }
    fn poll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Receipt(Some(message)) => self.notice = message,
                Event::Receipt(None) => {}
                Event::Credential(configured, result) => {
                    self.credential_busy = false;
                    match result {
                        Ok(()) => {
                            let prev = self.preferences.clone();
                            self.preferences.github_token_configured = configured;
                            self.persist(prev, cx);
                            self.token.update(cx, |s, cx| s.set_value("", window, cx));
                            self.cancel(cx);
                        }
                        Err(e) => self.notice = e,
                    }
                }
                Event::Checked(g, manual, result) if g == self.generation => {
                    self.worker = None;
                    self.last_checked = Some(SystemTime::now());
                    match *result {
                        Ok(Some(r)) => {
                            self.available = Some(r);
                            self.status = Status::Available;
                            if self.preferences.updates.auto_download
                                && self.installation.can_install()
                            {
                                self.download(cx);
                            }
                        }
                        Ok(None) => self.status = Status::Current,
                        Err(e) => {
                            if manual {
                                self.status = Status::Failed;
                                self.error = e;
                            } else {
                                log::info!("自动更新检查未完成：{e}");
                                self.status = Status::Idle;
                            }
                        }
                    }
                }
                Event::Progress(g, n, total) if g == self.generation => self.progress = (n, total),
                Event::Downloaded(g, result) if g == self.generation => {
                    self.worker = None;
                    match result {
                        Ok(d) => {
                            self.ready = Some(d);
                            self.status = Status::Ready;
                        }
                        Err(e) => {
                            self.error = e;
                            self.status = Status::Failed;
                        }
                    }
                }
                Event::Prepared(g, result) if g == self.generation => {
                    self.worker = None;
                    match result {
                        Ok(p) => {
                            self.prepared = Some(p);
                            cx.emit(RestartRequested);
                        }
                        Err(e) => {
                            self.error = e;
                            self.status = Status::Failed;
                        }
                    }
                }
                _ => {}
            }
            cx.notify();
        }
        if self.preferences.updates.auto_check
            && self
                .last_attempt
                .is_none_or(|t| t.elapsed() > Duration::from_secs(3600))
            && matches!(self.status, Status::Idle | Status::Current | Status::Failed)
        {
            self.check(false, cx);
        }
    }
    fn save_request_defaults(&mut self, cx: &mut Context<Self>) {
        let Ok(timeout) = self.timeout.read(cx).value().parse() else {
            self.notice = "请求超时必须为 1–3600 秒".into();
            cx.notify();
            return;
        };
        let previous = self.preferences.clone();
        self.preferences.requests.timeout_secs = timeout;
        self.preferences.requests.proxy = self.proxy.read(cx).value().trim().into();
        self.persist(previous, cx);
    }
    fn content(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut content = v_flex().gap_5().max_w(px(880.)).w_full();
        match self.page {
            Page::Appearance => {
                let mut buttons = h_flex().gap_2();
                for (appearance, label) in [
                    (Appearance::System, "跟随系统"),
                    (Appearance::Light, "浅色"),
                    (Appearance::Dark, "深色"),
                ] {
                    buttons = buttons.child(
                        Button::new(SharedString::from(format!("appearance-{label}")))
                            .label(label)
                            .selected(self.preferences.appearance == appearance)
                            .on_click(cx.listener(move |s, _, window, cx| {
                                s.set_appearance(appearance, window, cx)
                            })),
                    );
                }
                content = content.child(card(
                    "主题",
                    "应用到所有工作区，重新启动后保留。",
                    buttons,
                    cx,
                ));
            }
            Page::Requests => {
                content = content.child(card(
                    "新请求默认值",
                    "只影响之后新建的请求；已有调试配置保持独立。",
                    v_flex()
                        .gap_4()
                        .child(row("请求超时", Input::new(&self.timeout).w(px(130.)), cx))
                        .child(row(
                            "校验 TLS 证书",
                            Checkbox::new("default-tls")
                                .checked(self.preferences.requests.verify_tls)
                                .on_click(cx.listener(|s, value, _, cx| {
                                    let p = s.preferences.clone();
                                    s.preferences.requests.verify_tls = *value;
                                    s.persist(p, cx);
                                })),
                            cx,
                        ))
                        .child(row("代理地址", Input::new(&self.proxy).w(px(320.)), cx))
                        .child(
                            Button::new("save-request-defaults")
                                .primary()
                                .label("保存默认值")
                                .on_click(cx.listener(|s, _, _, cx| s.save_request_defaults(cx))),
                        ),
                    cx,
                ));
            }
            Page::Updates => {
                let title = match self.status {
                    Status::Idle => "检查 Zeph 更新",
                    Status::Checking => "正在检查更新…",
                    Status::Current => "未发现更新版本",
                    Status::Available => "发现新版本",
                    Status::Downloading => "正在下载更新…",
                    Status::Ready => "更新已下载并通过验签",
                    Status::Preparing => "正在准备重启…",
                    Status::Failed => "更新未完成",
                };
                let mut status = v_flex().gap_3().child(
                    div()
                        .text_size(px(18.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                );
                status = status.child(hint(
                    format!(
                        "当前版本 {} · {} / {}",
                        env!("CARGO_PKG_VERSION"),
                        std::env::consts::OS,
                        std::env::consts::ARCH
                    ),
                    cx,
                ));
                if let Some(release) = &self.available {
                    status =
                        status.child(div().text_sm().child(format!("新版本 {}", release.version)));
                    if !release.notes.is_empty() {
                        status = status.child(
                            div()
                                .id("release-notes")
                                .max_h(px(180.))
                                .overflow_y_scroll()
                                .text_sm()
                                .child(release.notes.chars().take(16000).collect::<String>()),
                        );
                    }
                }
                if self.status == Status::Downloading {
                    let (n, total) = self.progress;
                    status = status.child(hint(
                        format!(
                            "{:.1} / {:.1} MiB",
                            n as f64 / 1048576.,
                            total as f64 / 1048576.
                        ),
                        cx,
                    ));
                }
                if !self.error.is_empty() && self.status == Status::Failed {
                    status = status.child(div().text_sm().child(self.error.clone()));
                }
                if let Some(time) = self.last_checked {
                    let t: chrono::DateTime<chrono::Local> = time.into();
                    status =
                        status.child(hint(format!("上次检查 {}", t.format("%m-%d %H:%M")), cx));
                }
                let mut actions = h_flex().gap_2().child(
                    Button::new("check-updates")
                        .label("检查更新")
                        .disabled(matches!(
                            self.status,
                            Status::Checking
                                | Status::Downloading
                                | Status::Preparing
                                | Status::Ready
                        ))
                        .on_click(cx.listener(|s, _, _, cx| s.check(true, cx))),
                );
                if matches!(self.status, Status::Available | Status::Failed)
                    && self.available.is_some()
                {
                    actions = actions.child(
                        Button::new("download-update")
                            .primary()
                            .label(if self.installation.can_install() {
                                "下载更新"
                            } else {
                                "前往下载"
                            })
                            .on_click(cx.listener(|s, _, _, cx| s.download(cx))),
                    );
                }
                if matches!(self.status, Status::Checking | Status::Downloading) {
                    actions = actions.child(
                        Button::new("cancel-update")
                            .label("取消")
                            .on_click(cx.listener(|s, _, _, cx| s.cancel(cx))),
                    );
                }
                if self.status == Status::Ready {
                    actions=actions.child(Button::new("install-update").primary().label("重启并安装").on_click(cx.listener(|s,_,window,cx|{
                        let view=cx.entity().downgrade();let _=s;
                        window.open_dialog(cx,move |d,_,_|{let view=view.clone();d.title("安装 Zeph 更新")
                            .child("将保存所有工作区并重启 Zeph。运行中的 HTTP、gRPC 和 WebSocket 请求会停止。")
                            .button_props(DialogButtonProps::default().ok_text("保存并重启").cancel_text("暂不安装").show_cancel(true))
                            .on_ok(move |_,_,cx|{let _=view.update(cx,|s,cx|s.prepare(cx));true})});
                    })));
                }
                status = status.child(actions).child(hint(
                    if self.installation.can_install() {
                        "更新包与更新清单均经过签名验证；安装前会请求确认。"
                    } else {
                        "当前为源码或包管理器安装，可检查版本并前往 Releases 下载。"
                    },
                    cx,
                ));
                content = content.child(card("版本更新", "", status, cx));
                let mut channels = h_flex().gap_2();
                for (channel, label) in [
                    (updater::Channel::Stable, "稳定版"),
                    (updater::Channel::Development, "开发版"),
                ] {
                    channels = channels.child(
                        Button::new(SharedString::from(format!("channel-{label}")))
                            .label(label)
                            .selected(self.preferences.updates.channel == channel)
                            .disabled(self.status == Status::Preparing)
                            .on_click(cx.listener(move |s, _, _, cx| {
                                let p = s.preferences.clone();
                                s.preferences.updates.channel = channel;
                                if s.persist(p, cx) {
                                    s.cancel(cx);
                                    s.available = None;
                                    s.check(true, cx);
                                }
                            })),
                    );
                }
                content = content.child(card(
                    "更新偏好",
                    "后台每小时检查；自动检查失败不会打断操作。",
                    v_flex()
                        .gap_4()
                        .child(row("更新通道", channels, cx))
                        .child(row(
                            "自动检查更新",
                            Checkbox::new("auto-check-updates")
                                .checked(self.preferences.updates.auto_check)
                                .on_click(cx.listener(|s, v, _, cx| {
                                    let p = s.preferences.clone();
                                    s.preferences.updates.auto_check = *v;
                                    s.persist(p, cx);
                                })),
                            cx,
                        ))
                        .child(row(
                            "自动下载更新",
                            Checkbox::new("auto-download-updates")
                                .checked(self.preferences.updates.auto_download)
                                .on_click(cx.listener(|s, v, _, cx| {
                                    let p = s.preferences.clone();
                                    s.preferences.updates.auto_download = *v;
                                    if s.persist(p, cx) && *v && s.status == Status::Available {
                                        s.download(cx);
                                    }
                                })),
                            cx,
                        )),
                    cx,
                ));
                content=content.child(card("GitHub 访问（可选）","公开更新无需 Token。只有需要更高的 GitHub API 额度时才需配置；Token 仅保存到系统钥匙串。",v_flex().gap_3()
                    .child(hint(if self.preferences.github_token_configured{"已保存 GitHub Token"}else{"未配置 GitHub Token"},cx))
                    .child(Input::new(&self.token).w_full())
                    .child(h_flex().gap_2().child(Button::new("save-update-token").label("保存 Token").disabled(self.credential_busy).on_click(cx.listener(|s,_,_,cx|s.credentials(false,cx))))
                        .child(Button::new("remove-update-token").label("移除 Token").disabled(self.credential_busy||!self.preferences.github_token_configured).on_click(cx.listener(|s,_,_,cx|s.credentials(true,cx))))),cx));
            }
            Page::About => {
                content = content.child(card(
                    "Zeph",
                    "本地 API 工作台",
                    v_flex()
                        .gap_3()
                        .child(
                            div()
                                .text_size(px(24.))
                                .font_weight(FontWeight::BOLD)
                                .child(format!("Zeph {}", env!("CARGO_PKG_VERSION"))),
                        )
                        .child(hint("HTTP · gRPC · WebSocket · OpenAPI · Apifox", cx))
                        .child(hint(
                            "请求与工作区保存在本地。更新检查仅访问 GitHub，不上传请求数据。",
                            cx,
                        ))
                        .child(hint(format!("应用配置：{}", self.path.display()), cx))
                        .child(
                            Button::new("open-repository")
                                .label("GitHub · yoogoc/zeph")
                                .on_click(|_, _, cx| cx.open_url("https://github.com/yoogoc/zeph")),
                        ),
                    cx,
                ));
            }
        }
        content.into_any_element()
    }
}
impl Drop for ApplicationSettings {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            worker.abort();
        }
    }
}
impl Render for ApplicationSettings {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut nav = v_flex()
            .w(px(190.))
            .flex_shrink_0()
            .h_full()
            .p_4()
            .gap_2()
            .border_r_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().sidebar)
            .child(
                div()
                    .px_2()
                    .py_3()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("应用设置"),
            );
        for page in [Page::Appearance, Page::Requests, Page::Updates, Page::About] {
            nav = nav.child(
                Button::new(SharedString::from(format!("settings-{}", page.title())))
                    .ghost()
                    .w_full()
                    .justify_start()
                    .label(page.title())
                    .selected(self.page == page)
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.page = page;
                        s.notice.clear();
                        cx.notify();
                    })),
            );
        }
        h_flex().size_full().min_h_0().child(nav).child(
            v_flex()
                .flex_1()
                .h_full()
                .min_w_0()
                .child(
                    h_flex()
                        .px_8()
                        .py_5()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(22.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(self.page.title()),
                        )
                        .child(
                            Button::new("close-application-settings")
                                .ghost()
                                .label("返回工作台")
                                .on_click(cx.listener(|s, _, _, cx| {
                                    s.open = false;
                                    cx.notify();
                                })),
                        ),
                )
                .when(!self.notice.is_empty(), |v| {
                    v.child(div().px_8().py_2().text_sm().child(self.notice.clone()))
                })
                .child(
                    div()
                        .id("application-settings-scroll")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .p_8()
                        .child(self.content(cx)),
                ),
        )
    }
}
fn hint(text: impl Into<SharedString>, cx: &App) -> AnyElement {
    div()
        .text_size(px(12.))
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
        .into_any_element()
}
fn row(label: &str, control: impl IntoElement, cx: &App) -> AnyElement {
    h_flex()
        .gap_4()
        .items_center()
        .child(
            div()
                .w(px(160.))
                .text_sm()
                .text_color(cx.theme().foreground)
                .child(label.to_owned()),
        )
        .child(control)
        .into_any_element()
}
fn card(title: &str, description: &str, body: impl IntoElement, cx: &App) -> AnyElement {
    v_flex()
        .gap_4()
        .p_5()
        .rounded(px(10.))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
        .child(
            v_flex()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title.to_owned()),
                )
                .when(!description.is_empty(), |v| {
                    v.child(hint(description.to_owned(), cx))
                }),
        )
        .child(body)
        .into_any_element()
}
