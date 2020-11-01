mod client_data;
mod commands;
mod naver;

use std::{collections::HashSet, env};

use tracing::{error, info};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use serenity::{
    async_trait,
    framework::standard::{
        Args, CommandResult, CommandGroup,
        HelpOptions, help_commands, StandardFramework,
        macros::{group, help},
    },
    prelude::*,
    http::Http,
    model::prelude::*,
};

use client_data::*;
use commands::basic::*;
use commands::finance::*;

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
#[commands(show_index)]
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
async fn main() {
    // This will load the environment variables located at `./.env`.
    dotenv::dotenv().expect("Failed to load .env file");

    // Initialize the logger to use environment variables.
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to start the logger");

    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let http = Http::new_with_token(&token);

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

    let mut client = Client::new(&token)
        .framework(framework)
        .event_handler(Handler)
        .await
        .expect("Err creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<ShardManagerContainer>(client.shard_manager.clone());
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
}
