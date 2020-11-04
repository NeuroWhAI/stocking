use std::time::{SystemTime, UNIX_EPOCH};

use serenity::framework::standard::{macros::command, Args, CommandResult};
use serenity::model::prelude::*;
use serenity::prelude::*;

use crate::naver::api;
use crate::util::*;

#[command]
#[aliases("index")]
async fn show_index(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let name = args.rest().trim();
    let name = if name.is_empty() { "KOSPI" } else { name };

    match api::get_index(name).await {
        Ok(index) => {
            msg.channel_id
                .send_message(&ctx.http, |m| {
                    m.embed(|e| {
                        e.title(name);
                        e.description(format!(
                            "{}　{}{}　{:.2}%",
                            format_value(index.now_value, 2),
                            get_change_value_char(index.change_value),
                            format_value(index.change_value, 2),
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
                            format_value(stock.change_value(), 0),
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
