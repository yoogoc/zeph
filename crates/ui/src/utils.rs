use crate::primitives::Badge;
use gpui::*;
use gpui_component::{green_400, h_flex, red_400, IconName};
use models::{ContentType, ServiceType};

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
            ServiceType::Http(http) => Badge::new(http.method).compact(true).into_any_element(),
            ServiceType::Grpc(_) => h_flex()
                .p_1()
                .h_4()
                .rounded_lg()
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
                .rounded_lg()
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
