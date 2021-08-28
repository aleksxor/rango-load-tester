use std::sync::{Arc, Mutex};

use chrono::offset::Local;
use futures_util::{future::join_all, FutureExt, StreamExt};
use serde::Deserialize;
use serde_json::{from_slice, from_value, Error, Value};
use tokio::{net::TcpStream, spawn};
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, trace};

use crate::{
    rmq::{Cmd, RmqMessage},
    Config,
};

#[derive(Deserialize, Debug)]
struct SubscribeSuccess {
    message: String,
    streams: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct SubscribeMessage {
    success: SubscribeSuccess,
}

enum WsMsg {
    RmqMsg(RmqMessage),
    SubscribeMsg(SubscribeMessage),
}

pub async fn run(config: &Config) {
    let mut pool = vec![];

    for i in 0..config.ws_pool_size {
        debug!("Creating client nr.: {}", i + 1);
        let fut: _ = connect_async(&config.ws_addr).then(|connect| async {
            let msg_count = Arc::clone(&config.stats.msg_count);
            let mean_res_time = Arc::clone(&config.stats.mean_res_time);
            let socket_count = Arc::clone(&config.stats.socket_count);

            match connect {
                Ok((ws_stream, _)) => {
                    spawn(handle_message(
                        ws_stream,
                        config.stream.clone(),
                        msg_count,
                        mean_res_time,
                        socket_count,
                    ));
                }
                Err(err) => {
                    error!(?err, "Failed to connect");
                }
            }
        });

        pool.push(fut);
    }

    join_all(pool).await;
}

async fn handle_message(
    mut ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    stream: String,
    msg_count: Arc<Mutex<u64>>,
    mean_res_time: Arc<Mutex<f64>>,
    socket_count: Arc<Mutex<i32>>,
) {
    *socket_count.lock().unwrap() += 1;

    while let Some(msg) = ws.next().await {
        if let Ok(data) = msg {
            if data.is_ping() {
                continue;
            }

            if data.is_close() {
                break;
            }

            match parse_message(data.into_data(), &stream) {
                Ok(WsMsg::RmqMsg(message)) => {
                    debug!(?message, "Received ws message");

                    match message.cmd {
                        Cmd::Test => {
                            let mut mean = mean_res_time.lock().unwrap();
                            let mut count = msg_count.lock().unwrap();

                            *mean = calc_new_mean(message, *count, *mean);
                            *count += 1;
                        }
                        Cmd::Close => {
                            debug!("Closing ws connection");
                            break;
                        }
                    };
                }
                Ok(WsMsg::SubscribeMsg(message)) => {
                    if message.success.message.eq("subscribed") {
                        debug!(?message, "Subscribed");
                    }
                }
                _ => (),
            };
        }
    }

    *socket_count.lock().unwrap() -= 1;
    debug!("Connection closed")
}

fn calc_new_mean(message: RmqMessage, msg_count: u64, old_mean: f64) -> f64 {
    let now = Local::now().timestamp_millis();
    let diff = now - message.time;
    let new_mean = (old_mean * msg_count as f64 + (diff as f64)) / (msg_count as f64 + 1.);

    trace!(
        "now: {}, msg_ts: {}, diff is {}, old mean is: {}, new mean = {}",
        now,
        message.time,
        diff,
        old_mean,
        new_mean
    );

    new_mean
}

fn parse_message(data: Vec<u8>, stream: &str) -> Result<WsMsg, Error> {
    from_slice(&data).and_then(|parsed: Value| {
        let rmq_msg = from_value::<RmqMessage>(parsed[stream].to_owned()).map(WsMsg::RmqMsg);
        let sub_msg = from_value::<SubscribeMessage>(parsed).map(WsMsg::SubscribeMsg);

        rmq_msg.or(sub_msg)
    })
}
