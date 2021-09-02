use std::time::Duration;

use crate::Config;
use chrono::offset::Local;
use lapin::{
    options::{BasicPublishOptions, ConfirmSelectOptions},
    BasicProperties, Channel, Connection, ConnectionProperties, Result,
};
use serde::{Deserialize, Serialize};
use serde_json::to_vec;
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

pub async fn rmq_connect(url: &str) -> Channel {
    let mut polling_interval = tokio::time::interval(Duration::from_secs(3));

    loop {
        polling_interval.tick().await;

        match rmq_get_channel(url).await {
            Ok(chan) => return chan,
            Err(err) => {
                error!(?err, "Failed to connect: ");
                continue;
            }
        }
    }
}

async fn rmq_get_channel(url: &str) -> Result<Channel> {
    let conn = Connection::connect(url, ConnectionProperties::default().with_tokio()).await?;
    let channel = conn.create_channel().await?;
    channel
        .confirm_select(ConfirmSelectOptions { nowait: false })
        .await?;

    Ok(channel)
}

pub async fn send_public_messages(
    chan: Channel,
    config: &Config,
    to_send: u32,
    counter: &mut u32,
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
        *counter += 1;
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
