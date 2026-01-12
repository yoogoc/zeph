use gpui::*;
use gpui_component::label::Label;
use gpui_component::list::{ListDelegate, ListItem, ListState};
use gpui_component::*;

#[derive(Debug, Clone, PartialEq)]
pub enum Menu {
    Request,
    I18n,
    Theme,
    Proxy,
}

impl Menu {
    fn title(&self) -> SharedString {
        match self {
            Menu::Request => shared_t!("settings.request"),
            Menu::I18n => shared_t!("settings.i18n"),
            Menu::Theme => shared_t!("settings.theme"),
            Menu::Proxy => shared_t!("settings.proxy"),
        }
    }

    fn all() -> Vec<Menu> {
        vec![Menu::Request, Menu::I18n, Menu::Theme, Menu::Proxy]
    }
}

pub struct MenuList {
    items: Vec<Menu>,
    selected_index: Option<IndexPath>,
    pub selected_item: Option<Menu>,
}

impl MenuList {
    pub fn new() -> Self {
        MenuList {
            items: Menu::all(),
            selected_index: Some(IndexPath::new(0)),
            selected_item: Some(Menu::Request),
        }
    }
}

impl ListDelegate for MenuList {
    type Item = ListItem;

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.items.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        self.items.get(ix.row).map(|item| {
            ListItem::new(ix)
                .child(
                    Label::new(item.title())
                        .h_flex()
                        .items_center()
                        .justify_center(),
                )
                .selected(Some(ix) == self.selected_index)
        })
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix;
        self.selected_item = ix.and_then(|ix| self.items.get(ix.row).cloned());
        cx.notify();
    }
}
