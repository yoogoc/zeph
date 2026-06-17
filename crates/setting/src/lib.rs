//! Application settings: theme (light/dark + accent color) and language.
//!
//! Settings are persisted to `dirs::config_dir()/zeph/settings.json` and
//! applied to the global `gpui_component::Theme` at startup and on change.

use gpui::*;
use gpui_component::{Theme, ThemeMode};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

mod persist;

struct GlobalSettingRegistry(Entity<SettingRegistry>);
impl Global for GlobalSettingRegistry {}

impl EventEmitter<WorkspaceEvent> for SettingRegistry {}

#[derive(Clone, Debug)]
pub enum WorkspaceEvent {
    SetLang,
    SetTheme,
}

/// Light or dark base appearance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AppThemeMode {
    #[default]
    Light,
    Dark,
}

impl AppThemeMode {
    pub fn is_dark(self) -> bool {
        matches!(self, AppThemeMode::Dark)
    }
}

impl From<AppThemeMode> for ThemeMode {
    fn from(m: AppThemeMode) -> Self {
        match m {
            AppThemeMode::Light => ThemeMode::Light,
            AppThemeMode::Dark => ThemeMode::Dark,
        }
    }
}

/// The persisted settings document.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Setting {
    #[serde(default = "default_lang")]
    pub lang: String,
    #[serde(default)]
    pub theme_mode: AppThemeMode,
    /// Optional custom accent (primary) color as `0xRRGGBB`. `None` keeps the
    /// theme's default primary.
    #[serde(default)]
    pub accent: Option<u32>,
}

fn default_lang() -> String {
    "zh-CN".into()
}

impl Default for Setting {
    fn default() -> Self {
        Self {
            lang: default_lang(),
            theme_mode: AppThemeMode::Light,
            accent: None,
        }
    }
}

/// A small palette of preset accent colors offered in the UI.
pub const ACCENT_PRESETS: &[(&str, u32)] = &[
    ("Blue", 0x3b82f6),
    ("Indigo", 0x6366f1),
    ("Violet", 0x8b5cf6),
    ("Green", 0x22c55e),
    ("Orange", 0xf97316),
    ("Rose", 0xf43f5e),
];

pub fn init(cx: &mut App) {
    let setting = Setting::load();
    let registry = cx.new(|_| SettingRegistry { setting });
    cx.set_global(GlobalSettingRegistry(registry));
}

/// Applies the current settings' theme (mode + accent) to the global Theme.
/// Call after the Theme global exists (i.e. after `gpui_component` init).
pub fn apply_theme(cx: &mut App) {
    let setting = SettingRegistry::global(cx).read(cx).setting.clone();
    apply_setting_theme(&setting, None, cx);
}

/// Applies a setting's theme mode + accent to the global Theme.
fn apply_setting_theme(setting: &Setting, window: Option<&mut Window>, cx: &mut App) {
    Theme::change(ThemeMode::from(setting.theme_mode), window, cx);
    if let Some(hex) = setting.accent {
        apply_accent(hex, cx);
    }
}

/// Overrides the theme's primary color (and its hover/active/foreground
/// variants) with the given `0xRRGGBB` accent.
fn apply_accent(hex: u32, cx: &mut App) {
    let base: Hsla = rgb(hex).into();
    let theme = cx.global_mut::<Theme>();
    theme.primary = base;
    // Slightly darker on hover, darker still when active; readable foreground.
    theme.primary_hover = adjust_lightness(base, -0.06);
    theme.primary_active = adjust_lightness(base, -0.12);
    theme.primary_foreground = if base.l > 0.6 {
        rgb(0x0a0a0a).into()
    } else {
        rgb(0xffffff).into()
    };
}

fn adjust_lightness(mut c: Hsla, delta: f32) -> Hsla {
    c.l = (c.l + delta).clamp(0.0, 1.0);
    c
}

pub struct SettingRegistry {
    setting: Setting,
}

impl SettingRegistry {
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalSettingRegistry>().0.clone()
    }

    pub fn setting(&self) -> &Setting {
        &self.setting
    }

    pub fn lang(&self) -> &str {
        &self.setting.lang
    }

    pub fn theme_mode(&self) -> AppThemeMode {
        self.setting.theme_mode
    }

    pub fn accent(&self) -> Option<u32> {
        self.setting.accent
    }

    pub fn set_lang(&mut self, lang: impl Into<String>, cx: &mut Context<Self>) {
        self.setting.lang = lang.into();
        self.persist();
        cx.emit(WorkspaceEvent::SetLang);
    }

    /// Switches light/dark, applies it to the global Theme, and persists.
    pub fn set_theme_mode(
        &mut self,
        mode: AppThemeMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.setting.theme_mode = mode;
        let setting = self.setting.clone();
        apply_setting_theme(&setting, Some(window), cx);
        self.persist();
        cx.emit(WorkspaceEvent::SetTheme);
    }

    /// Sets the custom accent color (or clears it with `None`), applies, persists.
    pub fn set_accent(&mut self, accent: Option<u32>, window: &mut Window, cx: &mut Context<Self>) {
        self.setting.accent = accent;
        let setting = self.setting.clone();
        // Re-apply the full theme so clearing the accent restores the default.
        apply_setting_theme(&setting, Some(window), cx);
        self.persist();
        cx.emit(WorkspaceEvent::SetTheme);
    }

    fn persist(&self) {
        if let Err(e) = self.setting.save() {
            eprintln!("Failed to persist settings: {e}");
        }
    }
}

/// Path to the settings file: `<config_dir>/zeph/settings.json`.
pub(crate) fn settings_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("zeph")
        .join("settings.json")
}
