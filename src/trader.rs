use std::sync::{mpsc::Receiver, Arc};

use chrono::{Datelike, Duration, FixedOffset, Timelike, Utc};
use serenity::{http::Http, model::id::ChannelId, prelude::RwLock};
use tokio::time;
use tracing::{debug, error, info};

use crate::util::*;
use crate::{
    market::{Market, ShareKind},
    naver::api,
};

pub(crate) async fn update_market(
    discord: Arc<Http>,
    channel_id: u64,
    rx_quit: Receiver<()>,
    market: Arc<RwLock<Market>>,
) {
    info!("Start");

    let channel = ChannelId(channel_id);
    let time_zone = FixedOffset::east(9 * 3600);

    // 초기화.
    {
        for &code in &["KOSPI", "KOSDAQ"] {
            let not_exists = {
                let market = market.read().await;
                !market.contains(code)
            };
            
            if not_exists {
                let index = api::get_index(code)
                    .await
                    .expect(&format!("Init {}", code));

                let mut market = market.write().await;
                market.add_or_update_index(code, &index);
            }
        }
    }

    let mut prev_on_work = false;

    loop {
        if let Ok(_) = rx_quit.try_recv() {
            break;
        }

        let now = Utc::now().naive_utc() + time_zone;
        let now_time = now.time();
        let on_work = now.weekday().number_from_monday() <= 5 // 평일
            && now_time.hour() >= 8
            && now_time.hour() < 17;

        if !prev_on_work && on_work {
            prev_on_work = true;
            info!("시장 추적 시작.");
        } else if prev_on_work && !on_work {
            prev_on_work = false;
            info!("시장 추적 종료.");
        }

        if !on_work {
            time::delay_for(std::time::Duration::from_millis(3000)).await;
            continue;
        }

        // 주식 코드 목록 얻기.
        let codes: Vec<_> = {
            market
                .read()
                .await
                .share_codes()
                .into_iter()
                .map(|s| s.clone())
                .collect()
        };

        for code in codes {
            let kind = {
                let market = market.read().await;
                let share = market.get_share(&code);
                share.map(|s| s.kind)
            };

            if let Some(kind) = kind {
                match kind {
                    ShareKind::Index => {
                        let index = api::get_index(&code).await;
                        match index {
                            Ok(index) => {
                                let mut market = market.write().await;
                                market.add_or_update_index(&code, &index);
                            }
                            Err(err) => error!("{}", err),
                        }

                        let mut date_time = (Utc::now().naive_utc() + time_zone)
                            .date()
                            .and_hms(23, 59, 59);
                        let mut time_jump_cnt = 0;
                        let mut page_num = 1;
                        let mut graph_len = 0;

                        while graph_len < 60 && time_jump_cnt <= 10 {
                            // 추가 요청시 딜레이.
                            if page_num > 1 || time_jump_cnt > 0 {
                                time::delay_for(std::time::Duration::from_millis(200)).await;
                            }

                            debug!("get_index_quotes({}, {}, {})", &code, &date_time, &page_num);
                            let page = api::get_index_quotes(&code, &date_time, page_num).await;
                            match page {
                                Ok(page) => {
                                    if page.is_last {
                                        page_num = 1;
                                        date_time -= Duration::days(1);
                                        time_jump_cnt += 1;
                                    } else {
                                        page_num += 1;
                                    }

                                    let mut market = market.write().await;
                                    market.update_index_graph(&code, &page, &date_time.date());

                                    graph_len =
                                        market.get_share(&code).map(|s| s.graph.len()).unwrap_or(0);
                                }
                                Err(err) => {
                                    error!("{}", err);
                                    graph_len += 6; // 무한 루프 방지를 위해 이렇게 하고 재시도.
                                }
                            }
                        }
                    }
                    ShareKind::Stock => todo!(), // TODO:
                }
            }
        }

        time::delay_for(std::time::Duration::from_millis(3000)).await;
    }

    info!("Exit");
}

pub(crate) async fn notify_market_state(
    discord: Arc<Http>,
    channel_id: u64,
    rx_quit: Receiver<()>,
    market: Arc<RwLock<Market>>,
) {
    info!("Start");

    let mut prev_state = None;

    loop {
        if let Ok(_) = rx_quit.try_recv() {
            break;
        }

        for &code in &["KOSPI", "KOSDAQ"] {
            let data: Option<_> = {
                let market = market.read().await;
                market.get_share(code).map(|share| {
                    (
                        share.state,
                        share.value,
                        share.change_value,
                        share.change_rate,
                    )
                })
            };

            if let Some((state, value, change_value, change_rate)) = data {
                if let Some(prev_state) = &prev_state {
                    if *prev_state != state {
                        // 장 알림 전송.
                        let msg_result = ChannelId(channel_id)
                            .send_message(&discord, |m| {
                                m.embed(|e| {
                                    e.title(format!("{} {}", code, state));
                                    e.description(format!(
                                        "{}　{}{}　{:+.2}%",
                                        format_value(value, 2),
                                        get_change_value_char(change_value),
                                        format_value(change_value, 2),
                                        change_rate
                                    ));
                                    e.color(get_change_value_color(change_value));
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
                prev_state = Some(state);
            }
        }

        time::delay_for(std::time::Duration::from_millis(3000)).await;
    }

    info!("Exit");
}
