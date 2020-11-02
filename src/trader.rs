use std::{
    sync::{mpsc::Receiver, Arc},
    time::Duration,
};

use serenity::{http::Http, model::id::ChannelId};
use tokio::time;
use tracing::{error, info};

use crate::naver::api;
use crate::util::*;

pub async fn notify_market_state(discord: Arc<Http>, channel_id: u64, rx_quit: Receiver<()>) {
    info!("Start");

    let mut prev_state = None;

    loop {
        if let Ok(_) = rx_quit.try_recv() {
            break;
        }

        match api::get_index("KOSPI").await {
            Ok(index) => {
                if let Some(prev_state) = &prev_state {
                    if *prev_state != index.state {
                        let msg_result = ChannelId(channel_id)
                            .send_message(&discord, |m| {
                                m.embed(|e| {
                                    e.title(format!("KOSPI {}", index.state));
                                    e.description(format!(
                                        "{}　{}{}　{:.2}%",
                                        format_value(index.now_value, 2),
                                        get_change_value_char(index.change_value),
                                        format_value(index.change_value, 2),
                                        index.change_rate
                                    ));
                                    e.color(get_change_value_color(index.change_value));
                                    e
                                });
                                m
                            })
                            .await;

                        match msg_result {
                            Err(err) => error!("{}", err),
                            _ => {}
                        }
                    }
                }
                prev_state = Some(index.state);
            }
            Err(err) => error!("{}", err),
        }

        time::delay_for(Duration::from_millis(3000)).await;
    }

    info!("Exit");
}
