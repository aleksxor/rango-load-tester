use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use uuid::Uuid;

mod rmq;
mod ws;

const WS_POOL_SIZE: u32 = 50000;
const MSG_COUNT: u32 = 1_000;

pub struct MsgStats {
    pub msg_count: u32,
    pub socket_count: i32,
    pub mean_res_time: f64,
}

#[tokio::main]
async fn main() {
    let stats = Arc::new(Mutex::new(MsgStats {
        msg_count: 0,
        socket_count: 0,
        mean_res_time: 0.,
    }));

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
        Arc::clone(&stats),
    )
    .await;

    rmq::run(&rmq_addr, &stream, MSG_COUNT).await;

    let mut polling_interval = tokio::time::interval(Duration::from_millis(500));

    loop {
        polling_interval.tick().await;

        if stats.lock().unwrap().socket_count <= 0 {
            break;
        }
    }

    println!("Received {} messages", stats.lock().unwrap().msg_count);
    println!(
        "Mean response time is {}ms",
        stats.lock().unwrap().mean_res_time
    );
}
