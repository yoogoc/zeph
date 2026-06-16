//! Editable gRPC request metadata (key/value pairs sent as request headers).
//!
//! Mirrors the HTTP `RequestHeaders` panel: each row owns key/value `InputState`s
//! whose `Change` events push the current rows back into `GrpcState`.

use super::grpc_state::GrpcState;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::Button;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::StyledExt;
use models::Header;

struct MetadataRow {
    key: Entity<InputState>,
    value: Entity<InputState>,
    /// `Change` subscriptions on key/value that push edits back to `GrpcState`.
    _subscriptions: [Subscription; 2],
}

pub struct GrpcMetadata {
    grpc_state: Entity<GrpcState>,
    rows: Vec<MetadataRow>,
}

impl GrpcMetadata {
    pub fn new(
        grpc_state: Entity<GrpcState>,
        metadata: Vec<Header>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let rows = metadata
            .into_iter()
            .map(|h| {
                let (key, value) = Self::make_inputs(&h.key, &h.value, window, cx);
                Self::make_row(key, value, cx)
            })
            .collect();

        Self { grpc_state, rows }
    }

    fn make_inputs(
        key: &str,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Entity<InputState>, Entity<InputState>) {
        let key = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(key)
                .placeholder("Key")
        });
        let value = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(value)
                .placeholder("Value")
        });
        (key, value)
    }

    /// Builds a row, wiring `Change` subscriptions that sync edits to `GrpcState`.
    fn make_row(
        key: Entity<InputState>,
        value: Entity<InputState>,
        cx: &mut Context<Self>,
    ) -> MetadataRow {
        let key_sub = cx.subscribe(&key, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.sync(cx);
            }
        });
        let value_sub = cx.subscribe(&value, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.sync(cx);
            }
        });
        MetadataRow {
            key,
            value,
            _subscriptions: [key_sub, value_sub],
        }
    }

    /// Pushes the current rows to `GrpcState`.
    fn sync(&self, cx: &mut Context<Self>) {
        let metadata = self.metadata(cx);
        self.grpc_state
            .update(cx, |s, cx| s.set_metadata(metadata, cx));
    }

    pub fn add_row(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (key, value) = Self::make_inputs("", "", window, cx);
        let row = Self::make_row(key, value, cx);
        self.rows.push(row);
        self.sync(cx);
        cx.notify();
    }

    fn metadata(&self, cx: &App) -> Vec<Header> {
        self.rows
            .iter()
            .map(|row| {
                Header::new(
                    row.key.read(cx).value().to_string(),
                    row.value.read(cx).value().to_string(),
                )
            })
            .collect()
    }
}

impl Render for GrpcMetadata {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .items_center()
                    .child("Metadata")
                    .child(Button::new("add_metadata").label("+").on_click(cx.listener(
                        |this, _, window, cx| {
                            this.add_row(window, cx);
                        },
                    ))),
            )
            .children(self.rows.iter().enumerate().map(|(i, row)| {
                div()
                    .h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().flex_1().child(Input::new(&row.key)))
                    .child(div().flex_1().child(Input::new(&row.value)))
                    .child(
                        Button::new(("remove_metadata", i))
                            .label("×")
                            .w(px(24.0))
                            .h(px(24.0))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.rows.remove(i);
                                this.sync(cx);
                                cx.notify();
                            })),
                    )
            }))
            .when(self.rows.is_empty(), |el| {
                el.child(
                    div()
                        .p_4()
                        .text_center()
                        .text_color(rgb(0x6c757d))
                        .text_sm()
                        .child("No metadata"),
                )
            })
    }
}
