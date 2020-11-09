use std::sync::Arc;

use serenity::{
    client::bridge::gateway::ShardManager,
    prelude::{Mutex, RwLock, TypeMapKey},
};

pub(crate) struct ShardManagerContainer;

impl TypeMapKey for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}

pub(crate) struct MarketContainer;

impl TypeMapKey for MarketContainer {
    type Value = Arc<RwLock<crate::market::Market>>;
}
