mod client_data;
mod commands;
mod market;
mod naver;
mod trader;
mod util;

use std::{collections::HashSet, env, sync::mpsc, sync::Arc};

use tracing::{error, info};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use serenity::{
    async_trait,
    framework::standard::{
        help_commands,
        macros::{group, help},
        Args, CommandGroup, CommandResult, HelpOptions, StandardFramework,
    },
    futures::future::join_all,
    http::Http,
    model::prelude::*,
    prelude::*,
};
use tokio::{
    fs::OpenOptions,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
};

use client_data::*;
use commands::basic::*;
use commands::finance::*;
use market::{Market, ShareKind};
use naver::api;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        info!("Connected as {}", ready.user.name);
    }

    async fn resume(&self, _: Context, _: ResumedEvent) {
        info!("Resumed");
    }
}

#[group]
#[commands(ping, quit)]
struct General;

#[group]
#[commands(show_index, show_stock)]
struct Finance;

#[help]
async fn my_help(
    context: &Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    let _ = help_commands::with_embeds(context, msg, args, &help_options, groups, owners).await;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // This will load the environment variables located at `./.env`.
    dotenv::dotenv().expect("Failed to load .env file");

    // Initialize the logger to use environment variables.
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to start the logger");

    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let main_channel: u64 = env::var("DISCORD_CHANNEL")
        .map(|val| val.parse().expect("Can not parse channel"))
        .expect("Expected a channel in the environment");

    let http = Arc::new(Http::new_with_token(&token));

    let mut quit_channels = Vec::new();
    let mut traders = Vec::new();

    let market_one = Arc::new(RwLock::new(Market::new()));

    // Load my index.
    let index_path = "my_index.txt";
    if let Ok(index_file) = OpenOptions::new().read(true).open(index_path).await {
        let mut index_lines = BufReader::new(index_file).lines();
        let mut market = market_one.write().await;

        while let Ok(Some(code)) = index_lines.next_line().await {
            if !code.is_empty() {
                info!("Load index {}", code);
                let index = api::get_index(&code)
                    .await
                    .expect(&format!("Load {}", code));

                market.add_or_update_index(&code, &index);
            }
        }
    }

    // Load my stock.
    let stock_path = "my_stock.txt";
    /*if let Ok(stock_file) = OpenOptions::new().read(true).open(stock_path).await {
        let mut stock_lines = BufReader::new(stock_file).lines();
        let mut market = market_one.write().await;

        while let Ok(Some(code)) = stock_lines.next_line().await {
            if !code.is_empty() {
                info!("Load stock {}", code);
                let stock = api::get_stock(&code)
                    .await
                    .expect(&format!("Load {}", code));

                market.add_or_update_stock(&code, &stock);
            }
        }
    }*/

    // Start traders.
    {
        let (tx_quit, rx_quit) = mpsc::channel();
        let discord = Arc::clone(&http);
        let market = Arc::clone(&market_one);
        let handle = tokio::spawn(async move {
            trader::update_market(discord, main_channel, rx_quit, market).await
        });
        quit_channels.push(tx_quit);
        traders.push(handle);

        let (tx_quit, rx_quit) = mpsc::channel();
        let discord = Arc::clone(&http);
        let market = Arc::clone(&market_one);
        let handle = tokio::spawn(async move {
            trader::notify_market_state(discord, main_channel, rx_quit, market).await
        });
        quit_channels.push(tx_quit);
        traders.push(handle);
    }

    // Fetch bot's owners and id.
    let (owners, _bot_id) = match http.get_current_application_info().await {
        Ok(info) => {
            let mut owners = HashSet::new();
            owners.insert(info.owner.id);

            (owners, info.id)
        }
        Err(why) => panic!("Could not access application info: {:?}", why),
    };

    // Create the framework.
    let framework = StandardFramework::new()
        .configure(|c| c.owners(owners).prefix("!"))
        .help(&MY_HELP)
        .group(&GENERAL_GROUP)
        .group(&FINANCE_GROUP);

    let mut client = Client::builder(&token)
        .framework(framework)
        .event_handler(Handler)
        .await
        .expect("Err creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<ShardManagerContainer>(Arc::clone(&client.shard_manager));
        data.insert::<MarketContainer>(Arc::clone(&market_one));
    }

    let shard_manager = client.shard_manager.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Could not register ctrl+c handler");
        shard_manager.lock().await.shutdown_all().await;
    });

    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }

    for tx_quit in quit_channels {
        tx_quit.send(()).unwrap();
    }
    join_all(traders).await;

    // Save my index.
    for &(path, target_kind) in &[
        (index_path, ShareKind::Index),
        (stock_path, ShareKind::Stock),
    ] {
        if let Ok(mut file) = OpenOptions::new().write(true).create(true).open(path).await {
            let market = market_one.read().await;

            for (code, kind) in market.share_codes_with_kind() {
                if kind == target_kind {
                    file.write_all(code.as_bytes()).await?;
                    file.write_all(b"\n").await?;
                }
            }
        }
    }

    Ok(())
}
