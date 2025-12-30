use gpui::*;
use gpui_component::{blue_200, blue_800, gray_500, green_400, h_flex, red_400, IconName};
use models::{ContentType, HttpMethod, ServiceType};

pub fn cal_icon(ct: &ContentType) -> impl IntoElement {
    match ct {
        ContentType::ServiceFolder(_) | ContentType::SchemaFolder(_) => h_flex()
            .p_1()
            .h_4()
            .items_center()
            .justify_center()
            .child(IconName::Folder)
            .into_any_element(),
        ContentType::Service(service) => match service.service_type.clone() {
            ServiceType::Http(http) => {
                let (text, bg) = match http.method {
                    HttpMethod::Get => ("GET", green()),
                    HttpMethod::Post => ("POST", blue()),
                    HttpMethod::Put => ("PUT", blue_800()),
                    HttpMethod::Delete => ("DELETE", red()),
                    HttpMethod::Patch => ("PATCH", blue_200()),
                    HttpMethod::Head => ("HEAD", gray_500()),
                    HttpMethod::Options => ("OPTIONS", black()),
                };
                h_flex()
                    .p_1()
                    .h_4()
                    .items_center()
                    .justify_center()
                    .child(text)
                    .text_color(white())
                    .text_xs()
                    .bg(bg)
                    .into_any_element()
            }
            ServiceType::Grpc(_) => h_flex()
                .p_1()
                .h_4()
                .items_center()
                .justify_center()
                .child("GRPC")
                .text_color(white())
                .text_sm()
                .bg(green_400())
                .into_any_element(),
            ServiceType::WebSocket(_) => h_flex()
                .p_1()
                .h_4()
                .items_center()
                .justify_center()
                .child("WS")
                .text_color(white())
                .text_sm()
                .bg(red_400())
                .into_any_element(),
        },
        ContentType::Schema(_) => h_flex()
            .p_1()
            .h_4()
            .items_center()
            .justify_center()
            .child(IconName::Moon)
            .into_any_element(),
    }
}
