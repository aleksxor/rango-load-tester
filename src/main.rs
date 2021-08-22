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

#[derive(Debug)]
pub struct MsgStats {
    pub msg_count: Arc<Mutex<u64>>,
    pub socket_count: Arc<Mutex<i32>>,
    pub mean_res_time: Arc<Mutex<f64>>,
}

impl MsgStats {
    pub fn new() -> Self {
        MsgStats {
            msg_count: Arc::new(Mutex::new(0)),
            socket_count: Arc::new(Mutex::new(0)),
            mean_res_time: Arc::new(Mutex::new(0.)),
        }
    }
}

#[derive(Debug)]
pub enum State {
    Initial,
    RmqConnected,
    WsConnected,
    Finished,
}

#[derive(Debug)]
pub struct Config {
    pub stream: String,
    pub rmq_addr: String,
    pub ws_addr: String,
    pub ws_pool_size: u32,
    pub msg_delay: u32,
    pub msg_count: u32,
    pub stats: MsgStats,
    pub state: State,
}

impl Config {
    pub fn new() -> Self {
        let stream = format!("public.{}", Uuid::new_v4());
        let ws_addr = std::env::var("WS_ADDR").unwrap_or_else(|_| "ws://localhost:8080/".into());
        let ws_addr = format!("{}?stream={}", ws_addr, stream);

        let rmq_addr =
            std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://localhost:5672/%2f".into());

        let ws_pool_size = parse_env_u32("WS_POOL_SIZE", WS_POOL_SIZE);
        let msg_count = parse_env_u32("MSG_COUNT", MSG_COUNT);
        let msg_delay = parse_env_u32("MSG_DELAY", MSG_DELAY);

        Config {
            stream,
            rmq_addr,
            ws_addr,
            ws_pool_size,
            msg_delay,
            msg_count,
            stats: MsgStats::new(),
            state: State::Initial,
        }
    }
}

#[tokio::main]
async fn main() {
    std::env::set_var(
        "RUST_LOG",
        std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_owned()),
    );

    tracing_subscriber::fmt::init();

    let config = Config::new();
    info!("Config: {:?}", config);

    let mut polling_interval = tokio::time::interval(Duration::from_secs(5));

    ws::run(&config).await;

    let rmq_connect = rmq::run(&config);
    let rmq_iterate = async {
        loop {
            polling_interval.tick().await;

            let socket_count = *config.stats.socket_count.lock().unwrap();

            info!("Open sockets: {}", socket_count);
            info!(
                "Received {} messages out of {}",
                *config.stats.msg_count.lock().unwrap(),
                config.msg_count * config.ws_pool_size
            );
            if socket_count <= 0 {
                break;
            }
        }
    };

    join(rmq_connect, rmq_iterate).await;

    info!(
        "Received {} messages",
        *config.stats.msg_count.lock().unwrap()
    );
    info!(
        "Mean delivery time is {}ms",
        *config.stats.mean_res_time.lock().unwrap()
    );
}

fn parse_env_u32(env: &str, default: u32) -> u32 {
    std::env::var(env)
        .map(|var| var.parse::<u32>().unwrap_or(default))
        .unwrap_or(default)
}
