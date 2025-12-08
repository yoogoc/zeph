use gpui::*;
use gpui_component::{
    Root, TitleBar,
    button::{Button, ButtonVariants},
    h_flex,
    tab::{Tab, TabBar},
    v_flex,
};

pub struct Example {
    tab_index: usize,
}

impl Render for Example {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .size_full()
            .child(div().h_full().w_10().bg(gpui::red()))
            .child(
                v_flex()
                    .size_full()
                    .child(
                        // Render custom title bar on top of Root view.
                        TitleBar::new().child(
                            h_flex()
                                .w_full()
                                .pr_2()
                                .justify_between()
                                .child(
                                    h_flex().child("App with Custom title bar").child(
                                        TabBar::new("tabs0")
                                            .selected_index(self.tab_index)
                                            .on_click(cx.listener(|view, index, _, cx| {
                                                println!("Tab {} selected", index);
                                                view.tab_index = *index;
                                                cx.notify();
                                            }))
                                            .child(Tab::new().label("Account"))
                                            .child(Tab::new().label("Profile"))
                                            .child(Tab::new().label("Settings")),
                                    ),
                                )
                                .child(h_flex().child("Right Item")),
                        ),
                    )
                    .child(
                        div()
                            .id("window-body")
                            .p_5()
                            .size_full()
                            .items_center()
                            .justify_center()
                            .child("Hello, World!")
                            .child(
                                Button::new("ok")
                                    .primary()
                                    .label("Let's Go!")
                                    .on_click(|_, _, _| println!("Clicked!")),
                            )
                            .child(
                                TabBar::new("tabs")
                                    .selected_index(self.tab_index)
                                    .on_click(cx.listener(|view, index, _, cx| {
                                        println!("Tab {} selected", index);
                                        view.tab_index = *index;
                                        cx.notify();
                                    }))
                                    .child(Tab::new().label("Account"))
                                    .child(Tab::new().label("Profile"))
                                    .child(Tab::new().label("Settings")),
                            ),
                    ),
            )
    }
}

fn main() {
    let app = Application::new().with_assets(gpui_component_assets::Assets);

    app.run(move |cx| {
        gpui_component::init(cx);

        cx.spawn(async move |cx| {
            let window_options = WindowOptions {
                // Setup GPUI to use custom title bar
                titlebar: Some(TitleBar::title_bar_options()),
                ..Default::default()
            };

            cx.open_window(window_options, |window, cx| {
                let view = cx.new(|_| Example { tab_index: 0 });
                cx.new(|cx| Root::new(view, window, cx))
            })?;

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
