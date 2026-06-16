//! GrpcService: `ContentView` shell + request editor for a gRPC service.
//!
//! Owns the editing inputs (endpoint, proto, request JSON, metadata) and the
//! response panel, driving the `GrpcState` entity which holds all logic.

use super::grpc_client::MethodInfo;
use super::grpc_metadata::GrpcMetadata;
use super::grpc_response_panel::GrpcResponsePanel;
use super::grpc_state::{DefinitionSource, GrpcState};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState, TabSize};
use gpui_component::resizable::{resizable_panel, v_resizable};
use gpui_component::select::{Select, SelectEvent, SelectItem, SelectState};
use gpui_component::spinner::Spinner;
use gpui_component::tab::{Tab, TabBar, TabVariant};
use gpui_component::{h_flex, v_flex, Disableable, IndexPath, Sizable};
use uuid::Uuid;

/// Which editing tab is visible in the upper area. Order matches the tab bar
/// (Service Definition is last); the discriminant is used as the tab index.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum EditTab {
    #[default]
    Request,
    Metadata,
    Definition,
}

#[derive(Clone)]
struct MethodChoice {
    label: SharedString,
    index: usize,
}

impl SelectItem for MethodChoice {
    type Value = usize;
    fn title(&self) -> SharedString {
        self.label.clone()
    }
    fn value(&self) -> &Self::Value {
        &self.index
    }
}

pub struct GrpcService {
    service: models::Service,
    grpc_state: Entity<GrpcState>,
    response_panel: Entity<GrpcResponsePanel>,

    endpoint: Entity<InputState>,
    proto: Entity<InputState>,
    request_body: Entity<InputState>,
    metadata_panel: Entity<GrpcMetadata>,
    method_select: Entity<SelectState<Vec<MethodChoice>>>,
    /// Signature of the method list the `method_select` was built from; when the
    /// state's method list changes (e.g. after reflection), the select rebuilds.
    method_sig: Vec<String>,

    active_tab: EditTab,

    _subscriptions: Vec<Subscription>,
}

