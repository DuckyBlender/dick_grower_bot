use crate::Bot;
use crate::commands::viagra::is_viagra_active;
use crate::time::check_cooldown_minutes;
use crate::utils::ordinal_suffix;
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
    let _user_stats = match sqlx::query!(
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

            let time_left = check_cooldown_minutes(&last_grow);
            // Format time_left into discord timestamp
            let unix_timestamp = chrono::Utc::now().timestamp() + time_left.num_seconds();
            let discord_timestamp = format!("<t:{}:R>", unix_timestamp);

            if !time_left.is_zero() {
                let builder = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .add_embed(
                            CreateEmbed::new()
                                .title("🕒 Hold up, speedy!")
                                .description(format!(
                                    "You've already played with your dick today! Try again in {discord_timestamp}\n\nExcessive stimulation might cause injuries, you know?",
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

    // Generate growth amount (always positive now, no more negative growth)
    let base_growth = rand::rng().random_range(1..=10);
    
    // Check if viagra is active for this user
    let viagra_active = is_viagra_active(bot, &user_id, &guild_id).await;
    
    // Apply viagra boost if active (20% increase)
    let growth = if viagra_active {
        let boosted = (base_growth as f64 * 1.2).round() as i64;
        info!(
            "User {} has viagra active, boosting growth from {} to {}",
            user_id, base_growth, boosted
        );
        boosted
    } else {
        base_growth
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

    // Log the growth in length_history
    if let Err(why) = sqlx::query!(
        "INSERT INTO length_history (user_id, guild_id, length, growth_amount, growth_type)
         VALUES (?, ?, ?, ?, 'grow')",
        user_id,
        guild_id,
        new_length,
        growth
    )
    .execute(&bot.database)
    .await
    {
        error!("Error logging growth history: {:?}", why);
    }

    // Get user position in server top
    let position = match sqlx::query!(
        "SELECT COUNT(*) as pos FROM dicks WHERE guild_id = ? AND length > ?",
        guild_id,
        new_length
    )
    .fetch_one(&bot.database)
    .await
    {
        Ok(record) => record.pos + 1,
        Err(_) => {
            error!("Error fetching position");
            0
        }
    };
    let position = position as usize; // Safe to cast to usize

    // Calculate next grow time (cooldown)
    let last_grow = chrono::Utc::now();
    let next_grow_unix = (last_grow + chrono::Duration::minutes(60)).timestamp();
    let next_grow_discord = format!("<t:{}:R>", next_grow_unix);

    // Add viagra boost indicator
    let viagra_text = if viagra_active {
        " 💊 **(VIAGRA BOOST APPLIED!)**"
    } else {
        ""
    };

    // Create response with funny messages based on growth
    let (title, description, color) = if growth > 10 {
        (
            "🚀 INCREDIBLE GROWTH!",
            format!(
                "Holy moly! Your dick just grew by **{} cm**{} and is now a whopping **{} cm** long!\nYou are currently **{}{}** in the server.\n\nNext attempt: {}\n\nCareful, you might trip over it soon!",
                growth,
                viagra_text,
                new_length,
                position,
                ordinal_suffix(position),
                next_grow_discord
            ),
            0x00FF00, // Bright green
        )
    } else if growth > 7 {
        (
            "🔥 Impressive Growth!",
            format!(
                "Nice! Your dick grew by **{} cm**{}! Your new length is **{} cm**.\nYou are currently **{}{}** in the server's leaderboard.\n\nNext attempt: {}\n\nKeep up the good work, size king!",
                growth,
                viagra_text,
                new_length,
                position,
                ordinal_suffix(position),
                next_grow_discord
            ),
            0x33FF33, // Green
        )
    } else if growth > 3 {
        (
            "🌱 Solid Growth",
            format!(
                "A good **{} cm** added{}! You're now at **{} cm**.\nYou are currently **{}{}** in the server.\n\nNext attempt: {}\n\nEvery centimeter counts!",
                growth,
                viagra_text,
                new_length,
                position,
                ordinal_suffix(position),
                next_grow_discord
            ),
            0x66FF66, // Light green
        )
    } else {
        (
            "📏 Modest Growth",
            format!(
                "A small but positive **{} cm** added{}. You're now at **{} cm**.\nYou are currently **{}{}** in the server.\n\nNext attempt: {}\n\nSmall steps lead to big achievements!",
                growth,
                viagra_text,
                new_length,
                position,
                ordinal_suffix(position),
                next_grow_discord
            ),
            0x99FF99, // Lighter green
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
