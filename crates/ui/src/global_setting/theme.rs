//! Theme settings panel: light/dark mode + accent color.
//!
//! Reads/writes the persisted `setting::SettingRegistry`, which applies changes
//! to the global `gpui_component::Theme` and saves them to disk.

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{h_flex, v_flex, ActiveTheme, Sizable, StyledExt};
use setting::{AppThemeMode, SettingRegistry, ACCENT_PRESETS};

pub struct ThemeSettings {
    registry: Entity<SettingRegistry>,
}

impl ThemeSettings {
    pub fn new(cx: &mut App) -> Self {
        Self {
            registry: SettingRegistry::global(cx),
        }
    }

    fn set_mode(&self, mode: AppThemeMode, window: &mut Window, cx: &mut Context<Self>) {
        self.registry
            .update(cx, |r, cx| r.set_theme_mode(mode, window, cx));
        cx.refresh_windows();
    }

    fn set_accent(&self, accent: Option<u32>, window: &mut Window, cx: &mut Context<Self>) {
        self.registry
            .update(cx, |r, cx| r.set_accent(accent, window, cx));
        cx.refresh_windows();
    }
}

impl Render for ThemeSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current_mode = self.registry.read(cx).theme_mode();
        let current_accent = self.registry.read(cx).accent();

        let section = |title: SharedString| {
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().muted_foreground)
                .child(title)
        };

        let mode_button = |label: SharedString, mode: AppThemeMode| {
            let active = current_mode == mode;
            Button::new(SharedString::from(format!("theme_mode_{}", mode.is_dark())))
                .label(label)
                .small()
                .when(active, |b| b.primary())
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.set_mode(mode, window, cx);
                }))
        };

        // An accent swatch: a colored circle, ringed when selected.
        let swatch = |name: &'static str, hex: u32| {
            let selected = current_accent == Some(hex);
            div()
                .id(SharedString::from(format!("accent_{name}")))
                .size(px(28.))
                .rounded_full()
                .bg(rgb(hex))
                .cursor_pointer()
                .border_2()
                .border_color(if selected {
                    cx.theme().foreground
                } else {
                    cx.theme().border
                })
                .hover(|s| s.opacity(0.85))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.set_accent(Some(hex), window, cx);
                }))
        };

        v_flex()
            .p_6()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .font_bold()
                    .child(shared_t!("settings.theme")),
            )
            // Appearance: light / dark
            .child(
                v_flex()
                    .gap_2()
                    .child(section(shared_t!("settings.theme_appearance")))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(mode_button(
                                shared_t!("settings.theme_light"),
                                AppThemeMode::Light,
                            ))
                            .child(mode_button(
                                shared_t!("settings.theme_dark"),
                                AppThemeMode::Dark,
                            )),
                    ),
            )
            // Accent color presets + "default"
            .child(
                v_flex()
                    .gap_2()
                    .child(section(shared_t!("settings.theme_accent")))
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .children(ACCENT_PRESETS.iter().map(|(name, hex)| swatch(name, *hex)))
                            .child(
                                Button::new("accent_default")
                                    .label(shared_t!("settings.theme_accent_default"))
                                    .small()
                                    .ghost()
                                    .when(current_accent.is_none(), |b| b.primary())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.set_accent(None, window, cx);
                                    })),
                            ),
                    ),
            )
    }
}
