use crate::Bot;
use crate::commands::seasons::{cached_guild_label, cached_user_label};
use crate::utils::get_bot_stats;
use log::{error, info};
use rand::seq::IndexedRandom;
use serenity::all::{
    CommandInteraction, CreateAllowedMentions, CreateEmbed, CreateEmbedFooter,
    CreateInteractionResponse, CreateInteractionResponseMessage,
};
use serenity::prelude::*;
use sqlx::Row;
use tokio::time::Instant;

pub async fn handle_global_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> Result<(), serenity::Error> {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();
    let query_started = Instant::now();
    let top_users = match sqlx::query(
        "SELECT user_id, length, guild_id FROM dicks
         ORDER BY length DESC, user_id ASC, guild_id ASC
         LIMIT 10",
    )
    .fetch_all(&bot.database)
    .await
    {
        Ok(users) => users,
        Err(why) => {
            error!("Error fetching global top users: {:?}", why);
            return command
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new().add_embed(
                            CreateEmbed::new()
                                .title("⚠️ Global Leaderboard Error")
                                .description("Failed to measure the global leaderboard.")
                                .color(0xFF0000),
                        ),
                    ),
                )
                .await;
        }
    };
    let query_elapsed = query_started.elapsed();

    if top_users.is_empty() {
        return command
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new().add_embed(
                        CreateEmbed::new()
                            .title("👀 No Dicks Found")
                            .description(
                                "Nobody has grown anywhere yet. The world awaits a pioneer!",
                            )
                            .color(0xAAAAAA),
                    ),
                ),
            )
            .await;
    }

    let render_started = Instant::now();
    let (server_count_str, dick_count_str) = match get_bot_stats(ctx, bot).await {
        Ok(stats) => (stats.server_count.to_string(), stats.dick_count.to_string()),
        Err(why) => {
            error!("Error fetching bot stats for global command: {:?}", why);
            ("?".to_string(), "?".to_string())
        }
    };
    let mut description = "Here are the biggest dicks in the entire world:\n\n".to_string();
    let mut winner_name = String::new();

    for (index, row) in top_users.iter().enumerate() {
        let user_id: String = row.try_get("user_id").unwrap_or_default();
        let guild_id: String = row.try_get("guild_id").unwrap_or_default();
        let length: i64 = row.try_get("length").unwrap_or_default();
        let username = cached_user_label(ctx, &user_id);
        if index == 0 {
            winner_name.clone_from(&username);
        }
        let medal = match index {
            0 => "🥇",
            1 => "🥈",
            2 => "🥉",
            _ => "🔹",
        };
        description.push_str(&format!(
            "{medal} **{}. {username}**: {length} cm (from {})\n",
            index + 1,
            cached_guild_label(ctx, &guild_id)
        ));
    }

    let comments = [
        format!("NASA wants to study {winner_name}'s dick as a possible space elevator!"),
        format!("{winner_name} must need a special permit to carry that thing around!"),
        format!("{winner_name} is making the rest of the world feel inadequate!"),
        format!("{winner_name} is the global champion..."),
    ];
    if let Some(comment) = comments.choose(&mut rand::rng()) {
        description.push_str(&format!("\n{comment}"));
    }
    let render_elapsed = render_started.elapsed();
    info!(
        "/global stages: database={}ms render={}ms (no per-row REST requests)",
        query_elapsed.as_millis(),
        render_elapsed.as_millis()
    );

    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .allowed_mentions(CreateAllowedMentions::new())
                    .add_embed(
                    CreateEmbed::new()
                        .title("🌍 Global Dick Leaderboard 🏆")
                        .description(description)
                        .color(0x9B59B6)
                        .footer(CreateEmbedFooter::new(format!(
                            "🌐 {server_count_str} servers | 🍆 {dick_count_str} total dicks | Use /grow to climb!"
                        ))),
                    ),
            ),
        )
        .await
}
