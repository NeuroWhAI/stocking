use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serenity::prelude::*;
use serenity::{builder::CreateEmbed, model::prelude::*};
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    futures::future::join_all,
    utils::Colour,
};

use crate::{client_data::MarketContainer, naver::api};
use crate::{market::ShareKind, naver::model::MarketState, util::*};

#[command]
#[owners_only]
#[aliases("index")]
async fn show_index(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let name = args.rest().trim();
    let name = if name.is_empty() { "KOSPI" } else { name };

    match api::get_index(name).await {
        Ok(index) => {
            let response = msg
                .channel_id
                .send_message(&ctx.http, |m| {
                    m.embed(|e| {
                        e.title(name);
                        e.description(format!(
                            "{}　{}{}　{:+.2}%",
                            format_value(index.now_value, 2),
                            get_change_value_char(index.change_value),
                            format_value(index.change_value.abs(), 2),
                            index.change_rate
                        ));
                        e.thumbnail(format!(
                            "https://ssl.pstatic.net/imgfinance/chart/mobile/candle/day/{}_end.png",
                            name,
                        ));
                        e.image(format!(
                            "https://ssl.pstatic.net/imgstock/chart3/day/{}.png?sidcode={}",
                            name,
                            SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .map(|d| d.as_millis())
                                .unwrap_or_else(|_| 42)
                        ));
                        e.fields(vec![
                            ("거래량(천주)", format_value(index.trading_volume, 0), true),
                            ("거래대금(백만)", format_value(index.trading_value, 0), true),
                            ("장중최고", format_value(index.high_value, 2), true),
                            ("장중최저", format_value(index.low_value, 2), true),
                        ]);
                        e.footer(|f| {
                            f.text(index.state.to_string());
                            f
                        });
                        e.color(get_change_value_color(index.change_value));
                        e
                    });
                    m
                })
                .await?;

            // 선택용 이모지 달기.
            let emoji_add = '⭐';
            let emoji_del = '❌';
            let emoji_add = response.react(&ctx, emoji_add).await?;
            let emoji_del = response.react(&ctx, emoji_del).await?;

            // 응답 대기
            let answer = response
                .await_reaction(&ctx)
                .timeout(Duration::from_secs(30))
                .author_id(msg.author.id)
                .await;
            if let Some(answer) = answer {
                let data = ctx.data.read().await;
                if let Some(market) = data.get::<MarketContainer>() {
                    let mut market = market.write().await;
                    let emoji = &answer.as_inner_ref().emoji;
                    if *emoji == emoji_add.emoji {
                        // 내 마켓에 지수 추가.
                        market.add_or_update_index(name, &index);
                    } else if *emoji == emoji_del.emoji {
                        // 내 마켓에서 지수 삭제.
                        market.remove_share(name);
                    }
                }
            }

            // 선택 이모지 삭제.
            join_all(vec![emoji_add.delete_all(&ctx), emoji_del.delete_all(&ctx)]).await;

            Ok(())
        }
        Err(err) => {
            msg.reply(ctx, err.to_string()).await?;
            Err(err.into())
        }
    }
}

#[command]
#[owners_only]
#[aliases("stock")]
async fn show_stock(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let code = {
        let code = args.rest().trim();
        if code.parse::<usize>().is_err() {
            let results = api::search(code).await?;
            if results.len() >= 1 {
                results[0].code.clone()
            } else {
                msg.reply(ctx, "해당 검색어로 종목을 찾을 수 없습니다.")
                    .await?;
                return Ok(());
            }
        } else {
            code.to_owned()
        }
    };

    match api::get_stock(&code).await {
        Ok(stock) => {
            let response = msg.channel_id
                .send_message(&ctx.http, |m| {
                    m.embed(|e| {
                        e.title(&stock.name);
                        e.description(format!(
                            "{}　{}{}　{:.2}%",
                            format_value(stock.now_value, 0),
                            get_change_value_char(stock.change_value()),
                            format_value(stock.change_value().abs(), 0),
                            stock.change_rate()
                        ));
                        e.thumbnail(format!(
                            "https://ssl.pstatic.net/imgfinance/chart/mobile/candle/day/{}_end.png",
                            code,
                        ));
                        e.image(format!(
                            "https://ssl.pstatic.net/imgfinance/chart/mobile/day/{}_end.png?sidcode={}",
                            code,
                            SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .map(|d| d.as_millis())
                                .unwrap_or_else(|_| 42)
                        ));
                        e.fields(vec![
                            ("거래량", format_value(stock.trading_volume, 0), true),
                            ("거래대금(백만)", format_value(stock.trading_value / 1000000, 0), true),
                            ("장중최고", format_value(stock.high_value, 0), true),
                            ("장중최저", format_value(stock.low_value, 0), true),
                        ]);
                        e.footer(|f| {
                            f.text(stock.state.to_string());
                            f
                        });
                        e.color(get_change_value_color(stock.change_value()));
                        e
                    });
                    m
                })
                .await?;

            // 선택용 이모지 달기.
            let emoji_add = '⭐';
            let emoji_del = '❌';
            let emoji_add = response.react(&ctx, emoji_add).await?;
            let emoji_del = response.react(&ctx, emoji_del).await?;

            // 응답 대기
            let answer = response
                .await_reaction(&ctx)
                .timeout(Duration::from_secs(30))
                .author_id(msg.author.id)
                .await;
            if let Some(answer) = answer {
                let data = ctx.data.read().await;
                if let Some(market) = data.get::<MarketContainer>() {
                    let mut market = market.write().await;
                    let emoji = &answer.as_inner_ref().emoji;
                    if *emoji == emoji_add.emoji {
                        // 내 마켓에 종목 추가.
                        market.add_or_update_stock(&code, &stock);
                    } else if *emoji == emoji_del.emoji {
                        // 내 마켓에서 종목 삭제.
                        market.remove_share(&code);
                    }
                }
            }

            // 선택 이모지 삭제.
            join_all(vec![emoji_add.delete_all(&ctx), emoji_del.delete_all(&ctx)]).await;

            Ok(())
        }
        Err(err) => {
            msg.reply(ctx, err.to_string()).await?;
            Err(err.into())
        }
    }
}

