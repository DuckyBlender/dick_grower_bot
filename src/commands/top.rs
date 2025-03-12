use crate::Bot;
use log::error;
use serenity::all::{
    CommandInteraction, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use serenity::model::id::UserId;
use serenity::prelude::*;

pub async fn handle_top_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> CreateInteractionResponse {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    let guild_id = command.guild_id.unwrap().to_string();

    // Get top 10 users in this server
    let top_users = match sqlx::query!(
        "SELECT user_id, length FROM dicks 
         WHERE guild_id = ? 
         ORDER BY length DESC LIMIT 10",
        guild_id
    )
    .fetch_all(&bot.database)
    .await
    {
        Ok(users) => users,
        Err(why) => {
            error!("Error fetching top users: {:?}", why);
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Leaderboard Error")
                        .description(
                            "Failed to measure all the dicks. Some were too small to find.",
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
                        "Nobody has grown their dick in this server yet. Be the first one!",
                    )
                    .color(0xAAAAAA),
            ),
        );
    }

    // Build the leaderboard
    let mut description = "Here are the biggest dicks in this server:\n\n".to_string();

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

        let emoji = if i == 0 {
            "🍆 "
        } else if user.length <= 0 {
            "🥜 "
        } else if user.length > 30 {
            "🌵 "
        } else {
            ""
        };

        description.push_str(&format!(
            "{} **{}. {}{}**: {} cm\n",
            medal,
            i + 1,
            emoji,
            username,
            user.length
        ));
    }

    // Add funny comment about the winner
    if !top_users.is_empty() {
        let winner_name = match UserId::new(top_users[0].user_id.parse::<u64>().unwrap_or_default())
            .to_user(&ctx)
            .await
        {
            Ok(user) => user.name,
            Err(_) => "Unknown User".to_string(),
        };

        let length = top_users[0].length;
        let winner_comment = if length > 50 {
            format!(
                "Holy moly! {}' dick is so big it needs its own ZIP code!",
                winner_name
            )
        } else if length > 30 {
            format!(
                "Beware of {} in tight spaces. That thing is a lethal weapon!",
                winner_name
            )
        } else if length > 15 {
            format!(
                "{} is doing quite well. Impressive... most impressive.",
                winner_name
            )
        } else if length > 0 {
            format!(
                "{} is trying their best, though. Gold star for effort!",
                winner_name
            )
        } else {
            format!(
                "Poor {}... we need a microscope to find their dick.",
                winner_name
            )
        };

        description.push_str(&format!("\n\n{}", winner_comment));
    }

    let guild_name = match command.guild_id.unwrap().to_partial_guild(&ctx).await {
        Ok(guild) => guild.name,
        Err(_) => "This Server".to_string(),
    };

    CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new().add_embed(
            CreateEmbed::new()
                .title(format!("🍆 Dick Leaderboard: {} 🏆", guild_name))
                .description(description)
                .color(0x9B59B6) // Purple
                .footer(CreateEmbedFooter::new(
                    "Use /grow daily to increase your length!",
                )),
        ),
    )
}
