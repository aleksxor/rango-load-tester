use std::time::Duration;

use lapin::{
    options::BasicPublishOptions, BasicProperties, Channel, Connection, ConnectionProperties,
    Result,
};
use tokio::time::sleep;
use tokio_amqp::LapinTokioExt;
use tracing::{debug, error};

pub async fn rmq_listen(url: &str) {
    let mut retry_interval = tokio::time::interval(Duration::from_secs(5));

    loop {
        retry_interval.tick().await;
        debug!("Connecting rqm...");
        match init_rmq_listen(url).await {
            Ok(chan) => {
                debug!("rmq connected");
                let _ = send_public_messages(chan).await;

                break;
            }
            Err(err) => error!(?err, "rmq connection failed"),
        }
    }
}

async fn init_rmq_listen(url: &str) -> Result<Channel> {
    let conn = Connection::connect(url, ConnectionProperties::default().with_tokio()).await?;
    let channel = conn.create_channel().await?;

    Ok(channel)
}

pub async fn send_public_messages(chan: Channel) -> Result<()> {
    let payload = b"{\"data\":\"test\"}";

    for _ in 0..1000 {
        chan.basic_publish(
            "peatio.events.ranger",
            "public.test",
            BasicPublishOptions::default(),
            payload.to_vec(),
            BasicProperties::default(),
        )
        .await?
        .await?;

        let _ = sleep(Duration::from_millis(30));
    }

    Ok(())
}