#[command]
#[owners_only]
#[aliases("indices")]
async fn show_my_indices(ctx: &Context, msg: &Message) -> CommandResult {
    show_my_shares(ctx, msg, ShareKind::Index).await
}

#[command]
#[owners_only]
#[aliases("stocks")]
async fn show_my_stocks(ctx: &Context, msg: &Message) -> CommandResult {
    show_my_shares(ctx, msg, ShareKind::Stock).await
}

async fn show_my_shares(ctx: &Context, msg: &Message, target_kind: ShareKind) -> CommandResult {
    let radix = if target_kind == ShareKind::Index {
        2
    } else {
        0
    };

    let mut contents = Vec::new();
    let mut result_msg: Option<Message> = None;
    let mut emoji_stop: Option<Reaction> = None;

    let wait_timeout = Duration::from_secs(3);
    let max_edit = 60 * 3 / wait_timeout.as_secs();

    for time in 1..=max_edit {
        let mut rep_state = MarketState::Close;

        {
            let data = ctx.data.read().await;
            if let Some(market) = data.get::<MarketContainer>() {
                let market = market.read().await;

                for (code, kind) in market.share_codes_with_kind() {
                    if kind != target_kind {
                        continue;
                    }

                    if let Some(share) = market.get_share(code) {
                        let info = format!(
                            "{}　{}　{}{}　{:+.2}%",
                            share.name,
                            format_value(share.value, radix),
                            get_change_value_char(share.change_value),
                            format_value(share.change_value.abs(), radix),
                            share.change_rate
                        );
                        contents.push(info);
                        rep_state = share.state;
                    }
                }
            }
        }

        if contents.is_empty() {
            break;
        } else {
            fn embed_builder<'a>(
                e: &'a mut CreateEmbed,
                contents: &Vec<String>,
                kind: ShareKind,
                state: MarketState,
            ) -> &'a mut CreateEmbed {
                e.title(match kind {
                    ShareKind::Index => "관심 지수",
                    ShareKind::Stock => "관심 종목",
                });
                e.description(contents.join("\n"));
                e.color(match state {
                    MarketState::PreOpen => Colour::from_rgb(25, 118, 210),
                    MarketState::Close => Colour::from_rgb(97, 97, 97),
                    MarketState::Open => Colour::from_rgb(67, 160, 71),
                });
                e
            }

            match &mut result_msg {
                Some(result_msg) => {
                    // 메시지 수정.
                    result_msg
                        .edit(ctx, |m| {
                            m.embed(|e| embed_builder(e, &contents, target_kind, rep_state))
                        })
                        .await?;
                }
                None => {
                    // 수정할 새 메시지 생성.
                    let response = msg
                        .channel_id
                        .send_message(ctx, |m| {
                            m.embed(|e| embed_builder(e, &contents, target_kind, rep_state))
                        })
                        .await?;

                    // 중지 버튼 생성.
                    emoji_stop = response.react(&ctx, '🚫').await.ok();

                    result_msg = Some(response);
                }
            }
        }

        if time < max_edit {
            contents.clear();

            // 다음 데이터가 준비될 때까지 중지 리액션 기다리기.
            if let (Some(result_msg), Some(target_emoji)) = (&result_msg, &emoji_stop) {
                let answer = result_msg
                    .await_reaction(&ctx)
                    .timeout(wait_timeout)
                    .author_id(msg.author.id)
                    .await;

                if let Some(answer) = answer {
                    let emoji = &answer.as_inner_ref().emoji;
                    if *emoji == target_emoji.emoji {
                        break;
                    }
                }
            }
        }
    }

    if let Some(emoji_stop) = emoji_stop {
        emoji_stop.delete_all(ctx).await?;
    }

    Ok(())
}
