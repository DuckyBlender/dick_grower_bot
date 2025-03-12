use chrono::{NaiveDateTime, Utc};
use log::error;
use rand::Rng;
use serenity::all::{
    CommandInteraction, CreateEmbed,
    CreateEmbedFooter, CreateInteractionResponse, CreateInteractionResponseMessage,
};
use serenity::prelude::*;
use crate::time::check_30_minutes;
use crate::Bot;

pub async fn handle_grow_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> CreateInteractionResponse {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    let user_id = command.user.id.to_string();
    let guild_id = command.guild_id.unwrap().to_string();

    // Check if the user has grown today
    let _last_grow = match sqlx::query!(
        "SELECT last_grow FROM dicks WHERE user_id = ? AND guild_id = ?",
        user_id,
        guild_id
    )
    .fetch_optional(&bot.database)
    .await
    {
        Ok(Some(record)) => {
            let last_grow = NaiveDateTime::parse_from_str(&record.last_grow, "%Y-%m-%d %H:%M:%S")
                .unwrap_or_default();

            let time_left = check_30_minutes(&last_grow);
            if time_left.0 {
                return CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .add_embed(
                            CreateEmbed::new()
                                .title("🕒 Hold up, speedy!")
                                .description(format!(
                                    "You've already grown your dick today! Try again in **{}h {}m**\n\nExcessive stimulation might cause injuries, you know?",
                                    time_left.1.num_hours(),
                                    time_left.1.num_minutes() % 60
                                ))
                                .color(0xFF5733)
                                .footer(CreateEmbedFooter::new(
                                    "Patience is a virtue... especially for your little buddy.",
                                ))
                        )
                );
            }

            last_grow
        }
        Ok(None) => {
            // New user, create a record
            match sqlx::query!(
                "INSERT INTO dicks (user_id, guild_id, length, last_grow, dick_of_day_count, 
                                   pvp_wins, pvp_losses, pvp_max_streak, pvp_current_streak,
                                   cm_won, cm_lost)
                 VALUES (?, ?, 0, datetime('now'), 0, 0, 0, 0, 0, 0, 0)",
                user_id,
                guild_id
            )
            .execute(&bot.database)
            .await
            {
                Ok(_) => (),
                Err(why) => {
                    error!("Error creating user: {:?}", why);
                }
            };

            Utc::now().naive_utc()
        }
        Err(why) => {
            error!("Database error: {:?}", why);
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Database Error")
                        .description(
                            "Something went wrong with your dick growth. Maybe the universe is telling you something?",
                        )
                        .color(0xFF0000),
                ),
            );
        }
    };

    // Generate growth amount (-5 to 10 cm)
    let growth = rand::rng().random_range(-5..=10);

    // Update the database
    match sqlx::query!(
        "UPDATE dicks SET length = length + ?, last_grow = datetime('now')
         WHERE user_id = ? AND guild_id = ?",
        growth,
        user_id,
        guild_id
    )
    .execute(&bot.database)
    .await
    {
        Ok(_) => (),
        Err(why) => {
            error!("Error updating length: {:?}", why);
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Growth Error")
                        .description("Your dick refused to cooperate with the database.")
                        .color(0xFF0000),
                ),
            );
        }
    };

    // Get new length
    let new_length = match sqlx::query!(
        "SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?",
        user_id,
        guild_id
    )
    .fetch_one(&bot.database)
    .await
    {
        Ok(record) => record.length,
        Err(why) => {
            error!("Error fetching length: {:?}", why);
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Length Measurement Error")
                        .description(
                            "We couldn't measure your updated length. The measuring tape broke.",
                        )
                        .color(0xFF0000),
                ),
            );
        }
    };

    // Create response with funny messages based on growth
    let (title, description, color) = if growth > 7 {
        (
            "🚀 INCREDIBLE GROWTH!",
            format!(
                "Holy moly! Your dick grew by **{} cm**!\nYour new length: **{} cm**\n\nThat's some supernatural growth! Are you using some kind of black magic?",
                growth, new_length
            ),
            0x00FF00, // Bright green
        )
    } else if growth > 3 {
        (
            "🔥 Impressive Growth!",
            format!(
                "Nice! Your dick grew by **{} cm**!\nYour new length: **{} cm**\n\nKeep it up, that's some serious growth!",
                growth, new_length
            ),
            0x33FF33, // Green
        )
    } else if growth > 0 {
        (
            "🌱 Growth Achieved",
            format!(
                "Your dick grew by **{} cm**.\nYour new length: **{} cm**\n\nSlow and steady wins the race, right?",
                growth, new_length
            ),
            0x66FF66, // Light green
        )
    } else if growth == 0 {
        (
            "😐 No Change",
            format!(
                "Your dick didn't grow at all today.\nYour length: **{} cm**\n\nMaybe try some positive affirmations?",
                new_length
            ),
            0xFFFF33, // Yellow
        )
    } else if growth >= -3 {
        (
            "📉 Minor Shrinkage",
            format!(
                "Uh oh! Your dick shrank by **{} cm**.\nYour new length: **{} cm**\n\nDid you take a cold shower?",
                -growth, new_length
            ),
            0xFF9933, // Orange
        )
    } else {
        (
            "💀 CATASTROPHIC SHRINKAGE!",
            format!(
                "DISASTER! Your dick shrank by **{} cm**!\nYour new length: **{} cm**\n\nWhatever you're doing, STOP IMMEDIATELY!",
                -growth, new_length
            ),
            0xFF3333, // Red
        )
    };

    CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new().add_embed(
            CreateEmbed::new()
                .title(title)
                .description(description)
                .color(color)
                .footer(CreateEmbedFooter::new(
                    "Remember: it's not about the size, it's about... actually, it is about the size.",
                )),
        ),
    )
}

