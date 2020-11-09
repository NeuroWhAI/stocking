use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serenity::model::prelude::*;
use serenity::prelude::*;
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    futures::future::join_all,
};

use crate::util::*;
use crate::{client_data::MarketContainer, naver::api};

#[command]
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
                            "{}　{}{}　{:.2}%",
                            format_value(index.now_value, 2),
                            get_change_value_char(index.change_value),
                            format_value(index.change_value.abs(), 2),
                            index.change_rate
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
#[aliases("stock")]
async fn show_stock(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let code = args.rest().trim();

    match api::get_stock(code).await {
        Ok(stock) => {
            msg.channel_id
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
            Ok(())
        }
        Err(err) => {
            msg.reply(ctx, err.to_string()).await?;
            Err(err.into())
        }
    }
}
