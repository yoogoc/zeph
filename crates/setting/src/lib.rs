use gpui::*;

pub fn init(cx: &mut App) {
    SettingRegistry::set_global(cx.new(SettingRegistry::new), cx);
}

struct GlobalSettingRegistry(Entity<SettingRegistry>);
impl Global for GlobalSettingRegistry {}

impl EventEmitter<WorkspaceEvent> for SettingRegistry {}
#[derive(Clone, Debug)]
pub enum WorkspaceEvent {
    SetLang,
}

pub struct Setting {
    pub lang: SharedString,
    pub theme: ThemeSetting,
}

pub struct ThemeSetting {
    pub is_dark: bool,
    pub theme: Theme,
}

pub enum Theme {
    Prefab,
    Custom,
}

pub struct SettingRegistry {
    setting: Setting,
}

impl SettingRegistry {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            setting: Setting {
                lang: "zh-CN".into(),
                theme: ThemeSetting {
                    is_dark: false,
                    theme: Theme::Prefab,
                },
            },
        }
    }
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalSettingRegistry>().0.clone()
    }

    fn set_global(registry: Entity<Self>, cx: &mut App) {
        cx.set_global(GlobalSettingRegistry(registry));
    }

    pub fn set_lang(&mut self, lang: SharedString, cx: &mut Context<Self>) {
        self.setting.lang = lang;
        cx.emit(WorkspaceEvent::SetLang);
    }
}
