use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tracing::debug;
use uuid::Uuid;

mod rmq;
mod ws;

const WS_POOL_SIZE: u32 = 50000;
const MSG_COUNT: u32 = 10_000;

pub struct MsgStats {
    pub msg_count: Arc<Mutex<u64>>,
    pub socket_count: Arc<Mutex<i32>>,
    pub mean_res_time: Arc<Mutex<f64>>,
}

#[tokio::main]
async fn main() {
    let stats = MsgStats {
        msg_count: Arc::new(Mutex::new(0)),
        socket_count: Arc::new(Mutex::new(0)),
        mean_res_time: Arc::new(Mutex::new(0.)),
    };

    let stream = Arc::new(format!("public.{}", Uuid::new_v4()));
    let ws_addr = std::env::var("WS_ADDR")
        .unwrap_or_else(|_| format!("ws://localhost:8080/?stream={}", stream));
    let rmq_addr =
        std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://localhost:5672/%2f".into());
    let ws_pool_size = std::env::var("WS_POOL_SIZE")
        .map(|var| var.parse::<u32>().unwrap_or(WS_POOL_SIZE))
        .unwrap_or(WS_POOL_SIZE);

    tracing_subscriber::fmt::init();

    let ws_url = url::Url::parse(&ws_addr).unwrap();
    ws::run(
        ws_pool_size,
        ws_url,
        Arc::clone(&stream),
        &stats,
    )
    .await;

    rmq::run(&rmq_addr, &stream, MSG_COUNT).await;

    let mut polling_interval = tokio::time::interval(Duration::from_millis(500));

    loop {
        polling_interval.tick().await;

        let socket_count = *stats.socket_count.lock().unwrap();

        debug!("Socket count: {}", socket_count);
        if socket_count <= 0 {
            break;
        }
    }

    println!("Received {} messages", *stats.msg_count.lock().unwrap());
    println!(
        "Mean delivery time is {}ms",
        *stats.mean_res_time.lock().unwrap()
    );
}
