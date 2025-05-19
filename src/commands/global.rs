use crate::Bot;
use crate::commands::escape_markdown;
use crate::utils::get_bot_stats;
use log::error;
use rand::seq::IndexedRandom;
use serenity::all::{
    CommandInteraction, CreateEmbed, CreateEmbedFooter, CreateInteractionResponseFollowup,
};
use serenity::model::id::UserId;
use serenity::prelude::*;

pub async fn handle_global_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> Result<(), serenity::Error> {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    // Defer the command to avoid timeout
    // This is important for commands that take a while to process
    command.defer(&ctx.http).await?;

    // Get top 10 users globally
    let top_users = match sqlx::query!(
        "SELECT user_id, length, guild_id FROM dicks 
         ORDER BY length DESC LIMIT 10"
    )
    .fetch_all(&bot.database)
    .await
    {
        Ok(users) => users,
        Err(why) => {
            error!("Error fetching global top users: {:?}", why);
            command.create_followup(&ctx.http,
                CreateInteractionResponseFollowup::new().add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Global Leaderboard Error")
                        .description(
                            "Failed to measure all the world's dicks. The server is overwhelmed.",
                        )
                        .color(0xFF0000),
                ),
            ).await?;
            return Ok(());
        }
    };

    if top_users.is_empty() {
        command
            .create_followup(
                &ctx.http,
                CreateInteractionResponseFollowup::new().add_embed(
                    CreateEmbed::new()
                        .title("👀 No Dicks Found")
                        .description(
                            "Nobody has grown their dick anywhere yet. The world awaits a pioneer!",
                        )
                        .color(0xAAAAAA),
                ),
            )
            .await?;
        return Ok(());
    }

    // Fetch bot stats
    let (server_count_str, dick_count_str) = match get_bot_stats(ctx, bot).await {
        Ok(stats) => (stats.server_count.to_string(), stats.dick_count.to_string()),
        Err(why) => {
            error!("Error fetching bot stats for global command: {:?}", why);
            ("?".to_string(), "?".to_string()) // Use "?" on error
        }
    };

    // Build the global leaderboard
    let mut description = "Here are the biggest dicks in the entire world:\n\n".to_string();

    for (i, user) in top_users.iter().enumerate() {
        let medal = match i {
            0 => "🥇",
            1 => "🥈",
            2 => "🥉",
            _ => "🔹",
        };

        let username = match UserId::new(user.user_id.parse::<u64>().unwrap_or_default())
            .to_user(&ctx)
            .await
        {
            Ok(user) => escape_markdown(&user.name),
            Err(_) => "Unknown User".to_string(),
        };

        let guild_name = match user.guild_id.parse::<u64>() {
            Ok(id) => match ctx.http.get_guild(id.into()).await {
                Ok(guild) => {
                    // Only show guild name if it's a community server (public)
                    if guild.features.contains(&"COMMUNITY".to_string()) {
                        escape_markdown(&guild.name)
                    } else {
                        "private server".to_string()
                    }
                }
                Err(_) => "unknown server".to_string(),
            },
            Err(_) => "unknown server".to_string(),
        };

        description.push_str(&format!(
            "{} **{}. {}**: {} cm (from {})\n",
            medal,
            i + 1,
            username,
            user.length,
            guild_name
        ));
    }

    // Add funny comment about the global champion
    if !top_users.is_empty() {
        let winner_name = match UserId::new(top_users[0].user_id.parse::<u64>().unwrap_or_default())
            .to_user(&ctx)
            .await
        {
            Ok(user) => escape_markdown(&user.name),
            Err(_) => "Unknown User".to_string(),
        };

        let comments = [
            format!(
                "NASA wants to study {}'s dick as a possible space elevator!",
                winner_name
            ),
            format!(
                "{} must need a special permit to carry that thing around!",
                winner_name
            ),
            format!(
                "{} is making the rest of the world feel inadequate!",
                winner_name
            ),
            format!("{} is the global champion...", winner_name),
        ];

        // Select random comment
        let winner_comment = comments.choose(&mut rand::rng()).unwrap();

        description.push_str(&format!("\n\n{}", winner_comment));
    }

    command.create_followup(&ctx.http,
        CreateInteractionResponseFollowup::new().add_embed(
            CreateEmbed::new()
                .title("🌍 Global Dick Leaderboard 🏆")
                .description(description)
                .color(0x9B59B6) // Purple
                .footer(CreateEmbedFooter::new(
                    format!(
                        "🌐 {} servers | 🍆 {} total dicks | World domination starts with your dick. Start growing today with /grow!",
                        server_count_str, dick_count_str
                    )
                )),
        ),
    ).await?;

    Ok(())
}
