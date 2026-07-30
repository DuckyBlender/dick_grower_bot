use crate::Bot;
use crate::commands::escape_markdown;
use crate::commands::seasons::cached_user_label;
use log::{error, info};
use serenity::all::{
    CommandInteraction, CreateAllowedMentions, CreateEmbed, CreateEmbedFooter,
    CreateInteractionResponse, CreateInteractionResponseMessage,
};
use serenity::prelude::*;
use sqlx::Row;
use tokio::time::Instant;

pub async fn handle_top_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> Result<(), serenity::Error> {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();
    let guild_id = command.guild_id.expect("guild command").to_string();
    let query_started = Instant::now();
    let top_users = match sqlx::query(
        "SELECT user_id, length FROM dicks
         WHERE guild_id = ?
         ORDER BY length DESC, user_id ASC
         LIMIT 10",
    )
    .bind(&guild_id)
    .fetch_all(&bot.database)
    .await
    {
        Ok(users) => users,
        Err(why) => {
            error!("Error fetching server top users: {:?}", why);
            return command
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("The leaderboard is unavailable right now."),
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
                            .description("Nobody has grown in this server yet. Be the first one!")
                            .color(0xAAAAAA),
                    ),
                ),
            )
            .await;
    }

    let render_started = Instant::now();
    let mut description = "Here are the biggest dicks in this server:\n\n".to_string();
    let mut winner_name = String::new();
    let mut winner_length = 0;
    for (index, row) in top_users.iter().enumerate() {
        let user_id: String = row.try_get("user_id").unwrap_or_default();
        let length: i64 = row.try_get("length").unwrap_or_default();
        let username = cached_user_label(ctx, &user_id);
        if index == 0 {
            winner_name.clone_from(&username);
            winner_length = length;
        }
        let medal = match index {
            0 => "🥇",
            1 => "🥈",
            2 => "🥉",
            _ => "🔹",
        };
        description.push_str(&format!(
            "{medal} **{}. {username}**: {length} cm\n",
            index + 1
        ));
    }

    let winner_comment = if winner_length > 50 {
        format!("Holy moly! {winner_name}'s dick is so big it needs its own ZIP code!")
    } else if winner_length > 30 {
        format!("Beware of {winner_name} in tight spaces. That thing is a lethal weapon!")
    } else if winner_length > 15 {
        format!("{winner_name} is doing quite well. Impressive... most impressive.")
    } else if winner_length > 0 {
        format!("{winner_name} is trying their best, though. Gold star for effort!")
    } else {
        format!("Poor {winner_name}... we need a microscope to find their dick.")
    };
    description.push_str(&format!("\n{winner_comment}"));

    let guild_name = command
        .guild_id
        .and_then(|id| ctx.cache.guild(id))
        .map(|guild| escape_markdown(&guild.name))
        .unwrap_or_else(|| "This Server".to_string());
    info!(
        "/top stages: database={}ms render={}ms (no per-row REST requests)",
        query_elapsed.as_millis(),
        render_started.elapsed().as_millis()
    );
    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .allowed_mentions(CreateAllowedMentions::new())
                    .add_embed(
                        CreateEmbed::new()
                            .title(format!("🍆 Dick Leaderboard: {guild_name} 🏆"))
                            .description(description)
                            .color(0x9B59B6)
                            .footer(CreateEmbedFooter::new(
                                "Use /grow daily to increase your length!",
                            )),
                    ),
            ),
        )
        .await
}
