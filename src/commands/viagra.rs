use crate::Bot;

use chrono::{Duration, NaiveDateTime};
use log::{error, info};
use serenity::all::{
    CommandInteraction, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use serenity::prelude::*;

const VIAGRA_COOLDOWN_HOURS: i64 = 20;
const VIAGRA_DURATION_HOURS: i64 = 6; // 6 hours of effect

pub async fn handle_viagra_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> Result<(), serenity::Error> {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    let user_id = command.user.id.to_string();
    let guild_id = command.guild_id.unwrap().to_string();

    // Check user's viagra status
    let user_status = match sqlx::query!(
        "SELECT viagra_last_used, viagra_active_until FROM dicks WHERE user_id = ? AND guild_id = ?",
        user_id,
        guild_id
    )
    .fetch_optional(&bot.database)
    .await
    {
        Ok(Some(record)) => (record.viagra_last_used, record.viagra_active_until),
        Ok(None) => {
            // New user, create a record
            info!(
                "New user detected for viagra, adding user {} ({}) in guild id {} to database",
                command.user.name, user_id, guild_id
            );
            match sqlx::query!(
                "INSERT INTO dicks (user_id, guild_id, length, last_grow, growth_count, dick_of_day_count, 
                                   pvp_wins, pvp_losses, pvp_max_streak, pvp_current_streak,
                                   cm_won, cm_lost)
                 VALUES (?, ?, 0, datetime('now', '-2 days'), 0, 0, 0, 0, 0, 0, 0, 0)",
                user_id,
                guild_id
            )
            .execute(&bot.database)
            .await
            {
                Ok(_) => (),
                Err(why) => {
                    error!("Error creating user for viagra: {:?}", why);
                }
            };
            (None, None)
        }
        Err(why) => {
            error!("Database error checking viagra status: {:?}", why);
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Database Error")
                        .description("Failed to check your viagra status. The pharmacy is closed.")
                        .color(0xFF0000),
                ),
            );
            return command.create_response(&ctx.http, builder).await;
        }
    };

    let now = chrono::Utc::now().naive_utc();

    // Check if viagra is currently active
    if let Some(active_until_str) = user_status.1 {
        if let Ok(active_until) = NaiveDateTime::parse_from_str(&active_until_str, "%Y-%m-%d %H:%M:%S") {
            if now < active_until {
                let time_left = active_until - now;
                let unix_timestamp = chrono::Utc::now().timestamp() + time_left.num_seconds();
                let discord_timestamp = format!("<t:{}:R>", unix_timestamp);

                let builder = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .add_embed(
                            CreateEmbed::new()
                                .title("💊 Viagra Already Active!")
                                .description(format!(
                                    "Your viagra is still working its magic! 🔥\n\nEffect ends: {}\n\nYou'll get +20% growth until then. No need to double dose!",
                                    discord_timestamp
                                ))
                                .color(0x3498DB)
                                .footer(CreateEmbedFooter::new(
                                    "Patience, young grasshopper. Good things come to those who wait.",
                                ))
                        )
                        .ephemeral(true)
                );
                return command.create_response(&ctx.http, builder).await;
            }
        }
    }

    // Check cooldown
    if let Some(last_used_str) = user_status.0 {
        if let Ok(last_used) = NaiveDateTime::parse_from_str(&last_used_str, "%Y-%m-%d %H:%M:%S") {
            let time_since_last = now - last_used;
            let cooldown_remaining = Duration::hours(VIAGRA_COOLDOWN_HOURS) - time_since_last;

            if !cooldown_remaining.is_zero() && cooldown_remaining > Duration::zero() {
                let unix_timestamp = chrono::Utc::now().timestamp() + cooldown_remaining.num_seconds();
                let discord_timestamp = format!("<t:{}:R>", unix_timestamp);

                let builder = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .add_embed(
                            CreateEmbed::new()
                                .title("🚫 Viagra Cooldown Active")
                                .description(format!(
                                    "Whoa there, speedster! You need to wait before taking another viagra.\n\nCooldown ends: {}\n\nYour body needs time to recover from the last enhancement session.",
                                    discord_timestamp
                                ))
                                .color(0xFF5733)
                                .footer(CreateEmbedFooter::new(
                                    "Remember: Too much enhancement can lead to... complications.",
                                ))
                        )
                        .ephemeral(true)
                );
                return command.create_response(&ctx.http, builder).await;
            }
        }
    }

    // Activate viagra
    let active_until = now + Duration::hours(VIAGRA_DURATION_HOURS);
    let active_until_str = active_until.format("%Y-%m-%d %H:%M:%S").to_string();
    let now_str = now.format("%Y-%m-%d %H:%M:%S").to_string();

    match sqlx::query!(
        "UPDATE dicks SET viagra_last_used = ?, viagra_active_until = ? WHERE user_id = ? AND guild_id = ?",
        now_str,
        active_until_str,
        user_id,
        guild_id
    )
    .execute(&bot.database)
    .await
    {
        Ok(_) => (),
        Err(why) => {
            error!("Error activating viagra: {:?}", why);
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Activation Failed")
                        .description("Failed to activate viagra. The pharmacy system is down.")
                        .color(0xFF0000),
                ),
            );
            return command.create_response(&ctx.http, builder).await;
        }
    };

    // Calculate when effect ends
    let effect_ends_unix = chrono::Utc::now().timestamp() + Duration::hours(VIAGRA_DURATION_HOURS).num_seconds();
    let effect_ends_discord = format!("<t:{}:R>", effect_ends_unix);

    // Calculate next viagra availability
    let next_viagra_unix = chrono::Utc::now().timestamp() + Duration::hours(VIAGRA_COOLDOWN_HOURS).num_seconds();
    let next_viagra_discord = format!("<t:{}:R>", next_viagra_unix);

    let builder = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new().add_embed(
            CreateEmbed::new()
                .title("💊 VIAGRA ACTIVATED! 🔥")
                .description(format!(
                    "You've taken the magical blue pill! 💎\n\n**Enhancement Details:**\n• +20% growth boost for all /grow commands\n• Effect duration: 6 hours\n• Effect ends: {}\n\n**Next viagra available:** {}\n\nYour dick is now supercharged! Get growing! 🚀",
                    effect_ends_discord, next_viagra_discord
                ))
                .color(0x3498DB) // Blue like viagra
                .footer(CreateEmbedFooter::new(
                    "Warning: Side effects may include uncontrollable confidence and swagger.",
                ))
        )
    );
    return command.create_response(&ctx.http, builder).await;
}

