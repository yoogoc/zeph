use futures::{Stream, StreamExt};
use std::pin::Pin;
use tonic::{Request, Response, Status};
pub mod shop {
    tonic::include_proto!("shop.v1");
}
use shop::order_service_server::{OrderService, OrderServiceServer};
use shop::*;
#[derive(Default)]
pub struct Orders;
type RpcStream<T> = Pin<Box<dyn Stream<Item = Result<T, Status>> + Send>>;
#[tonic::async_trait]
impl OrderService for Orders {
    async fn get_order(
        &self,
        request: Request<GetOrderRequest>,
    ) -> Result<Response<Order>, Status> {
        let body = request.into_inner();
        Ok(Response::new(Order {
            order_id: body.order_id,
            status: "paid".into(),
            total_amount: 129900,
            currency: "CNY".into(),
        }))
    }
    type WatchOrdersStream = RpcStream<OrderEvent>;
    async fn watch_orders(
        &self,
        _request: Request<WatchOrdersRequest>,
    ) -> Result<Response<Self::WatchOrdersStream>, Status> {
        Ok(Response::new(Box::pin(futures::stream::iter([
            Ok(OrderEvent {
                order_id: "ord_1024".into(),
                status: "paid".into(),
                version: 1,
            }),
            Ok(OrderEvent {
                order_id: "ord_1024".into(),
                status: "shipped".into(),
                version: 2,
            }),
        ]))))
    }
    async fn create_orders(
        &self,
        request: Request<tonic::Streaming<CreateOrderRequest>>,
    ) -> Result<Response<CreateOrdersSummary>, Status> {
        let mut stream = request.into_inner();
        let mut count = 0;
        while let Some(message) = stream.message().await? {
            if message.quantity < 1 {
                return Err(Status::invalid_argument("quantity must be positive"));
            }
            count += 1;
        }
        Ok(Response::new(CreateOrdersSummary {
            created_count: count,
            order_ids: (0..count).map(|i| format!("ord_{}", 2000 + i)).collect(),
        }))
    }
    type SyncOrdersStream = RpcStream<SyncOrderResponse>;
    async fn sync_orders(
        &self,
        request: Request<tonic::Streaming<SyncOrderRequest>>,
    ) -> Result<Response<Self::SyncOrdersStream>, Status> {
        let stream = request.into_inner().map(|message| {
            message.map(|m| SyncOrderResponse {
                order_id: m.order_id,
                accepted: true,
                version: m.version,
            })
        });
        Ok(Response::new(Box::pin(stream)))
    }
}
pub async fn serve(
    listener: tokio::net::TcpListener,
    shutdown: tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    let descriptor = include_bytes!(concat!(env!("OUT_DIR"), "/order.bin"));
    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(descriptor)
        .build_v1()?;
    let legacy = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(descriptor)
        .build_v1alpha()?;
    tonic::transport::Server::builder()
        .add_service(reflection)
        .add_service(legacy)
        .add_service(OrderServiceServer::new(Orders))
        .serve_with_incoming_shutdown(
            tokio_stream::wrappers::TcpListenerStream::new(listener),
            shutdown.cancelled_owned(),
        )
        .await?;
    Ok(())
}
