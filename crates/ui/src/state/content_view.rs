//! Content view abstraction for the workspace content area.
//!
//! Mirrors the `Panel` / `PanelView` pattern from `gpui-component`'s dock:
//!
//! - [`ContentView`] is the *business* trait, implemented by concrete view
//!   types (`HttpService`, `FolderDetailPage`, `PlaceholderView`). Its methods
//!   take `&mut Context<Self>`, so they are written like normal entity methods.
//! - [`ContentViewHandle`] is the *object-safe* dispatch trait. Its methods only
//!   use `&App` / `&mut App`, so `Box<dyn ContentViewHandle>` can be stored and
//!   driven generically. `impl<T: ContentView> ContentViewHandle for Entity<T>`
//!   bridges the two by `read`/`update`-ing the entity.
//!
//! This lets `ServiceState` cache heterogeneous open views as
//! `Box<dyn ContentViewHandle>` while still rendering them (`view()` →
//! `AnyView`) and invoking lifecycle methods (`is_modified`/`save`).

use gpui::*;
use uuid::Uuid;

/// Business trait implemented by concrete content view types.
#[allow(unused_variables)]
pub trait ContentView: Render {
    fn id(&self) -> Uuid;
    /// Title shown on the content's tab.
    fn title(&self) -> SharedString;

    /// Icon element shown as the tab prefix.
    fn tab_icon(&self) -> AnyElement;

    /// Updates the title shown on the tab (e.g. after a rename).
    fn set_title(&mut self, title: SharedString, cx: &mut Context<Self>) {}

    /// Whether the view has unsaved changes.
    fn is_modified(&self, cx: &App) -> bool {
        false
    }

    /// Persists any pending changes.
    fn save(&mut self, cx: &mut Context<Self>) {}

    /// Called right before the view is removed from the workspace.
    fn on_close(&mut self, cx: &mut Context<Self>) {}
}

/// Object-safe handle over a content view entity, used for type-erased storage.
pub trait ContentViewHandle: 'static {
    fn id(&self, cx: &App) -> Uuid;
    fn title(&self, cx: &App) -> SharedString;
    fn tab_icon(&self, cx: &App) -> AnyElement;
    fn set_title(&self, title: SharedString, cx: &mut App);
    fn is_modified(&self, cx: &App) -> bool;
    fn save(&self, cx: &mut App);
    fn on_close(&self, cx: &mut App);
    fn view(&self) -> AnyView;
}

impl<T: ContentView> ContentViewHandle for Entity<T> {
    fn id(&self, cx: &App) -> Uuid {
        self.read(cx).id()
    }

    fn title(&self, cx: &App) -> SharedString {
        self.read(cx).title()
    }

    fn tab_icon(&self, cx: &App) -> AnyElement {
        self.read(cx).tab_icon()
    }

    fn set_title(&self, title: SharedString, cx: &mut App) {
        self.update(cx, |this, cx| this.set_title(title, cx));
    }

    fn is_modified(&self, cx: &App) -> bool {
        self.read(cx).is_modified(cx)
    }

    fn save(&self, cx: &mut App) {
        self.update(cx, |this, cx| this.save(cx));
    }

    fn on_close(&self, cx: &mut App) {
        self.update(cx, |this, cx| this.on_close(cx));
    }

    fn view(&self) -> AnyView {
        self.clone().into()
    }
}

/// A placeholder view for content types without a real implementation yet
/// (gRPC, WebSocket, Schema, ...).
pub struct PlaceholderView {
    id: Uuid,
    title: SharedString,
    body: SharedString,
    icon: gpui_component::IconName,
}

impl PlaceholderView {
    pub fn new(
        title: impl Into<SharedString>,
        body: impl Into<SharedString>,
        icon: gpui_component::IconName,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            body: body.into(),
            icon,
        }
    }
}

impl Render for PlaceholderView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_4()
            .items_center()
            .justify_center()
            .child(self.body.clone())
    }
}

impl ContentView for PlaceholderView {
    fn id(&self) -> Uuid {
        self.id
    }
    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn tab_icon(&self) -> AnyElement {
        self.icon.clone().into_any_element()
    }
}
