mod rabbit;
mod websockets;

const WS_POOL_SIZE: u32 = 50000;

#[tokio::main]
async fn main() {
    let connect_addr =
        std::env::var("WS_ADDR").unwrap_or_else(|_| "ws://localhost:8080/public".into());
    let _rabbit_addr =
        std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://localhost:5672/%2f".into());
    let ws_pool_size = std::env::var("WS_POOL_SIZE")
        .map(|var| var.parse::<u32>().unwrap_or(WS_POOL_SIZE))
        .unwrap_or(WS_POOL_SIZE);

    tracing_subscriber::fmt::init();

    let url = url::Url::parse(&connect_addr).unwrap();
    let msg_count = websockets::ws_create_connections(ws_pool_size, url).await;

    println!("Received {} messages", msg_count);
}
