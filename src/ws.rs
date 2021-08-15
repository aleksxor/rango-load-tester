use std::sync::{Arc, Mutex};

use futures_util::{future::join_all, FutureExt, StreamExt};
use serde_json::from_slice;
use tokio::{net::TcpStream, spawn};
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error};

use crate::rmq::{Cmd, RmqMessage};

pub async fn run(size: u32, url: url::Url, msg_count: Arc<Mutex<u32>>) {
    let mut pool = vec![];

    for i in 0..size {
        debug!("Creating client nr.: {}", i + 1);
        let fut: _ = connect_async(&url).then(|connect| async {
            let counter = Arc::clone(&msg_count);

            match connect {
                Ok((ws_stream, _)) => {
                    let _ = spawn(handle_message(ws_stream, counter));
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
    counter: Arc<Mutex<u32>>,
) {
    while let Some(msg) = ws.next().await {
        if let Ok(data) = msg {
            if data.is_ping() {
                continue;
            }

            let data = data.into_data();
            if let Ok(message) = from_slice::<RmqMessage>(&data) {
                *counter.lock().unwrap() += 1;
                debug!(?data, "Received ws message");

                if message.cmd == Cmd::Close {
                    debug!("Closing ws connection");
                    let _ = ws.close(None).await;
                }
            }
        }
    }

    debug!("Connection closed")
}