impl GrpcService {
    pub fn new(
        service: models::Service,
        service_state: Entity<crate::state::ServiceState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let grpc_state = cx.new(|cx| GrpcState::new(service.clone(), service_state, window, cx));
        let response_panel = cx.new(|cx| GrpcResponsePanel::new(window, cx));
        grpc_state.update(cx, |s, _| s.set_response_panel(response_panel.downgrade()));

        let (
            endpoint_val,
            proto_val,
            body_val,
            metadata_val,
            method_choices,
            method_ix,
            method_sig,
        ) = {
            let st = grpc_state.read(cx);
            (
                st.endpoint().to_string(),
                st.proto_content().to_string(),
                st.request_body().to_string(),
                st.metadata().to_vec(),
                Self::method_choices(st),
                st.selected_method_index(),
                Self::sig_of(st.methods().iter()),
            )
        };

        let endpoint = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("http://localhost:50051")
                .default_value(&endpoint_val)
        });
        let proto = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("proto")
                .line_number(true)
                .default_value(&proto_val)
        });
        let request_body = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("json")
                .tab_size(TabSize {
                    tab_size: 2,
                    hard_tabs: false,
                })
                .line_number(true)
                .default_value(&body_val)
        });
        let metadata_panel =
            cx.new(|cx| GrpcMetadata::new(grpc_state.clone(), metadata_val, window, cx));
        let method_select = cx
            .new(|cx| SelectState::new(method_choices, method_ix.map(IndexPath::new), window, cx));

        let mut subs = Vec::new();
        subs.push(cx.subscribe(&endpoint, |this, ent, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                let v = ent.read(cx).value().to_string();
                this.grpc_state.update(cx, |s, cx| s.set_endpoint(v, cx));
            }
        }));
        subs.push(
            cx.subscribe(&request_body, |this, ent, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    let v = ent.read(cx).value().to_string();
                    this.grpc_state
                        .update(cx, |s, cx| s.set_request_body(v, cx));
                }
            }),
        );
        // Proto edits recompile and refresh the method list.
        subs.push(
            cx.subscribe_in(&proto, window, |this, ent, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::Change) {
                    let v = ent.read(cx).value().to_string();
                    this.grpc_state
                        .update(cx, |s, cx| s.set_proto_content(v, cx));
                    this.refresh_method_select(window, cx);
                }
            }),
        );
        subs.push(cx.subscribe(
            &method_select,
            |this, _, ev: &SelectEvent<Vec<MethodChoice>>, cx| {
                if let SelectEvent::Confirm(Some(ix)) = ev {
                    let ix = *ix;
                    this.grpc_state.update(cx, |s, cx| s.select_method(ix, cx));
                }
            },
        ));

        Self {
            service,
            grpc_state,
            response_panel,
            endpoint,
            proto,
            request_body,
            metadata_panel,
            method_select,
            method_sig,
            active_tab: EditTab::default(),
            _subscriptions: subs,
        }
    }

    /// A stable signature of the method list, to detect changes for select rebuild.
    fn sig_of<'a>(methods: impl Iterator<Item = &'a MethodInfo>) -> Vec<String> {
        methods
            .map(|m| format!("{}/{}", m.service, m.method))
            .collect()
    }

    /// Opens a file picker for a `.proto` file and loads it into the editor.
    fn import_proto_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        let grpc_state = self.grpc_state.clone();
        let proto_input = self.proto.clone();
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await {
                if let Some(path) = paths.into_iter().next() {
                    let content = std::fs::read_to_string(&path).unwrap_or_default();
                    cx.update(|window, cx| {
                        proto_input.update(cx, |s, cx| s.set_value(&content, window, cx));
                        grpc_state.update(cx, |s, cx| s.import_proto_file(path, cx));
                    })
                    .ok();
                    this.update_in(cx, |this, window, cx| {
                        this.refresh_method_select(window, cx)
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    /// Opens a directory picker and adds it as a proto import root.
    fn add_include_dir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        let grpc_state = self.grpc_state.clone();
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await {
                if let Some(dir) = paths.into_iter().next() {
                    cx.update(|_, cx| {
                        grpc_state.update(cx, |s, cx| s.add_include_dir(dir, cx));
                    })
                    .ok();
                    this.update_in(cx, |this, window, cx| {
                        this.refresh_method_select(window, cx)
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn method_choices(st: &GrpcState) -> Vec<MethodChoice> {
        st.methods()
            .iter()
            .enumerate()
            .map(|(index, m)| {
                let suffix = match (m.client_streaming, m.server_streaming) {
                    (false, false) => "",
                    (true, false) => " (client stream)",
                    (false, true) => " (server stream)",
                    (true, true) => " (bidi stream)",
                };
                MethodChoice {
                    label: format!("{}/{}{}", m.service, m.method, suffix).into(),
                    index,
                }
            })
            .collect()
    }

    /// Rebuilds the method `Select` after a proto recompile.
    fn refresh_method_select(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (choices, ix) = {
            let st = self.grpc_state.read(cx);
            (Self::method_choices(st), st.selected_method_index())
        };
        self.method_select =
            cx.new(|cx| SelectState::new(choices, ix.map(IndexPath::new), window, cx));
        let sub = cx.subscribe(
            &self.method_select,
            |this, _, ev: &SelectEvent<Vec<MethodChoice>>, cx| {
                if let SelectEvent::Confirm(Some(ix)) = ev {
                    let ix = *ix;
                    this.grpc_state.update(cx, |s, cx| s.select_method(ix, cx));
                }
            },
        );
        self._subscriptions.push(sub);
        cx.notify();
    }

    fn render_definition(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let st = self.grpc_state.read(cx);
        let source = st.source();
        let proto_error = st.proto_error().map(|e| e.to_string());
        let is_reflecting = st.is_reflecting();
        let include_dirs: Vec<String> = st
            .include_dirs()
            .iter()
            .map(|p| p.display().to_string())
            .collect();

        let mode_button = |label: &'static str, mode: DefinitionSource, active: bool| {
            Button::new(SharedString::from(format!("def_mode_{label}")))
                .label(label)
                .xsmall()
                .when(active, |b| b.primary())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.grpc_state.update(cx, |s, cx| s.set_source(mode, cx));
                }))
        };

        v_flex()
            .size_full()
            .gap_2()
            // Mode switch: proto editor vs server reflection.
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(mode_button(
                        "From Proto",
                        DefinitionSource::Proto,
                        source == DefinitionSource::Proto,
                    ))
                    .child(mode_button(
                        "From Reflection",
                        DefinitionSource::Reflection,
                        source == DefinitionSource::Reflection,
                    )),
            )
            .map(|el| match source {
                DefinitionSource::Proto => el
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("import_proto")
                                    .label("Import .proto")
                                    .xsmall()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.import_proto_file(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("add_include")
                                    .label("Add import path")
                                    .xsmall()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.add_include_dir(window, cx);
                                    })),
                            ),
                    )
                    .when(!include_dirs.is_empty(), |el| {
                        el.child(
                            v_flex()
                                .gap_1()
                                .children(include_dirs.iter().enumerate().map(|(i, d)| {
                                    h_flex()
                                        .gap_1()
                                        .items_center()
                                        .text_xs()
                                        .child(div().flex_1().child(d.clone()))
                                        .child(
                                            Button::new(("rm_inc", i))
                                                .label("×")
                                                .xsmall()
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.grpc_state.update(cx, |s, cx| {
                                                        s.remove_include_dir(i, cx)
                                                    });
                                                })),
                                        )
                                })),
                        )
                    })
                    .child(div().flex_1().child(Input::new(&self.proto).h_full())),
                DefinitionSource::Reflection => el.child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            Button::new("do_reflect")
                                .label(if is_reflecting {
                                    "Fetching..."
                                } else {
                                    "Fetch from server"
                                })
                                .primary()
                                .xsmall()
                                .disabled(is_reflecting)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    // Async fetch; the method select rebuilds in
                                    // render once the new method list arrives.
                                    this.grpc_state
                                        .update(cx, |s, cx| s.refresh_from_reflection(cx));
                                })),
                        )
                        .child(
                            div()
                                .text_xs()
                                .child("Discovers services/methods from the endpoint above."),
                        ),
                ),
            })
            .when_some(proto_error, |el, err| {
                el.child(div().text_sm().text_color(gpui::rgb(0xdc3545)).child(err))
            })
    }
}

