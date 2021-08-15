use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::time::sleep;

mod rmq;
mod ws;

const WS_POOL_SIZE: u32 = 50000;
const MSG_COUNT: u32 = 1_000;
const STREAMS: &[&str] = &["public.test"];

#[tokio::main]
async fn main() {
    let msg_count = Arc::new(Mutex::new(0u32));

    let streams = STREAMS.join(",");
    let ws_addr = std::env::var("WS_ADDR")
        .unwrap_or_else(|_| format!("ws://localhost:8080/?stream={}", streams));
    let rmq_addr =
        std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://localhost:5672/%2f".into());
    let ws_pool_size = std::env::var("WS_POOL_SIZE")
        .map(|var| var.parse::<u32>().unwrap_or(WS_POOL_SIZE))
        .unwrap_or(WS_POOL_SIZE);

    tracing_subscriber::fmt::init();

    let ws_url = url::Url::parse(&ws_addr).unwrap();
    ws::run(ws_pool_size, ws_url, Arc::clone(&msg_count)).await;

    rmq::run(&rmq_addr, MSG_COUNT).await;

    sleep(Duration::from_secs(60)).await;

    println!("Received {} messages", *msg_count.lock().unwrap());
}