// Helper function to check if viagra is active for a user
pub async fn is_viagra_active(bot: &Bot, user_id: &str, guild_id: &str) -> bool {
    match sqlx::query!(
        "SELECT viagra_active_until FROM dicks WHERE user_id = ? AND guild_id = ?",
        user_id,
        guild_id
    )
    .fetch_optional(&bot.database)
    .await
    {
        Ok(Some(record)) => {
            if let Some(active_until_str) = record.viagra_active_until {
                if let Ok(active_until) = NaiveDateTime::parse_from_str(&active_until_str, "%Y-%m-%d %H:%M:%S") {
                    let now = chrono::Utc::now().naive_utc();
                    return now < active_until;
                }
            }
            false
        }
        _ => false,
    }
}

// Helper function to get viagra status for stats display
pub async fn get_viagra_status(bot: &Bot, user_id: &str, guild_id: &str) -> (bool, Option<String>, Option<String>) {
    match sqlx::query!(
        "SELECT viagra_active_until, viagra_last_used FROM dicks WHERE user_id = ? AND guild_id = ?",
        user_id,
        guild_id
    )
    .fetch_optional(&bot.database)
    .await
    {
        Ok(Some(record)) => {
            let now = chrono::Utc::now().naive_utc();
            
            // Check if currently active
            let is_active = if let Some(active_until_str) = &record.viagra_active_until {
                if let Ok(active_until) = NaiveDateTime::parse_from_str(active_until_str, "%Y-%m-%d %H:%M:%S") {
                    now < active_until
                } else {
                    false
                }
            } else {
                false
            };

            // Calculate next availability
            let next_available = if let Some(last_used_str) = &record.viagra_last_used {
                if let Ok(last_used) = NaiveDateTime::parse_from_str(last_used_str, "%Y-%m-%d %H:%M:%S") {
                    let time_since_last = now - last_used;
                    let cooldown_remaining = Duration::hours(VIAGRA_COOLDOWN_HOURS) - time_since_last;
                    
                    if cooldown_remaining > Duration::zero() {
                        let unix_timestamp = chrono::Utc::now().timestamp() + cooldown_remaining.num_seconds();
                        Some(format!("<t:{}:R>", unix_timestamp))
                    } else {
                        Some("Now".to_string())
                    }
                } else {
                    None
                }
            } else {
                Some("Now".to_string())
            };

            // Calculate when current effect ends (if active)
            let effect_ends = if is_active {
                if let Some(active_until_str) = &record.viagra_active_until {
                    if let Ok(active_until) = NaiveDateTime::parse_from_str(active_until_str, "%Y-%m-%d %H:%M:%S") {
                        let time_left = active_until - now;
                        let unix_timestamp = chrono::Utc::now().timestamp() + time_left.num_seconds();
                        Some(format!("<t:{}:R>", unix_timestamp))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            (is_active, effect_ends, next_available)
        }
        _ => (false, None, Some("Now".to_string())),
    }
} 