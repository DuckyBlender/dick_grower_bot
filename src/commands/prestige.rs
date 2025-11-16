use crate::Bot;
use crate::commands::escape_markdown;
use log::{error, info};
use serenity::all::{
    CommandInteraction, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use serenity::prelude::*;

const PRESTIGE_REQUIRED_LENGTH: i64 = 1000; // Need 1000cm to prestige
const PRESTIGE_BONUS_MULTIPLIER: f64 = 0.1; // 10% bonus per prestige level

pub async fn handle_prestige_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> Result<(), serenity::Error> {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    let user_id = command.user.id.to_string();
    let guild_id = command.guild_id.unwrap().to_string();

    // Get user's current stats
    let user_stats = match sqlx::query!(
        "SELECT length, prestige_level, prestige_points FROM dicks WHERE user_id = ? AND guild_id = ?",
        user_id,
        guild_id
    )
    .fetch_optional(&bot.database)
    .await
    {
        Ok(Some(stats)) => stats,
        Ok(None) => {
            // Create new user with 0 length
            info!(
                "New user detected, adding user {} ({}) in guild id {} to database",
                command.user.name, user_id, guild_id
            );
            match sqlx::query!(
                "INSERT INTO dicks (user_id, guild_id, length, last_grow, growth_count, dick_of_day_count, 
                                   pvp_wins, pvp_losses, pvp_max_streak, pvp_current_streak,
                                   cm_won, cm_lost, prestige_level, prestige_points)
                 VALUES (?, ?, 0, datetime('now', '-2 days'), 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)",
                user_id,
                guild_id
            )
            .execute(&bot.database)
            .await
            {
                Ok(_) => {
                    // Fetch the newly created record
                    match sqlx::query!(
                        "SELECT length, prestige_level, prestige_points FROM dicks WHERE user_id = ? AND guild_id = ?",
                        user_id,
                        guild_id
                    )
                    .fetch_one(&bot.database)
                    .await
                    {
                        Ok(stats) => stats,
                        Err(why) => {
                            error!("Error fetching new user stats: {:?}", why);
                            let builder = CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .add_embed(
                                        CreateEmbed::new()
                                            .title("⚠️ Database Error")
                                            .description("Failed to create your user record.")
                                            .color(0xFF0000),
                                    )
                                    .ephemeral(true),
                            );
                            return command.create_response(&ctx.http, builder).await;
                        }
                    }
                }
                Err(why) => {
                    error!("Error creating user: {:?}", why);
                    let builder = CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .add_embed(
                                CreateEmbed::new()
                                    .title("⚠️ Database Error")
                                    .description("Failed to create your user record.")
                                    .color(0xFF0000),
                            )
                            .ephemeral(true),
                    );
                    return command.create_response(&ctx.http, builder).await;
                }
            }
        }
        Err(why) => {
            error!("Database error: {:?}", why);
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("⚠️ Database Error")
                            .description("Failed to retrieve your stats.")
                            .color(0xFF0000),
                    )
                    .ephemeral(true),
            );
            return command.create_response(&ctx.http, builder).await;
        }
    };

    // Check if user has enough length to prestige
    if user_stats.length < PRESTIGE_REQUIRED_LENGTH {
        let builder = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("🌱 Not Ready to Prestige")
                        .description(format!(
                            "You need at least **{} cm** to prestige, but you only have **{} cm**.\n\nKeep growing your plant to reach the required size!",
                            PRESTIGE_REQUIRED_LENGTH, user_stats.length
                        ))
                        .color(0xFF5733)
                        .footer(CreateEmbedFooter::new(
                            "Every big plant started as a small seed. Keep nurturing yours!",
                        ))
                )
                .ephemeral(true),
        );
        return command.create_response(&ctx.http, builder).await;
    }

    // Calculate prestige bonus
    let new_prestige_level = user_stats.prestige_level + 1;
    let bonus_points = (user_stats.length as f64 * PRESTIGE_BONUS_MULTIPLIER).round() as i64;
    let new_prestige_points = user_stats.prestige_points + bonus_points;

    // Start transaction
    let mut transaction = match bot.database.begin().await {
        Ok(tx) => tx,
        Err(why) => {
            error!("Error starting transaction: {:?}", why);
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("⚠️ Transaction Error")
                            .description("Failed to start prestige transaction.")
                            .color(0xFF0000),
                    )
                    .ephemeral(true),
            );
            return command.create_response(&ctx.http, builder).await;
        }
    };

    // Reset length to 0 and update prestige info
    if let Err(why) = sqlx::query!(
        "UPDATE dicks SET length = 0, prestige_level = ?, prestige_points = ? WHERE user_id = ? AND guild_id = ?",
        new_prestige_level,
        new_prestige_points,
        user_id,
        guild_id
    )
    .execute(&mut *transaction)
    .await
    {
        error!("Error updating prestige: {:?}", why);
        let _ = transaction.rollback().await;
        let builder = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Prestige Failed")
                        .description("Failed to process the prestige. Transaction rolled back.")
                        .color(0xFF0000),
                )
                .ephemeral(true),
        );
        return command.create_response(&ctx.http, builder).await;
    }

    // Log the prestige event
    if let Err(why) = sqlx::query!(
        "INSERT INTO prestige_history (user_id, guild_id, prestige_level, length_before_reset)
         VALUES (?, ?, ?, ?)",
        user_id,
        guild_id,
        new_prestige_level,
        user_stats.length
    )
    .execute(&mut *transaction)
    .await
    {
        error!("Error logging prestige history: {:?}", why);
    }

    // Commit transaction
    if let Err(why) = transaction.commit().await {
        error!("Error committing prestige transaction: {:?}", why);
        let builder = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Prestige Failed")
                        .description("Failed to commit the prestige transaction.")
                        .color(0xFF0000),
                )
                .ephemeral(true),
        );
        return command.create_response(&ctx.http, builder).await;
    }

    // Calculate growth bonus based on prestige level
    let growth_bonus = (new_prestige_level as f64 * 0.5).round() as i64; // 0.5cm bonus per prestige level

    // Create response with fun messages
    let (title, description, color) = if new_prestige_level == 1 {
        (
            "🌱 FIRST PRESTIGE! 🌱",
            format!(
                "Congratulations! You've successfully prestiged your plant for the first time!\n\n\
                • Your plant has been reset to 0 cm\n\
                • You gained **{} prestige points**\n\
                • You are now prestige level **{}**\n\
                • You now get a **{} cm** bonus growth per /grow!\n\n\
                Your dedication has paid off! This is just the beginning of your journey to becoming a plant master.",
                bonus_points, new_prestige_level, growth_bonus
            ),
            0x00FF00, // Green
        )
    } else if new_prestige_level >= 10 {
        (
            "🌟 LEGENDARY PRESTIGE! 🌟",
            format!(
                "INCREDIBLE! You've reached prestige level **{}**!\n\n\
                • Your plant has been reset to 0 cm\n\
                • You gained **{} prestige points**\n\
                • You now have a total of **{} prestige points**\n\
                • You now get a **{} cm** bonus growth per /grow!\n\n\
                You are a true plant legend! Your green thumb is unmatched!",
                new_prestige_level, bonus_points, new_prestige_points, growth_bonus
            ),
            0xFFD700, // Gold
        )
    } else {
        (
            "🌿 Prestige Complete! 🌿",
            format!(
                "Well done! You've prestiged your plant to level **{}**!\n\n\
                • Your plant has been reset to 0 cm\n\
                • You gained **{} prestige points**\n\
                • You now have a total of **{} prestige points**\n\
                • You now get a **{} cm** bonus growth per /grow!\n\n\
                Each prestige makes you stronger! Keep growing!",
                new_prestige_level, bonus_points, new_prestige_points, growth_bonus
            ),
            0x32CD32, // Lime Green
        )
    };

    let builder = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .title(title)
                    .description(description)
                    .color(color)
                    .footer(CreateEmbedFooter::new(
                        "With each prestige, your plant grows stronger!",
                    )),
            ),
    );
    return command.create_response(&ctx.http, builder).await;
}
