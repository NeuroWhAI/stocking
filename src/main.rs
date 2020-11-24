mod alarm;
mod client_data;
mod commands;
mod market;
mod naver;
mod trader;
mod util;

use std::{collections::HashSet, env, path::PathBuf, sync::mpsc, sync::Arc};

use anyhow::bail;
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
    fs::{self, OpenOptions},
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
};

use alarm::StockAlarm;
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
#[commands(
    show_index,
    show_stock,
    show_my_indices,
    show_my_stocks,
    set_alarm,
    off_alarm,
    show_alarms
)]
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
    if let Ok(stock_file) = OpenOptions::new().read(true).open(stock_path).await {
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
    }

    let stock_alarms = Arc::new(RwLock::new(StockAlarm::new()));

    // Load my alarms.
    let alarm_folder = "my_alarms";
    if fs::metadata(&alarm_folder).await.is_ok() {
        let mut files = fs::read_dir(&alarm_folder).await?;
        while let Some(file) = files.next_entry().await? {
            let path = file.path();
            let code = path
                .file_stem()
                .and_then(|os_str| os_str.to_str())
                .expect("file name without extension");

            info!("Load alarms for {}", code);
            let alarms = load_alarms(&path).await?;
            info!("{} alarms loaded", alarms.len());

            let mut manager = stock_alarms.write().await;
            for target_value in alarms {
                manager.set_alarm(code, target_value)
            }
        }
    } else {
        // Create a folder for alarms if it doesn't exists.
        fs::create_dir(&alarm_folder).await?;
    }

    // Start traders.
    {
        let (tx_quit, rx_quit) = mpsc::channel();
        let discord = Arc::clone(&http);
        let market = Arc::clone(&market_one);
        let stock_alarms = Arc::clone(&stock_alarms);
        let handle = tokio::spawn(async move {
            trader::update_market(discord, main_channel, rx_quit, market, stock_alarms).await
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

        let (tx_quit, rx_quit) = mpsc::channel();
        let discord = Arc::clone(&http);
        let market = Arc::clone(&market_one);
        let handle = tokio::spawn(async move {
            trader::notify_change_rate(discord, main_channel, rx_quit, market).await
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
        data.insert::<AlarmContainer>(Arc::clone(&stock_alarms));
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
        if let Ok(mut file) = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(path)
            .await
        {
            let market = market_one.read().await;

            for (code, kind) in market.share_codes_with_kind() {
                if kind == target_kind {
                    file.write_all(code.as_bytes()).await?;
                    file.write_all(b"\n").await?;
                }
            }
        }
    }

    // Save my alarms.
    let stock_alarms = stock_alarms.read().await;
    let alarm_codes = stock_alarms.codes();
    for &code in &alarm_codes {
        if let Some(alarms) = stock_alarms.get_alarms(code) {
            let mut path = PathBuf::new();
            path.push(alarm_folder);
            path.push(code);
            path.set_extension("txt");

            save_alarms(&path, alarms).await?;
        }
    }

    // 목록에 없는 종목의 알람 파일은 삭제.
    let mut alarm_files = fs::read_dir(&alarm_folder).await?;
    while let Some(file) = alarm_files.next_entry().await? {
        let path = file.path();
        let code = path
            .file_stem()
            .and_then(|os_str| os_str.to_str())
            .expect("file name without extension");

        if alarm_codes.iter().find(|&&c| c == code).is_none() {
            if let Err(why) = fs::remove_file(path).await {
                error!("Fail to remove alarm file: {:?}", why);
            }
        }
    }

    Ok(())
}

async fn load_alarms(path: &PathBuf) -> anyhow::Result<Vec<i64>> {
    if let Ok(file) = OpenOptions::new().read(true).open(path).await {
        let mut lines = BufReader::new(file).lines();
        let mut alarms = Vec::new();

        while let Ok(Some(target_value)) = lines.next_line().await {
            if let Ok(target_value) = target_value.parse() {
                alarms.push(target_value);
            }
        }

        Ok(alarms)
    } else {
        bail!("Fail to load alarms");
    }
}

async fn save_alarms(path: &PathBuf, alarms: &Vec<i64>) -> anyhow::Result<()> {
    if let Ok(mut file) = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(path)
        .await
    {
        for target_value in alarms {
            file.write_all(target_value.to_string().as_bytes()).await?;
            file.write_all(b"\n").await?;
        }
    }

    Ok(())
}
