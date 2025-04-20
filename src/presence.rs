use crate::Bot;
use log::{error, info};
use serenity::all::ActivityData;
use serenity::prelude::*;

// Update presence based on current stats
pub async fn update_presence(ctx: &Context) {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    // Get current guild count from context cache
    let guild_count = ctx.cache.guilds().len();

    // Count unique users from database
    let user_count = match sqlx::query!("SELECT COUNT(DISTINCT user_id) as count FROM dicks")
        .fetch_one(&bot.database)
        .await
    {
        Ok(result) => result.count.unwrap_or(0) as usize,
        Err(e) => {
            error!("Error counting users: {:?}", e);
            return;
        }
    };

    let desc = format!("{} servers & {} dicks", guild_count, user_count);
    info!("Updating presence to: {}", desc);

    ctx.set_activity(Some(ActivityData::watching(desc)));
}
