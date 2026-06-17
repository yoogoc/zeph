use ::setting::SettingRegistry;
use gpui::*;
use gpui_component::select::{Select, SelectDelegate, SelectEvent, SelectState};
use gpui_component::*;

pub struct LangSettings {
    select_state: Entity<SelectState<Vec<SharedString>>>,
    _subscriptions: Vec<Subscription>,
}

impl LangSettings {
    pub fn new(window: &mut Window, cx: &mut App) -> LangSettings {
        // Initial selection reflects the persisted locale.
        let locale = SettingRegistry::global(cx).read(cx).lang().to_string();
        let locales: Vec<SharedString> = i18n::available_locales()
            .iter()
            .map(|l| SharedString::from(l.clone()))
            .collect();
        let init_locale = locales.position(&SharedString::from(locale));
        let select_state = cx.new(|cx| SelectState::new(locales, init_locale, window, cx));

        let registry = SettingRegistry::global(cx);
        let ss = cx.subscribe(&select_state, move |_view, event, cx| match event {
            SelectEvent::Confirm(value) => {
                if let Some(selected_value) = value {
                    // Persist + apply via the registry (which calls i18n::set_locale).
                    registry.update(cx, |r, cx| r.set_lang(selected_value.to_string(), cx));
                    cx.refresh_windows();
                }
            }
        });

        LangSettings {
            select_state,
            _subscriptions: vec![ss],
        }
    }
}

impl Render for LangSettings {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .text_center()
            .text_2xl()
            .font_bold()
            .p_2()
            .child("I18n")
            .child(Select::new(&self.select_state))
    }
}
