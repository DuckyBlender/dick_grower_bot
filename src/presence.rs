use crate::Bot;
use crate::commands::events::get_active_global_event;
use log::info;
use serenity::all::ActivityData;
use serenity::prelude::*;

// Update presence based on current event
pub async fn update_presence(ctx: &Context) {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    if let Some(event) = get_active_global_event(bot).await {
        let desc = format!("Event: {}", event.name);
        info!("Updating presence to: {}", desc);
        ctx.set_activity(Some(ActivityData::watching(desc)));
    } else {
        ctx.set_activity(Some(ActivityData::watching("No event active")));
    }
}
