use futures_util::future::join;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tracing::info;
use uuid::Uuid;

mod rmq;
mod ws;

const WS_POOL_SIZE: u32 = 20_000;
const MSG_COUNT: u32 = 1_000;
const MSG_DELAY: u32 = 30;

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

    std::env::set_var(
        "RUST_LOG",
        std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_owned()),
    );

    let stream = format!("public.{}", Uuid::new_v4());
    let ws_addr = std::env::var("WS_ADDR").unwrap_or_else(|_| "ws://localhost:8080/".into());
    let ws_addr = format!("{}?stream={}", ws_addr, stream);

    let rmq_addr =
        std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://localhost:5672/%2f".into());

    let ws_pool_size = parse_env_u32("WS_POOL_SIZE", WS_POOL_SIZE);
    let msg_count = parse_env_u32("MSG_COUNT", MSG_COUNT);
    let msg_delay = parse_env_u32("MSG_DELAY", MSG_DELAY);

    tracing_subscriber::fmt::init();

    info!(
        "ws_pool_size: {}, msg_count: {}, msg_delay: {}",
        ws_pool_size, msg_count, msg_delay
    );

    let ws_url = url::Url::parse(&ws_addr).unwrap();

    ws::run(ws_pool_size, &ws_url, &stream, &stats).await;

    let mut polling_interval = tokio::time::interval(Duration::from_secs(5));
    let rmq_connect = rmq::run(
        &rmq_addr,
        rmq::RmqConfig {
            stream: &stream,
            msg_count,
            msg_delay,
        },
    );
    let rmq_iterate = async {
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

    join(rmq_connect, rmq_iterate).await;

    info!("Received {} messages", *stats.msg_count.lock().unwrap());
    info!(
        "Mean delivery time is {}ms",
        *stats.mean_res_time.lock().unwrap()
    );
}

fn parse_env_u32(env: &str, default: u32) -> u32 {
    std::env::var(env)
        .map(|var| var.parse::<u32>().unwrap_or(default))
        .unwrap_or(default)
}
