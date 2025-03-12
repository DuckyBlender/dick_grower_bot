use log::error;
use serenity::all::{
    CommandInteraction, CreateEmbed,
    CreateEmbedFooter, CreateInteractionResponse, CreateInteractionResponseMessage,
};
use serenity::model::id::UserId;
use serenity::prelude::*;
use crate::Bot;

pub async fn handle_global_command(ctx: &Context, _: &CommandInteraction) -> CreateInteractionResponse {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

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
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Global Leaderboard Error")
                        .description(
                            "Failed to measure all the world's dicks. The server is overwhelmed.",
                        )
                        .color(0xFF0000),
                ),
            );
        }
    };

    if top_users.is_empty() {
        return CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().add_embed(
                CreateEmbed::new()
                    .title("👀 No Dicks Found")
                    .description(
                        "Nobody has grown their dick anywhere yet. The world awaits a pioneer!",
                    )
                    .color(0xAAAAAA),
            ),
        );
    }

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
            Ok(user) => user.name,
            Err(_) => "Unknown User".to_string(),
        };

        let guild_name = match user.guild_id.parse::<u64>() {
            Ok(id) => match ctx.http.get_guild(id.into()).await {
                Ok(guild) => guild.name,
                Err(_) => "Unknown Server".to_string(),
            },
            Err(_) => "Unknown Server".to_string(),
        };

        let emoji = if i == 0 {
            "🌎 "
        } else if user.length <= 0 {
            "🥜 "
        } else if user.length > 50 {
            "🚀 "
        } else if user.length > 30 {
            "🌵 "
        } else {
            "🍆 "
        };

        description.push_str(&format!(
            "{} **{}. {}{}**: {} cm (from {})\n",
            medal,
            i + 1,
            emoji,
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
            Ok(user) => user.name,
            Err(_) => "Unknown User".to_string(),
        };

        let length = top_users[0].length;
        let winner_comment = if length > 60 {
            format!(
                "NASA wants to study {}'s dick as a possible space elevator!",
                winner_name
            )
        } else if length > 40 {
            format!(
                "{} must need a special permit to carry that thing around!",
                winner_name
            )
        } else if length > 20 {
            format!(
                "{} is making the rest of the world feel inadequate!",
                winner_name
            )
        } else {
            format!(
                "{} is the global champion... though the bar seems pretty low, honestly.",
                winner_name
            )
        };

        description.push_str(&format!("\n\n{}", winner_comment));
    }

    CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new().add_embed(
            CreateEmbed::new()
                .title("🌍 Global Dick Leaderboard 🏆")
                .description(description)
                .color(0x9B59B6) // Purple
                .footer(CreateEmbedFooter::new(
                    "World domination starts with your dick. Use /grow daily!",
                )),
        ),
    )
}