impl Render for GrpcService {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Rebuild the method select if the discovered method list changed (e.g.
        // after reflection completed asynchronously or the proto recompiled).
        let current_sig = Self::sig_of(self.grpc_state.read(cx).methods().iter());
        if current_sig != self.method_sig {
            let became_available = self.method_sig.is_empty() && !current_sig.is_empty();
            self.method_sig = current_sig;
            self.refresh_method_select(window, cx);
            // Once a service is defined (methods just appeared), return to the
            // Request tab so the user can pick a method and send.
            if became_available && self.active_tab == EditTab::Definition {
                self.active_tab = EditTab::Request;
            }
        }

        let st = self.grpc_state.read(cx);
        let is_loading = st.is_loading;
        let is_modified = st.is_modified(cx);
        let has_methods = !st.methods().is_empty();
        let active_tab = self.active_tab;
        let response_panel = self.response_panel.clone();

        let editor = match active_tab {
            EditTab::Definition => self.render_definition(cx).into_any_element(),
            EditTab::Request => v_flex()
                .size_full()
                .child(
                    div()
                        .flex_1()
                        .child(Input::new(&self.request_body).h_full()),
                )
                .into_any_element(),
            EditTab::Metadata => self.metadata_panel.clone().into_any_element(),
        };

        v_flex()
            .size_full()
            .py_2()
            .px_4()
            .gap_2()
            .relative()
            .child(
                h_flex()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&self.endpoint)))
                    .when(has_methods, |el| {
                        // The trigger adapts to the selected label (clamped so it
                        // neither collapses nor overruns the bar); the dropdown
                        // popup gets its own wider width so full method names show
                        // even when the trigger is narrow.
                        el.child(
                            div()
                                .min_w(px(160.))
                                .max_w(px(480.))
                                .child(Select::new(&self.method_select).menu_width(px(420.))),
                        )
                    })
                    .when(!has_methods, |el| {
                        // No methods yet: prompt the user to define the service.
                        el.child(
                            Button::new("goto_definition")
                                .label("Select a service…")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.active_tab = EditTab::Definition;
                                    cx.notify();
                                })),
                        )
                    })
                    .child(
                        Button::new("grpc_send")
                            .primary()
                            .label(if is_loading { "Sending..." } else { "Invoke" })
                            .disabled(is_loading)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.grpc_state
                                    .update(cx, |s, cx| s.send_request(window, cx));
                            })),
                    )
                    .child(
                        Button::new("grpc_save")
                            .label("Save")
                            .disabled(!is_modified)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.grpc_state.update(cx, |s, cx| s.save(cx));
                            })),
                    ),
            )
            .child(
                v_resizable("grpc-service")
                    .child(
                        resizable_panel().child(
                            v_flex()
                                .size_full()
                                .gap_2()
                                .child(
                                    TabBar::new("grpc_tabs")
                                        .with_variant(TabVariant::Underline)
                                        .child(Tab::new().label("Request"))
                                        .child(Tab::new().label("Metadata"))
                                        .child(Tab::new().label("Service Definition"))
                                        .selected_index(active_tab as usize)
                                        .on_click(cx.listener(|this, ix: &usize, _, cx| {
                                            this.active_tab = match *ix {
                                                1 => EditTab::Metadata,
                                                2 => EditTab::Definition,
                                                _ => EditTab::Request,
                                            };
                                            cx.notify();
                                        })),
                                )
                                .child(div().flex_1().child(editor)),
                        ),
                    )
                    .child(
                        resizable_panel().child(
                            v_flex()
                                .size_full()
                                .gap_1()
                                .child("Response")
                                .child(div().flex_1().child(response_panel)),
                        ),
                    ),
            )
            .when(is_loading, |this| {
                this.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(hsla(0., 0., 1., 0.5))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| cx.stop_propagation())
                        .child(
                            v_flex()
                                .items_center()
                                .gap_2()
                                .child(Spinner::new().large())
                                .child(Button::new("grpc_cancel").label("Cancel").on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.grpc_state
                                            .update(cx, |s, cx| s.cancel_request(window, cx));
                                    }),
                                )),
                        ),
                )
            })
    }
}

impl crate::state::ContentView for GrpcService {
    fn id(&self) -> Uuid {
        self.service.id
    }

    fn title(&self) -> SharedString {
        self.service.name.clone()
    }

    fn tab_icon(&self) -> AnyElement {
        gpui_component::IconName::Globe.into_any_element()
    }

    fn set_title(&mut self, title: SharedString, cx: &mut Context<Self>) {
        self.service.name = title;
        cx.notify();
    }

    fn is_modified(&self, cx: &App) -> bool {
        self.grpc_state.read(cx).is_modified(cx)
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        self.grpc_state.update(cx, |s, cx| s.save(cx));
    }
}
