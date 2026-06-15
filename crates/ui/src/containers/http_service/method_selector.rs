use super::http_state::HttpState;
use gpui::*;
use gpui_component::select::{Select, SelectEvent, SelectItem, SelectState};
use gpui_component::IndexPath;
use models::{HttpConfig, HttpMethod};
use smallvec::{smallvec, SmallVec};
use std::sync::LazyLock;

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
    http_state: Entity<HttpState>,
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

        let sub = Self::wire_subscription(&select, http_state.clone(), cx);

        Self {
            http_state,
            select,
            _subscriptions: smallvec![sub],
        }
    }

    /// Wires the `Confirm` subscription that pushes the chosen method to `HttpState`.
    fn wire_subscription(
        select: &Entity<SelectState<Vec<MethodItem>>>,
        http_state: Entity<HttpState>,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe(
            select,
            move |_this, _, ev: &SelectEvent<Vec<MethodItem>>, cx| {
                if let SelectEvent::Confirm(Some(method)) = ev {
                    let method = *method;
                    http_state.update(cx, |s, cx| s.set_method(method, cx));
                }
            },
        )
    }

    /// Rebuilds the select for the given method without writing back to
    /// `HttpState`. Used by undo/redo. Rebuilding (rather than selecting) avoids
    /// firing a `Confirm` event.
    pub fn reset(&mut self, method: HttpMethod, window: &mut Window, cx: &mut Context<Self>) {
        let idx = METHODS
            .iter()
            .position(|i| i.0 == method)
            .map(IndexPath::new);
        let select = cx.new(|cx| SelectState::new(METHODS.clone(), idx, window, cx));
        let sub = Self::wire_subscription(&select, self.http_state.clone(), cx);
        self.select = select;
        self._subscriptions = smallvec![sub];
        cx.notify();
    }
}

impl Render for MethodSelector {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(100.0)).child(Select::new(&self.select))
    }
}
