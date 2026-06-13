use super::http_state::HttpState;
use gpui::*;
use gpui_component::select::{Select, SelectEvent, SelectItem, SelectState};
use gpui_component::IndexPath;
use models::{HttpConfig, HttpMethod};
use std::sync::LazyLock;
use smallvec::{smallvec, SmallVec};

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

    _subscriptions: SmallVec<[Subscription; 1]>,
}

static METHODS: LazyLock<Vec<MethodItem>> = LazyLock::new(|| {
    vec![
        MethodItem(HttpMethod::Get),
        MethodItem(HttpMethod::Post),
        MethodItem(HttpMethod::Put),
        MethodItem(HttpMethod::Delete),
        MethodItem(HttpMethod::Patch),
        MethodItem(HttpMethod::Head),
        MethodItem(HttpMethod::Options),
    ]
});

impl MethodSelector {
    pub fn new(http_state: Entity<HttpState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let config = match &http_state.read(cx).service.service_type {
            models::ServiceType::Http(config) => config.clone(),
            _ => HttpConfig::default(),
        };

        let idx = METHODS
            .iter()
            .position(|i| i.0 == config.method)
            .map(IndexPath::new);
        let select = cx.new(|cx| SelectState::new(METHODS.clone(), idx, window, cx));

        let sub = cx.subscribe(
            &select,
            move |_this, _, ev: &SelectEvent<Vec<MethodItem>>, cx| {
                if let SelectEvent::Confirm(Some(method)) = ev {
                    let method = *method;
                    http_state.update(cx, |s, cx| s.set_method(method, cx));
                }
            },
        );

        Self { select, _subscriptions: smallvec![sub]}
    }
}

impl Render for MethodSelector {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(100.0)).child(Select::new(&self.select))
    }
}
