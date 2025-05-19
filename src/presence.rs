use crate::Bot;
use crate::utils::get_bot_stats;
use log::{info, warn};
use serenity::all::ActivityData;
use serenity::prelude::*;

// Update presence based on current stats
pub async fn update_presence(ctx: &Context) {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    match get_bot_stats(ctx, bot).await {
        Ok(stats) => {
            let desc = format!(
                "{} servers & {} dicks",
                stats.server_count, stats.dick_count
            );
            info!("Updating presence to: {}", desc);
            ctx.set_activity(Some(ActivityData::watching(desc)));
        }
        Err(e) => {
            warn!("Error fetching bot stats for presence update: {:?}", e);
            let desc = "? servers & ? dicks";
            ctx.set_activity(Some(ActivityData::watching(desc)));
        }
    }
}
