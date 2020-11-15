use std::{
    collections::HashMap,
    sync::{mpsc::Receiver, Arc},
};

use chrono::{Datelike, Duration, FixedOffset, Timelike, Utc};
use serenity::{http::Http, model::id::ChannelId, prelude::RwLock, utils::Colour};
use tokio::time;
use tracing::{debug, error, info};

use crate::{
    market::{Market, ShareKind},
    naver::api,
};
use crate::{naver::model::MarketState, util::*};

pub(crate) async fn update_market(rx_quit: Receiver<()>, market: Arc<RwLock<Market>>) {
    info!("Start");

    let time_zone = FixedOffset::east(9 * 3600);

    // 초기화.
    {
        for &code in &["KOSPI", "KOSDAQ"] {
            let not_exists = {
                let market = market.read().await;
                !market.contains(code)
            };

            if not_exists {
                let index = api::get_index(code).await.expect(&format!("Init {}", code));

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
                .share_codes_with_kind()
                .into_iter()
                .map(|(code, kind)| (code.clone(), kind))
                .collect()
        };

        for (code, kind) in codes {
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
                }
                ShareKind::Stock => {
                    let stock = api::get_stock(&code).await;
                    match stock {
                        Ok(stock) => {
                            let mut market = market.write().await;
                            market.add_or_update_stock(&code, &stock);
                        }
                        Err(err) => error!("{}", err),
                    }
                }
            }

            let mut date_time = (Utc::now().naive_utc() + time_zone)
                .date()
                .and_hms(23, 59, 59);
            let mut time_jump_cnt = 0;
            let mut page_num = 1;
            let mut graph_len = 0;

            while graph_len < 120 && time_jump_cnt <= 10 {
                // 추가 요청시 딜레이.
                if page_num > 1 || time_jump_cnt > 0 {
                    time::delay_for(std::time::Duration::from_millis(200)).await;
                }

                debug!("Get quotes: {}, {}, {}", code, date_time, page_num);

                // 그래프 갱신 및 마지막 페이지 여부 확인.
                let is_last = {
                    let mut market = market.write().await;
                    match kind {
                        ShareKind::Index => {
                            let page = api::get_index_quotes(&code, &date_time, page_num).await;
                            page.and_then(|page| {
                                market.update_index_graph(&code, &page, &date_time.date());
                                Ok(page.is_last)
                            })
                        }
                        ShareKind::Stock => {
                            let page = api::get_stock_quotes(&code, &date_time, page_num).await;
                            page.and_then(|page| {
                                market.update_stock_graph(&code, &page, &date_time.date());
                                Ok(page.is_last)
                            })
                        }
                    }
                };

                match is_last {
                    Ok(is_last) => {
                        let market = market.read().await;
                        graph_len = market.get_share(&code).map(|s| s.graph.len()).unwrap_or(0);

                        // 다음 페이지를 선택하되 마지막 페이지라면 더 전날로 이동.
                        if is_last {
                            page_num = 1;
                            date_time -= Duration::days(1);
                            time_jump_cnt += 1;
                        } else {
                            page_num += 1;
                        }
                    }
                    Err(err) => {
                        error!("{}", err);
                        graph_len += 10; // 무한 루프 방지를 위해 이렇게 하고 재시도.
                        time::delay_for(std::time::Duration::from_millis(5000)).await;
                    }
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

    let mut prev_states = HashMap::new();

    loop {
        if let Ok(_) = rx_quit.try_recv() {
            break;
        }

        let mut alarms = Vec::new();
        let mut rep_state = MarketState::Close;

        let codes: Vec<_> = {
            let market = market.read().await;
            market
                .share_codes()
                .into_iter()
                .map(|s| s.clone())
                .collect()
        };

        for code in codes {
            let data: Option<_> = {
                let market = market.read().await;
                market.get_share(&code).map(|share| {
                    (
                        share.name.clone(),
                        share.kind,
                        share.state,
                        share.value,
                        share.change_value,
                        share.change_rate,
                    )
                })
            };

            if let Some((name, kind, state, value, change_value, change_rate)) = data {
                if let Some(prev_state) = prev_states.get(&code) {
                    if *prev_state != state {
                        let radix = if kind == ShareKind::Index { 2 } else { 0 };
                        let msg = format!(
                            "{}　{}　{}{}　{:+.2}%",
                            name,
                            format_value(value, radix),
                            get_change_value_char(change_value),
                            format_value(change_value.abs(), radix),
                            change_rate
                        );
                        alarms.push(msg);
                        rep_state = state;
                    }
                }
                prev_states.insert(code, state);
            }
        }

        // 장 알림 전송.
        if !alarms.is_empty() {
            let msg_result = ChannelId(channel_id)
                .send_message(&discord, |m| {
                    m.embed(|e| {
                        e.title(rep_state);
                        e.description(alarms.join("\n"));
                        e.color(match rep_state {
                            MarketState::PreOpen => Colour::from_rgb(25, 118, 210),
                            MarketState::Close => Colour::from_rgb(97, 97, 97),
                            MarketState::Open => Colour::from_rgb(67, 160, 71),
                        });
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

        time::delay_for(std::time::Duration::from_millis(3000)).await;
    }

    info!("Exit");
}
