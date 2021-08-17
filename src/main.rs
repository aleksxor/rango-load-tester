use futures_util::future::join;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tracing::info;
use uuid::Uuid;

mod rmq;
mod ws;

const WS_POOL_SIZE: u32 = 50000;
const MSG_COUNT: u32 = 1_000;

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

    let stream = format!("public.{}", Uuid::new_v4());
    let ws_addr = std::env::var("WS_ADDR").unwrap_or_else(|_| "ws://localhost:8080/".into());
    let ws_addr = format!("{}?stream={}", ws_addr, stream);
    let rmq_addr =
        std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://localhost:5672/%2f".into());
    let ws_pool_size = std::env::var("WS_POOL_SIZE")
        .map(|var| var.parse::<u32>().unwrap_or(WS_POOL_SIZE))
        .unwrap_or(WS_POOL_SIZE);

    tracing_subscriber::fmt::init();

    let ws_url = url::Url::parse(&ws_addr).unwrap();

    ws::run(ws_pool_size, &ws_url, &stream, &stats).await;

    let rmq = rmq::run(&rmq_addr, &stream, MSG_COUNT);

    let mut polling_interval = tokio::time::interval(Duration::from_secs(5));

    let iterate = async {
        loop {
            polling_interval.tick().await;

            let socket_count = *stats.socket_count.lock().unwrap();

            info!("Socket count: {}", socket_count);
            info!(
                "Received {} messages out of {}",
                *stats.msg_count.lock().unwrap(),
                MSG_COUNT * ws_pool_size
            );
            if socket_count <= 0 {
                break;
            }
        }
    };

    join(iterate, rmq).await;

    info!("Received {} messages", *stats.msg_count.lock().unwrap());
    info!(
        "Mean delivery time is {}ms",
        *stats.mean_res_time.lock().unwrap()
    );
}
