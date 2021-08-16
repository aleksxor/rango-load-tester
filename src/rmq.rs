use std::{cell::RefCell, time::Duration};

use chrono::offset::Local;
use lapin::{
    options::BasicPublishOptions, BasicProperties, Channel, Connection, ConnectionProperties,
    Result,
};
use serde::{Deserialize, Serialize};
use serde_json::to_vec;
use tokio::time::sleep;
use tokio_amqp::LapinTokioExt;
use tracing::{debug, error};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum Cmd {
    Test,
    Close,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RmqMessage {
    pub cmd: Cmd,
    pub time: i64,
}

impl RmqMessage {
    pub fn new(cmd: Cmd, time: i64) -> Self {
        RmqMessage { cmd, time }
    }
}

pub async fn run(url: &str, stream: &str, msg_count: u32) {
    let msg_sent = RefCell::new(0u32);
    let mut retry_interval = tokio::time::interval(Duration::from_secs(5));

    loop {
        retry_interval.tick().await;
        debug!("Connecting rqm...");
        match init_rmq_listen(url).await {
            Ok(chan) => {
                debug!("rmq connected");
                let msg_left = msg_count - *msg_sent.borrow();
                debug!("Sending {} messages", msg_left);
                let _ = send_public_messages(chan, stream, msg_left, &msg_sent).await;

                if *msg_sent.borrow() >= msg_count {
                    debug!("Finished sending messages");
                    break;
                }
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

pub async fn send_public_messages(
    chan: Channel,
    stream: &str,
    to_send: u32,
    counter: &RefCell<u32>,
) -> Result<()> {
    for _ in 0..to_send {
        send_message(&chan, stream, Cmd::Test).await?;
        *counter.borrow_mut() += 1;

        let _ = sleep(Duration::from_millis(30)).await;
    }

    send_message(&chan, stream, Cmd::Close).await?;
    Ok(())
}

async fn send_message(chan: &Channel, stream: &str, cmd: Cmd) -> Result<()> {
    let payload = RmqMessage::new(cmd, Local::now().timestamp_millis());

    debug!(?payload, "Sending rmq message");

    chan.basic_publish(
        "peatio.events.ranger",
        stream,
        BasicPublishOptions::default(),
        to_vec(&payload).unwrap(),
        BasicProperties::default(),
    )
    .await?
    .await?;

    Ok(())
}
