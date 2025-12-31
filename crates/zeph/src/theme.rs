use gpui::{rgb, App};
use gpui_component::Theme;

pub fn init(cx: &mut App) {
    Theme::global_mut(cx).border = rgb(0xf5f5f5).into();
}