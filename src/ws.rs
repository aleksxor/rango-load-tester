use std::sync::{Arc, Mutex};

use chrono::offset::Local;
use futures_util::{future::join_all, FutureExt, StreamExt};
use serde_json::{from_slice, from_value, Value};
use tokio::{net::TcpStream, spawn};
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, trace};

use crate::{
    rmq::{Cmd, RmqMessage},
    MsgStats,
};

pub async fn run(size: u32, url: url::Url, stream: Arc<String>, stats: &MsgStats) {
    let mut pool = vec![];

    for i in 0..size {
        debug!("Creating client nr.: {}", i + 1);
        let fut: _ = connect_async(&url).then(|connect| async {
            let msg_count = Arc::clone(&stats.msg_count);
            let mean_res_time = Arc::clone(&stats.mean_res_time);
            let stream = Arc::clone(&stream);

            match connect {
                Ok((ws_stream, _)) => {
                    *stats.socket_count.lock().unwrap() += 1;
                    let _ = spawn(handle_message(
                        ws_stream,
                        stream,
                        msg_count,
                        mean_res_time,
                        Arc::clone(&stats.socket_count),
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
    stream: Arc<String>,
    msg_count: Arc<Mutex<u64>>,
    mean_res_time: Arc<Mutex<f64>>,
    socket_count: Arc<Mutex<i32>>,
) {
    while let Some(msg) = ws.next().await {
        if let Ok(data) = msg {
            if data.is_ping() {
                continue;
            }

            let now = Local::now().timestamp_millis();
            let data = data.into_data();
            let contents = from_slice(&data)
                .and_then(|data: Value| from_value::<RmqMessage>(data[stream.as_ref()].to_owned()));

            if let Ok(message) = contents {
                debug!(?message, "Received ws message");

                match message.cmd {
                    Cmd::Test => {
                        let diff = now - message.time;
                        let msgs = *msg_count.lock().unwrap() as f64;
                        let mean = *mean_res_time.lock().unwrap();
                        let new_mean = (mean * msgs + (diff as f64)) / (msgs + 1.);

                        trace!(
                            "now: {}, msg_ts: {}, diff is {}, mean is: {}, new mean = {}",
                            now,
                            message.time,
                            diff,
                            mean,
                            new_mean
                        );

                        *msg_count.lock().unwrap() += 1;
                        *mean_res_time.lock().unwrap() = new_mean;
                    }
                    Cmd::Close => {
                        debug!("Closing ws connection");
                        break;
                    }
                }
            }
        }
    }

    *socket_count.lock().unwrap() -= 1;
    debug!("Connection closed")
}
