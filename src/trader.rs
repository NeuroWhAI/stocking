use std::{
    collections::HashMap,
    sync::{mpsc::Receiver, Arc},
};

use chrono::{Datelike, Duration, FixedOffset, Timelike, Utc};
use serenity::{http::Http, model::id::ChannelId, prelude::RwLock, utils::Colour};
use tokio::time;
use tracing::{debug, error, info};

use crate::{
    alarm::StockAlarm,
    market::{Market, ShareKind},
    naver::api,
    naver::model::MarketState,
    naver::model::Stock,
    util::*,
};

pub(crate) const UPDATE_TERM: std::time::Duration = std::time::Duration::from_millis(3000);

pub(crate) async fn update_market(
    discord: Arc<Http>,
    channel_id: u64,
    rx_quit: Receiver<()>,
    market: Arc<RwLock<Market>>,
    stock_alarm: Arc<RwLock<StockAlarm>>,
) {
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
                let index = api::get_index(code)
                    .await
                    .unwrap_or_else(|_| panic!("Init {}", code));

                let mut market = market.write().await;
                market.add_or_update_index(code, &index);
            }
        }
    }

    let mut prev_on_work = false;

    loop {
        if rx_quit.try_recv().is_ok() {
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
            time::sleep(UPDATE_TERM).await;
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
                            // 다른 쪽에서 삭제되었을 수 있으니 lock 걸고 존재하는지 확인한 뒤 갱신.
                            if market.contains(&code) {
                                market.add_or_update_index(&code, &index);
                            }
                        }
                        Err(err) => error!("{}", err),
                    }
                }
                ShareKind::Stock => {
                    let stock = api::get_stock(&code).await;
                    match stock {
                        Ok(stock) => {
                            let prev_value = {
                                let market = market.read().await;
                                market.get_share(&code).map(|share| share.value)
                            };

                            // 알람 확인.
                            let mut executed_alarms = Vec::new();
                            if let Some(prev_value) = prev_value {
                                let stock_alarm = stock_alarm.read().await;
                                if let Some(alarms) = stock_alarm.get_alarms(&code) {
                                    for &target_value in alarms {
                                        // 상승, 하락 돌파 조건.
                                        if (prev_value <= target_value
                                            && target_value <= stock.now_value)
                                            || (prev_value >= target_value
                                                && target_value >= stock.now_value)
                                        {
                                            executed_alarms.push(target_value);
                                        }
                                    }
                                }
                            }

                            // 알람 전송.
                            if !executed_alarms.is_empty() {
                                // 알람은 일회성이라 삭제하고 보냄.
                                {
                                    let mut stock_alarm = stock_alarm.write().await;
                                    for &target_value in &executed_alarms {
                                        stock_alarm.remove_alarm(&code, target_value);
                                    }
                                }
                                let move_val =
                                    prev_value.map(|prev| stock.now_value - prev).unwrap_or(0);
                                send_alarm(
                                    &discord,
                                    channel_id,
                                    &stock,
                                    &executed_alarms,
                                    move_val,
                                )
                                .await;
                            }

                            let mut market = market.write().await;
                            // 다른 쪽에서 삭제되었을 수 있으니 lock 걸고 존재하는지 확인한 뒤 갱신.
                            if market.contains(&code) {
                                market.add_or_update_stock(&code, &stock);
                            }
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
                    time::sleep(std::time::Duration::from_millis(200)).await;
                }

                debug!("Get quotes: {}, {}, {}", code, date_time, page_num);

                // 그래프 갱신 및 마지막 페이지 여부 확인.
                let is_last = {
                    let mut market = market.write().await;
                    match kind {
                        ShareKind::Index => {
                            let page = api::get_index_quotes(&code, &date_time, page_num).await;
                            page.map(|page| {
                                market.update_index_graph(&code, &page, &date_time.date());
                                page.is_last
                            })
                        }
                        ShareKind::Stock => {
                            let page = api::get_stock_quotes(&code, &date_time, page_num).await;
                            page.map(|page| {
                                market.update_stock_graph(&code, &page, &date_time.date());
                                page.is_last
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
                        time::sleep(std::time::Duration::from_millis(5000)).await;
                    }
                }
            }
        }

        time::sleep(UPDATE_TERM).await;
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
        if rx_quit.try_recv().is_ok() {
            break;
        }

        let mut alarms = Vec::new();
        let mut rep_state = MarketState::Close;

        let codes: Vec<_> = {
            let market = market.read().await;
            market.share_codes().into_iter().cloned().collect()
        };

        // 관심 종목이 아닌 것의 상태 기억은 제거.
        prev_states.retain(|k, _| codes.contains(k));

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
                let prev_state = prev_states.entry(code.clone()).or_insert(state);
                if prev_state != &state {
                    *prev_state = state;

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

            if let Err(err) = msg_result {
                error!("{}", err);
            }
        }

        time::sleep(UPDATE_TERM).await;
    }

    info!("Exit");
}

pub(crate) async fn notify_change_rate(
    discord: Arc<Http>,
    channel_id: u64,
    rx_quit: Receiver<()>,
    market: Arc<RwLock<Market>>,
) {
    info!("Start");

    // 상한 범위.
    let limit_range = 1.0;

    let mut prev_states = HashMap::new();
    let mut rate_limits = HashMap::new();

    loop {
        if rx_quit.try_recv().is_ok() {
            break;
        }

        let codes: Vec<_> = {
            let market = market.read().await;
            market.share_codes().into_iter().cloned().collect()
        };

        // 관심 종목이 아닌 것의 정보는 제거.
        prev_states.retain(|k, _| codes.contains(k));
        rate_limits.retain(|k, _| codes.contains(k));

        for code in codes {
            let data: Option<_> = {
                let market = market.read().await;
                market
                    .get_share(&code)
                    .and_then(|share| {
                        // 지수는 알리지 않음.
                        if share.kind != ShareKind::Stock {
                            None
                        } else {
                            Some(share)
                        }
                    })
                    .map(|share| {
                        (
                            share.name.clone(),
                            share.state,
                            share.value,
                            share.change_value,
                            share.change_rate,
                        )
                    })
            };

            if let Some((name, state, value, change_value, change_rate)) = data {
                // 장 상태가 장중으로 바뀌는 시점에 상한 초기화.
                let prev_state = prev_states.entry(code.clone()).or_insert(state);
                if prev_state != &state {
                    *prev_state = state;

                    if state == MarketState::Open {
                        rate_limits.insert(code.clone(), limit_range);
                    }
                }

                // 장중 상태에서만 알림.
                if state != MarketState::Open {
                    continue;
                }

                let mut update_limit = false;

                // 현재 등락률이 설정된 범위를 벗어났는지 확인.
                if let Some(&upper) = rate_limits.get(&code) {
                    let lower = upper - limit_range * 2.0;
                    if change_rate > upper - f64::EPSILON || change_rate < lower + f64::EPSILON {
                        // 현재 등락률 기준으로 상한 다시 계산.
                        update_limit = true;

                        // 범위 중간에서 얼마나 움직였나 계산.
                        let move_val = change_rate - (upper - limit_range);

                        // 등락 알림 전송.
                        let msg_result = ChannelId(channel_id)
                            .send_message(&discord, |m| {
                                m.embed(|e| {
                                    let move_desc =
                                        if move_val > 0.0 { "상승" } else { "하락" };
                                    e.title(format!("{} - {}", move_desc, name));
                                    e.description(format!(
                                        "{}　{}　{}{}　{:+.2}%",
                                        name,
                                        format_value(value, 0),
                                        get_change_value_char(change_value),
                                        format_value(change_value.abs(), 0),
                                        change_rate
                                    ));
                                    e.color(get_light_change_color(move_val));
                                    e
                                });
                                m
                            })
                            .await;

                        if let Err(err) = msg_result {
                            error!("{}", err);
                        }
                    }
                } else {
                    // 장중에 추가된 종목이면 여기 올 수 있음.
                    // 상한을 현재 등락률로 계산해서 초기화하도록 함.
                    update_limit = true;
                }

                if update_limit {
                    // 현재 등락률 기준으로 상한 계산.
                    let new_upper = (change_rate / limit_range).round() * limit_range + limit_range;
                    rate_limits.insert(code, new_upper);
                }
            }
        }

        time::sleep(UPDATE_TERM).await;
    }

    info!("Exit");
}

pub(crate) async fn notify_high_trading_vol(
    discord: Arc<Http>,
    channel_id: u64,
    rx_quit: Receiver<()>,
    market: Arc<RwLock<Market>>,
) {
    info!("Start");

    let mut prev_noti = HashMap::new();

    loop {
        if rx_quit.try_recv().is_ok() {
            break;
        }

        let codes: Vec<_> = {
            let market = market.read().await;
            market
                .share_codes_with_kind()
                .into_iter()
                .filter_map(|(code, kind)| {
                    if kind == ShareKind::Stock {
                        Some(code.clone())
                    } else {
                        None
                    }
                })
                .collect()
        };

        // 관심 종목이 아닌 것의 정보는 제거.
        prev_noti.retain(|k, _| codes.contains(k));

        for code in codes {
            let data: Option<_> = {
                let market = market.read().await;
                market
                    .get_share(&code)
                    .filter(|share| share.state == MarketState::Open) // 장중일 때만.
                    .map(|share| {
                        (
                            share.name.clone(),
                            share.value,
                            share.change_value,
                            share.change_rate,
                            share.graph.latest_time(),
                            share.graph.avg_trading_vol_move(0, 1), // 현재 변동량.
                            share.graph.avg_trading_vol_move(1, 20), // 평균 변동량.
                        )
                    })
            };

            if let Some((
                name,
                value,
                change_value,
                change_rate,
                Some(time),
                Some(curr_move),
                Some(avg_move),
            )) = data
            {
                // 현재 거래 변동량이 최소한은 있고 과거 평균의 일정 배를 초과하는 것이 급등 조건.
                if curr_move > 3000.0 && curr_move > avg_move * 5.0 {
                    let scale = curr_move / avg_move;

                    // 최초 알림이거나 아래 조건 만족시에만 알림.
                    let cond = prev_noti.get(&code).map(|&(prev_t, prev_scale)| {
                        // 이전 알림과 중복 시간이 아니고
                        // 급등 기록을 갱신했거나 이전 알림 후 일정 시간이 지났다면.
                        prev_t != time
                            && (scale > prev_scale || time - prev_t > Duration::minutes(10))
                    });
                    let new_noti = matches!(cond, None | Some(true));

                    if new_noti {
                        // 최근 알림 기록.
                        prev_noti.insert(code, (time, scale));

                        // 급등 알림 전송.
                        let msg_result = ChannelId(channel_id)
                            .send_message(&discord, |m| {
                                m.embed(|e| {
                                    e.title(format!("거래량 급등 - {}", name));
                                    e.description(format!(
                                        "{}　{}{}　{:+.2}%\n변동량 {}(평균 {}의 {:.1}%)",
                                        format_value(value, 0),
                                        get_change_value_char(change_value),
                                        format_value(change_value.abs(), 0),
                                        change_rate,
                                        format_value(curr_move as i64, 0),
                                        format_value(avg_move.round() as i64, 0),
                                        scale * 100.0,
                                    ));
                                    e.color(get_change_value_color(change_value));
                                    e
                                });
                                m
                            })
                            .await;

                        if let Err(err) = msg_result {
                            error!("{}", err);
                        }
                    }
                }
            }
        }

        time::sleep(UPDATE_TERM).await;
    }

    info!("Exit");
}

async fn send_alarm(
    discord: &Arc<Http>,
    channel_id: u64,
    stock: &Stock,
    target_values: &[i64],
    move_val: i64,
) {
    let msg_result = ChannelId(channel_id)
        .send_message(discord, |m| {
            m.embed(|e| {
                e.title(format!("알람 - {}", stock.name));
                let alarm_desc = target_values
                    .iter()
                    .map(|&val| format_value(val, 0) + "원")
                    .collect::<Vec<_>>()
                    .join(", ");
                e.description(format!(
                    "{}　{}{}　{:.2}%\n돌파: {}",
                    format_value(stock.now_value, 0),
                    get_change_value_char(stock.change_value()),
                    format_value(stock.change_value().abs(), 0),
                    stock.change_rate(),
                    alarm_desc,
                ));
                e.color(get_light_change_color(move_val));
                e
            });
            m
        })
        .await;

    if let Err(err) = msg_result {
        error!("{}", err);
    }
}
