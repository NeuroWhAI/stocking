use std::time::{SystemTime, UNIX_EPOCH};

use serenity::framework::standard::{macros::command, Args, CommandResult};
use serenity::model::prelude::*;
use serenity::prelude::*;
use serenity::utils::Colour;

use crate::naver::api;

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
                            format_value_with_base_100(index.now_value),
                            get_change_value_char(index.change_value),
                            format_value_with_base_100(index.change_value),
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
                            ("거래량(천주)", index.trading_volume.to_string(), true),
                            ("\u{200B}", "\u{200B}".into(), true),
                            ("거래대금(백만)", index.trading_value.to_string(), true),
                            ("장중최고", index.high_value.to_string(), true),
                            ("\u{200B}", "\u{200B}".into(), true),
                            ("장중최저", index.low_value.to_string(), true),
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

fn format_value_with_base_100(mut val: i64) -> String {
    let mut s = String::new();

    if val < 0 {
        val = -val;
        s.push('-');
    }

    if val >= 100000 {
        s.push_str(&(val / 100000).to_string());
        s.push(',');
    }

    s.push_str(&(val % 100000 / 100).to_string());

    s.push('.');
    s.push_str(&(val % 100).to_string());

    s
}

fn get_change_value_char(val: i64) -> char {
    if val > 0 {
        '▲'
    } else if val < 0 {
        '▼'
    } else {
        '='
    }
}

fn get_change_value_color(val: i64) -> Colour {
    if val > 0 {
        Colour::from_rgb(217, 4, 0)
    } else if val < 0 {
        Colour::from_rgb(0, 93, 222)
    } else {
        Colour::from_rgb(51, 51, 51)
    }
}
