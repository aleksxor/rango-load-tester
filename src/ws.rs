use std::sync::{Arc, Mutex};

use chrono::offset::Local;
use futures_util::{future::join_all, FutureExt, StreamExt};
use serde_json::{from_slice, from_value, Value};
use tokio::{net::TcpStream, spawn};
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error};

use crate::{
    rmq::{Cmd, RmqMessage},
    MsgStats,
};

pub async fn run<'a>(size: u32, url: url::Url, stream: Arc<String>, stats: Arc<Mutex<MsgStats>>) {
    let mut pool = vec![];

    for i in 0..size {
        debug!("Creating client nr.: {}", i + 1);
        let fut: _ = connect_async(&url).then(|connect| async {
            let stats = Arc::clone(&stats);
            let stream = Arc::clone(&stream);

            match connect {
                Ok((ws_stream, _)) => {
                    stats.lock().unwrap().socket_count += 1;
                    let _ = spawn(handle_message(ws_stream, stream, stats));
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
    stats: Arc<Mutex<MsgStats>>,
) {
    while let Some(msg) = ws.next().await {
        if let Ok(data) = msg {
            if data.is_ping() {
                continue;
            }

            let data = data.into_data();
            let contents = from_slice(&data)
                .and_then(|data: Value| from_value::<RmqMessage>(data[stream.as_ref()].to_owned()));

            if let Ok(message) = contents {
                debug!(?message, "Received ws message");

                match message.cmd {
                    Cmd::Test => {
                        let diff = Local::now().timestamp_millis() - message.time;
                        let msg_count = stats.lock().unwrap().msg_count as f64;
                        let mean = stats.lock().unwrap().mean_res_time;

                        stats.lock().unwrap().msg_count += 1;
                        stats.lock().unwrap().mean_res_time =
                            (mean * msg_count + (diff as f64)) / (msg_count + 1.);
                    }
                    Cmd::Close => {
                        debug!("Closing ws connection");
                        break;
                    }
                }
            }
        }
    }

    stats.lock().unwrap().socket_count -= 1;
    debug!("Connection closed")
}
