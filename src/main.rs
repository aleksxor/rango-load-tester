use futures_util::future::join3;
use std::{
    cell::RefCell,
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

#[derive(Debug, PartialEq)]
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
    state: RefCell<State>,
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
            state: RefCell::new(State::Initial),
        }
    }

    pub fn rmq_connected(&self) {
        if *self.state.borrow() == State::Initial {
            *self.state.borrow_mut() = State::RmqConnected;
        }
    }

    pub fn ws_connected(&self) {
        if *self.state.borrow() == State::RmqConnected {
            *self.state.borrow_mut() = State::WsConnected;
        }
    }

    async fn wait_for_state(&self, state: State, millis: u64) {
        let mut polling_interval = tokio::time::interval(Duration::from_millis(millis));

        loop {
            polling_interval.tick().await;

            if *self.state.borrow() == state {
                break;
            }
        }
    }

    pub async fn wait_for_rmq(&self) {
        self.wait_for_state(State::RmqConnected, 300).await;
    }

    pub async fn wait_for_ws(&self) {
        self.wait_for_state(State::WsConnected, 300).await;
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

    let total_msgs = config.msg_count as u64 * config.ws_pool_size as u64;

    let rmq_connect = rmq::run(&config);
    let ws_connect = async {
        config.wait_for_rmq().await;
        ws::run(&config).await;

        tokio::time::sleep(Duration::from_secs(3)).await;
        config.ws_connected();
    };

    let rmq_iterate = async {
        config.wait_for_rmq().await;

        let mut polling_interval = tokio::time::interval(Duration::from_secs(5));

        loop {
            polling_interval.tick().await;

            let socket_count = *config.stats.socket_count.lock().unwrap();

            info!("Open sockets: {}", socket_count);
            info!(
                "Received {} messages out of {}",
                *config.stats.msg_count.lock().unwrap(),
                total_msgs
            );

            if *config.state.borrow() == State::WsConnected && socket_count <= 0 {
                break;
            }
        }
    };

    join3(rmq_connect, ws_connect, rmq_iterate).await;

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
