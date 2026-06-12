use super::http_state::HttpState;
use gpui::*;
use gpui_component::select::{Select, SelectEvent, SelectItem, SelectState};
use gpui_component::IndexPath;
use models::HttpMethod;

#[derive(Clone)]
struct MethodItem(HttpMethod);

impl SelectItem for MethodItem {
    type Value = HttpMethod;
    fn title(&self) -> SharedString {
        self.0.as_str().into()
    }
    fn value(&self) -> &Self::Value {
        &self.0
    }
}

pub struct MethodSelector {
    select: Entity<SelectState<Vec<MethodItem>>>,
}

impl MethodSelector {
    pub fn new(
        method: HttpMethod,
        http_state: Entity<HttpState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let items = vec![
            MethodItem(HttpMethod::Get),
            MethodItem(HttpMethod::Post),
            MethodItem(HttpMethod::Put),
            MethodItem(HttpMethod::Delete),
            MethodItem(HttpMethod::Patch),
            MethodItem(HttpMethod::Head),
            MethodItem(HttpMethod::Options),
        ];
        let idx = items.iter().position(|i| i.0 == method).map(IndexPath::new);
        let select = cx.new(|cx| SelectState::new(items, idx, window, cx));

        cx.subscribe(
            &select,
            move |_this, _, ev: &SelectEvent<Vec<MethodItem>>, cx| {
                if let SelectEvent::Confirm(Some(_)) = ev {
                    http_state.update(cx, |s, cx| s.on_input_changed(cx));
                }
            },
        )
        .detach();

        Self { select }
    }

    pub fn method(&self, cx: &App) -> HttpMethod {
        self.select
            .read(cx)
            .selected_value()
            .copied()
            .unwrap_or(HttpMethod::Get)
    }
}

impl Render for MethodSelector {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(100.0)).child(Select::new(&self.select))
    }
}
