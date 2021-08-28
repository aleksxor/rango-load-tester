use futures_util::future::join;
use std::time::Duration;
use tracing::{error, info};

use crate::config::Config;

mod config;
mod rmq;
mod ws;

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

    let (tx, rx) = tokio::sync::oneshot::channel();
    let main_worker = async {
        let mut chan = rmq::rmq_connect(&config.rmq_addr).await;
        info!("rmq connected");

        tokio::time::sleep(Duration::from_secs(3)).await;
        ws::run(&config).await;
        info!("websockets connected");
        tx.send(()).unwrap();

        let mut msg_sent = 0;

        loop {
            let to_send = config.msg_count - msg_sent;
            info!("start sending {} messages", to_send);
            match rmq::send_public_messages(chan, &config, to_send, &mut msg_sent).await {
                Err(err) => {
                    error!(?err, "Failed to send a message: ");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    chan = rmq::rmq_connect(&config.rmq_addr).await;
                }
                Ok(()) => {
                    info!("Finished sending messages.");
                    break;
                }
            }
        }
    };

    let watch = async {
        rx.await.unwrap();

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

            if socket_count <= 0 {
                break;
            }
        }
    };

    join(main_worker, watch).await;

    info!(
        "Received {} messages",
        *config.stats.msg_count.lock().unwrap()
    );
    info!(
        "Mean delivery time is {}ms",
        *config.stats.mean_res_time.lock().unwrap()
    );
}
