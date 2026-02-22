use crate::Bot;
use log::{error, info};
use serenity::all::{
    CommandInteraction, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use serenity::prelude::*;

const PRESTIGE_BASE_REQUIRED_LENGTH: i64 = 1000;
const PRESTIGE_REQUIREMENT_MULTIPLIER: f64 = 1.12;
const PRESTIGE_BONUS_MULTIPLIER: f64 = 0.1;

fn required_length_for_next_prestige(current_prestige_level: i64) -> i64 {
    let level = current_prestige_level.clamp(0, 50) as i32;
    (PRESTIGE_BASE_REQUIRED_LENGTH as f64 * PRESTIGE_REQUIREMENT_MULTIPLIER.powi(level)).round()
        as i64
}

pub fn calculate_prestige_growth_bonus(prestige_level: i64) -> i64 {
    let level = prestige_level.max(0) as f64;
    level.sqrt().round() as i64
}

pub async fn handle_prestige_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> Result<(), serenity::Error> {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    let user_id = command.user.id.to_string();
    let guild_id = command.guild_id.unwrap().to_string();

    // Get user's current stats
    let (current_length, current_prestige_level, current_prestige_points) = match sqlx::query!(
        "SELECT length, prestige_level, prestige_points FROM dicks WHERE user_id = ? AND guild_id = ?",
        user_id,
        guild_id
    )
    .fetch_optional(&bot.database)
    .await
    {
        Ok(Some(stats)) => (stats.length, stats.prestige_level, stats.prestige_points),
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
                        Ok(stats) => (stats.length, stats.prestige_level, stats.prestige_points),
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

    let required_length = required_length_for_next_prestige(current_prestige_level);

    // Check if user has enough length to prestige
    if current_length < required_length {
        let builder = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("🍆 Not Ready to Prestige")
                        .description(format!(
                            "You need at least **{} cm** to prestige, but you only have **{} cm**.\n\nGrow harder to reach the next reset milestone.",
                            required_length, current_length
                        ))
                        .color(0xFF5733)
                        .footer(CreateEmbedFooter::new(
                            "Every legend started small. Keep pumping those /grow reps.",
                        ))
                )
                .ephemeral(true),
        );
        return command.create_response(&ctx.http, builder).await;
    }

    // Calculate prestige bonus
    let new_prestige_level = current_prestige_level + 1;
    let bonus_points = (current_length as f64 * PRESTIGE_BONUS_MULTIPLIER).round() as i64;
    let new_prestige_points = current_prestige_points + bonus_points;

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

    // Reset length and update prestige info.
    // Guard with current level and required length so concurrent calls can't double-dip.
    let update_result = match sqlx::query!(
        "UPDATE dicks
         SET length = 0, prestige_level = ?, prestige_points = ?
         WHERE user_id = ? AND guild_id = ? AND length >= ? AND prestige_level = ?",
        new_prestige_level,
        new_prestige_points,
        user_id,
        guild_id,
        required_length,
        current_prestige_level
    )
    .execute(&mut *transaction)
    .await
    {
        Ok(result) => result,
        Err(why) => {
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
    };

    if update_result.rows_affected() == 0 {
        let _ = transaction.rollback().await;
        let builder = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Prestige Failed")
                        .description(
                            "Your stats changed while prestiging. Try /prestige again in a moment.",
                        )
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
        current_length
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

    let growth_bonus = calculate_prestige_growth_bonus(new_prestige_level);
    let next_required_length = required_length_for_next_prestige(new_prestige_level);

    // Create response with fun messages
    let (title, description, color) = if new_prestige_level == 1 {
        (
            "🍆 FIRST PRESTIGE! 🍆",
            format!(
                "Congratulations! You've prestiged your dick for the first time!\n\n\
                • Your dick has been reset to 0 cm\n\
                • You gained **{} prestige points**\n\
                • You are now prestige level **{}**\n\
                • You now get a **{} cm** bonus growth per /grow!\n\n\
                Next prestige target: **{} cm**.\n\
                Your dedication paid off. This is only the beginning.",
                bonus_points, new_prestige_level, growth_bonus, next_required_length
            ),
            0x00FF00, // Green
        )
    } else if new_prestige_level >= 10 {
        (
            "🌟 LEGENDARY PRESTIGE! 🌟",
            format!(
                "INCREDIBLE! You've reached prestige level **{}**!\n\n\
                • Your dick has been reset to 0 cm\n\
                • You gained **{} prestige points**\n\
                • You now have a total of **{} prestige points**\n\
                • You now get a **{} cm** bonus growth per /grow!\n\n\
                Next prestige target: **{} cm**.\n\
                You are now operating at certified monster status.",
                new_prestige_level,
                bonus_points,
                new_prestige_points,
                growth_bonus,
                next_required_length
            ),
            0xFFD700, // Gold
        )
    } else {
        (
            "🏆 Prestige Complete! 🏆",
            format!(
                "Well done! You've prestiged to level **{}**!\n\n\
                • Your dick has been reset to 0 cm\n\
                • You gained **{} prestige points**\n\
                • You now have a total of **{} prestige points**\n\
                • You now get a **{} cm** bonus growth per /grow!\n\n\
                Next prestige target: **{} cm**.\n\
                Keep grinding. Every prestige makes future growth stronger.",
                new_prestige_level,
                bonus_points,
                new_prestige_points,
                growth_bonus,
                next_required_length
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
                        "Bigger resets, bigger flex. Keep climbing the prestige ladder.",
                    )),
            ),
    );
    return command.create_response(&ctx.http, builder).await;
}
