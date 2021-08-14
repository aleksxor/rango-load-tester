use std::sync::{Arc, Mutex};

use futures_util::{future::join_all, stream::SplitStream, FutureExt, StreamExt};
use tokio::spawn;
use tokio_tungstenite::connect_async;
use tracing::{debug, error};

pub async fn run(size: u32, url: url::Url) -> u32 {
    let msg_count = Arc::new(Mutex::new(0u32));
    let mut pool = vec![];

    for i in 0..size {
        debug!("Creating client nr.: {}", i + 1);
        let fut: _ = connect_async(&url).then(|connect| async {
            let counter = Arc::clone(&msg_count);

            match connect {
                Ok((ws_stream, _)) => {
                    spawn(async move {
                        let (_, mut recv) = ws_stream.split();
                        while let Some(msg) = recv.next().await {
                            if let Ok(data) = msg {
                                if data.is_ping() {
                                    continue;
                                }

                                *counter.lock().unwrap() += 1;
                                debug!(?data, "Received ws message");
                            }
                        }
                        debug!("Closed connection");
                    })
                    .await;
                }
                Err(err) => {
                    error!(?err, "Failed to connect");
                }
            }
        });

        pool.push(fut);
    }

    join_all(pool).await;
    let count = *msg_count.lock().unwrap();
    count
}

async fn handle_message<S>(rx: SplitStream<S>, counter: Arc<Mutex<u32>>) {}
