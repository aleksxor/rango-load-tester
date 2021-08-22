use std::{cell::RefCell, time::Duration};

use chrono::offset::Local;
use lapin::{
    options::BasicPublishOptions, BasicProperties, Channel, Connection, ConnectionProperties,
    Result,
};
use serde::{Deserialize, Serialize};
use serde_json::to_vec;
use tokio_amqp::LapinTokioExt;
use tracing::{debug, error, info};

use crate::Config;

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

pub async fn run(config: &Config) {
    let msg_sent = RefCell::new(0u32);
    let mut retry_interval = tokio::time::interval(Duration::from_secs(5));

    loop {
        retry_interval.tick().await;
        info!("Connecting rqm to {}...", config.rmq_addr);
        match init_rmq_listen(&config.rmq_addr).await {
            Ok(chan) => {
                info!("rmq connected");
                let msg_left = config.msg_count - *msg_sent.borrow();
                info!("Sending {} messages", msg_left);
                let _ = send_public_messages(chan, &config, msg_left, &msg_sent).await;

                if *msg_sent.borrow() >= config.msg_count {
                    info!("Finished sending messages");
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
    config: &Config,
    to_send: u32,
    counter: &RefCell<u32>,
) -> Result<()> {
    let mut timeout_interval = match config.msg_delay {
        0 => None,
        _ => Some(tokio::time::interval(Duration::from_millis(
            config.msg_delay as u64,
        ))),
    };

    for _ in 0..to_send {
        if let Some(ref mut interval) = timeout_interval {
            interval.tick().await;
        }

        send_message(&chan, &config.stream, Cmd::Test).await?;
        *counter.borrow_mut() += 1;
    }

    send_message(&chan, &config.stream, Cmd::Close).await?;
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
