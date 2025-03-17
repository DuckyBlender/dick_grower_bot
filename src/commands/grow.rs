use crate::Bot;
use crate::time::check_30_minutes;
use chrono::NaiveDateTime;
use log::{error, info};
use rand::Rng;
use serenity::all::{
    CommandInteraction, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use serenity::prelude::*;

pub async fn handle_grow_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> Result<(), serenity::Error> {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    let user_id = command.user.id.to_string();
    let guild_id = command.guild_id.unwrap().to_string();

    // Check if the user has grown today and get their stats
    let user_stats = match sqlx::query!(
        "SELECT last_grow, length, growth_count FROM dicks WHERE user_id = ? AND guild_id = ?",
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
            if !time_left.is_zero() {
                let builder = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .add_embed(
                            CreateEmbed::new()
                                .title("🕒 Hold up, speedy!")
                                .description(format!(
                                    "You've already played with your dick today! Try again in **{}h {}m**\n\nExcessive stimulation might cause injuries, you know?",
                                    time_left.num_hours(),
                                    time_left.num_minutes() % 60
                                ))
                                .color(0xFF5733)
                                .footer(CreateEmbedFooter::new(
                                    "Patience is key... especially for your little buddy.",
                                ))
                        )
                        .ephemeral(true)
                );
                return command.create_response(&ctx.http, builder).await;   
            }

            // Return user stats
            (record.growth_count, record.length)
        }
        Ok(None) => {
            // New user, create a record
            info!(
                "New user detected, adding user {} ({}) in guild id {} to database",
                command.user.name, user_id, guild_id
            );
            match sqlx::query!(
                "INSERT INTO dicks (user_id, guild_id, length, last_grow, growth_count, dick_of_day_count, 
                                   pvp_wins, pvp_losses, pvp_max_streak, pvp_current_streak,
                                   cm_won, cm_lost)
                 VALUES (?, ?, 0, datetime('now'), 0, 0, 0, 0, 0, 0, 0, 0)",
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

            // New user with 0 growth count
            (0, 0)
        }
        Err(why) => {
            error!("Database error: {:?}", why);
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Database Error")
                        .description(
                            "Something went wrong with your dick growth. Maybe the universe is telling you something?",
                        )
                        .color(0xFF0000),
                ),
            );
            return command.create_response(&ctx.http, builder).await;
        }
    };

    // Check if user is in grace period (first 7 growths)
    let is_grace_period = user_stats.0 < 7;

    // Generate growth amount based on whether user is in grace period
    let growth = if is_grace_period {
        // During grace period: 1 to 10 cm (only positive)
        info!(
            "User {} is in grace period (growth #{}), generating positive growth only",
            user_id,
            user_stats.0 + 1
        );
        rand::rng().random_range(1..=10)
    } else {
        // After grace period: -5 to 10 cm with more positive chance
        let sign_ratio: f32 = 0.80; // 80% chance of positive growth
        let sign_ratio_percent = (sign_ratio * 100.0).round() as u32;

        // Generate a random value
        let is_positive = rand::rng().random_ratio(sign_ratio_percent, 100);

        if is_positive {
            rand::rng().random_range(1..=10) // Positive growth
        } else {
            rand::rng().random_range(-5..=-1) // Negative growth (never 0)
        }
    };

    // Update the database - increment growth count too
    match sqlx::query!(
        "UPDATE dicks SET length = length + ?, last_grow = datetime('now'), growth_count = growth_count + 1
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
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Growth Error")
                        .description("Your dick refused to cooperate with the database.")
                        .color(0xFF0000),
                ),
            );
            return command.create_response(&ctx.http, builder).await;
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
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Length Measurement Error")
                        .description(
                            "We couldn't measure your updated length. The measuring tape broke.",
                        )
                        .color(0xFF0000),
                ),
            );
            return command.create_response(&ctx.http, builder).await;
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
    } else if growth >= -3 {
        (
            "📉 Minor Shrinkage",
            format!(
                "Uh oh! Your dick shrank by **{} cm**.\nYour new length: **{} cm**\n\nDid you take a cold shower?",
                -growth, new_length
            ),
            0xFF9933, // Orange
        )
        // impossible to get 0 growth
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

    let builder = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new().add_embed(
            CreateEmbed::new()
                .title(title)
                .description(description)
                .color(color)
                .footer(CreateEmbedFooter::new(
                    "Remember: it's not about the size, it's about... actually, it is about the size.",
                )),
        ),
    );
    return command.create_response(&ctx.http, builder).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_growth_distribution() {
        // Generate many growths to get meaningful statistics
        const ITERATIONS: usize = 10000;

        let mut positive_values = Vec::new();
        let mut negative_values = Vec::new();

        for _ in 0..ITERATIONS {
            // This is the same code from your grow function
            let sign_ratio: f32 = 0.80; // 80% chance of positive growth
            let sign_ratio_percent = (sign_ratio * 100.0).round() as u32;

            let is_positive = rand::rng().random_ratio(sign_ratio_percent, 100);

            let growth = if is_positive {
                rand::rng().random_range(1..=10) // Positive growth
            } else {
                rand::rng().random_range(-5..=-1) // Negative growth
            };

            if growth > 0 {
                positive_values.push(growth);
            } else {
                negative_values.push(growth);
            }
        }

        // Calculate statistics
        let positive_count = positive_values.len();
        let negative_count = negative_values.len();

        let positive_avg = positive_values.iter().sum::<i64>() as f64 / positive_count as f64;
        let negative_avg =
            negative_values.iter().map(|&x| -x).sum::<i64>() as f64 / negative_count as f64;
        let positive_ratio = positive_count as f64 / ITERATIONS as f64;

        println!("Total samples: {}", ITERATIONS);
        println!(
            "Positive count: {}, Negative count: {}",
            positive_count, negative_count
        );
        println!(
            "Positive average: {:.2}, Negative average: {:.2}",
            positive_avg, negative_avg
        );
        println!("Positive ratio: {:.2} (target: 0.80)", positive_ratio);

        // Verify distribution is approximately correct
        assert!(
            (positive_ratio - 0.8).abs() < 0.03,
            "Positive ratio should be around 0.8"
        );

        // Expected average for values 1-10 is 5.5
        assert!(
            (positive_avg - 5.5).abs() < 0.3,
            "Positive average should be around 5.5"
        );

        // Expected average for values -5 to -1 is 3.0
        assert!(
            (negative_avg - 3.0).abs() < 0.3,
            "Negative average should be around 3.0"
        );
    }
}
