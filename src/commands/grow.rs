use crate::Bot;
use crate::commands::daily::{
    consume_cooldown_skip, consume_daily_growth_boost_percent, consume_lucky_roll,
    update_growth_streak,
};
use crate::commands::events::{add_to_community_pot, get_active_global_event};
use crate::commands::prestige::{add_prestige_bonus, growth_bonus_for_profile};
use crate::commands::seasons::{active_season, credit_earned_growth};
use crate::commands::viagra::is_viagra_active;
use crate::time::check_cooldown_with_minutes;
use crate::utils::{ordinal_suffix, pluralize};
use chrono::NaiveDateTime;
use log::{error, info};
use rand::RngExt;
use serenity::all::{
    CommandInteraction, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use serenity::prelude::*;

const BASE_GROWTH_MIN_CM: i64 = 1;
const BASE_GROWTH_MAX_CM: i64 = 10;

async fn apply_streak_reward(
    bot: &Bot,
    user_id: &str,
    guild_id: &str,
    streak: i64,
) -> Option<(i64, i64)> {
    let reward = (1.0 + (streak as f64).ln() * 0.8).round() as i64;
    let reward = reward.clamp(1, 5);
    let now_str = chrono::Utc::now()
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    let season = active_season(&bot.database).await.ok()?;
    let mut tx = bot.database.begin().await.ok()?;
    if let Err(why) = sqlx::query(
        "UPDATE dicks
         SET length = length + ?, streak_last_claimed = ?
         WHERE user_id = ? AND guild_id = ?",
    )
    .bind(reward)
    .bind(now_str)
    .bind(user_id)
    .bind(guild_id)
    .execute(&mut *tx)
    .await
    {
        error!("Error applying automatic streak reward: {:?}", why);
        return None;
    }
    if let Err(why) = credit_earned_growth(&mut tx, season.id, user_id, guild_id, reward).await {
        error!("Error crediting streak progression: {:?}", why);
        return None;
    }
    let new_length =
        sqlx::query_scalar::<_, i64>("SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?")
            .bind(user_id)
            .bind(guild_id)
            .fetch_one(&mut *tx)
            .await
            .ok()?;
    sqlx::query(
        "INSERT INTO length_history (user_id, guild_id, length, growth_amount, growth_type)
         VALUES (?, ?, ?, ?, 'streak')",
    )
    .bind(user_id)
    .bind(guild_id)
    .bind(new_length)
    .bind(reward)
    .execute(&mut *tx)
    .await
    .ok()?;
    tx.commit().await.ok()?;

    Some((reward, new_length))
}

pub async fn handle_grow_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> Result<(), serenity::Error> {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    let user_id = command.user.id.to_string();
    let guild_id = command.guild_id.unwrap().to_string();
    let mut cooldown_skip_used = false;

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

            let active_event = get_active_global_event(bot).await;
            let cooldown_minutes = active_event
                .as_ref()
                .and_then(|event| event.grow_cooldown_minutes())
                .unwrap_or(60);
            let time_left = check_cooldown_with_minutes(&last_grow, cooldown_minutes);
            // Format time_left into discord timestamp
            let unix_timestamp = chrono::Utc::now().timestamp() + time_left.num_seconds();
            let discord_timestamp = format!("<t:{}:R>", unix_timestamp);

            if !time_left.is_zero() {
                if consume_cooldown_skip(bot, &user_id, &guild_id).await {
                    cooldown_skip_used = true;
                } else {
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

    let active_event = get_active_global_event(bot).await;

    let (growth_min, growth_max) = active_event
        .as_ref()
        .and_then(|event| event.growth_range())
        .unwrap_or((BASE_GROWTH_MIN_CM, BASE_GROWTH_MAX_CM));
    let first_roll = rand::rng().random_range(growth_min..=growth_max);
    let lucky_roll_active = consume_lucky_roll(bot, &user_id, &guild_id).await;
    let event_double_roll = active_event
        .as_ref()
        .is_some_and(|event| event.rolls_growth_twice());
    let base_growth = if lucky_roll_active || event_double_roll {
        let second_roll = rand::rng().random_range(growth_min..=growth_max);
        first_roll.max(second_roll)
    } else {
        first_roll
    };

    // Check if viagra is active for this user
    let viagra_active = is_viagra_active(bot, &user_id, &guild_id).await;
    let daily_boost_percent = consume_daily_growth_boost_percent(bot, &user_id, &guild_id).await;
    let prestige_bonus_percent = growth_bonus_for_profile(bot, &user_id, &guild_id).await;

    let mut multiplier = 1.0;
    let mut boost_notes = Vec::new();

    if cooldown_skip_used {
        boost_notes.push("⏩ Cooldown skip".to_string());
    }

    if lucky_roll_active {
        boost_notes.push("🍀 Lucky roll".to_string());
    }

    if event_double_roll && let Some(event) = active_event.as_ref() {
        boost_notes.push(format!("🌍 {}", event.name));
    }

    if viagra_active {
        multiplier += 0.20;
        boost_notes.push("💊 Viagra +20%".to_string());
    }

    if let Some(percent) = daily_boost_percent {
        multiplier += percent as f64 / 100.0;
        boost_notes.push(format!("⚡ Daily +{}%", percent));
    }

    if prestige_bonus_percent > 0 {
        multiplier = add_prestige_bonus(multiplier, prestige_bonus_percent);
        boost_notes.push(format!("🏆 Prestige +{}%", prestige_bonus_percent));
    }

    if let Some(event) = active_event.as_ref()
        && let Some(event_multiplier) = event.growth_multiplier()
    {
        multiplier += event_multiplier - 1.0;
        boost_notes.push(format!("🌍 {} +{}%", event.name, event.bonus_value));
    }

    let mut growth = if multiplier > 1.0 {
        let boosted = (base_growth as f64 * multiplier).round() as i64;
        info!(
            "User {} growth boosted from {} to {} with multiplier {:.2}",
            user_id, base_growth, boosted, multiplier
        );
        boosted
    } else {
        base_growth
    };

    if let Some(event) = active_event.as_ref()
        && let Some(jackpot) = event.jackpot_extra_cm()
    {
        growth += jackpot;
        boost_notes.push(format!("🌍 {} +{} cm", event.name, jackpot));
    }

    let season = match active_season(&bot.database).await {
        Ok(season) => season,
        Err(why) => {
            error!("Error selecting active season for growth: {:?}", why);
            return command
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content(
                                "Growth is temporarily unavailable while the season rolls over.",
                            )
                            .ephemeral(true),
                    ),
                )
                .await;
        }
    };
    let mut tx = match bot.database.begin().await {
        Ok(tx) => tx,
        Err(why) => {
            error!("Error starting growth transaction: {:?}", why);
            return Ok(());
        }
    };
    let growth_result = async {
        sqlx::query(
            "UPDATE dicks
             SET length = length + ?, last_grow = datetime('now'), growth_count = growth_count + 1
             WHERE user_id = ? AND guild_id = ?",
        )
        .bind(growth)
        .bind(&user_id)
        .bind(&guild_id)
        .execute(&mut *tx)
        .await?;
        credit_earned_growth(&mut tx, season.id, &user_id, &guild_id, growth).await?;
        let new_length = sqlx::query_scalar::<_, i64>(
            "SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?",
        )
        .bind(&user_id)
        .bind(&guild_id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO length_history (user_id, guild_id, length, growth_amount, growth_type)
             VALUES (?, ?, ?, ?, 'grow')",
        )
        .bind(&user_id)
        .bind(&guild_id)
        .bind(new_length)
        .bind(growth)
        .execute(&mut *tx)
        .await?;
        Ok::<i64, sqlx::Error>(new_length)
    }
    .await;
    let new_length = match growth_result {
        Ok(new_length) => {
            if let Err(why) = tx.commit().await {
                error!("Error committing growth transaction: {:?}", why);
                return Ok(());
            }
            new_length
        }
        Err(why) => {
            error!("Error applying atomic growth: {:?}", why);
            let _ = tx.rollback().await;
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

    if let Some(event) = active_event.as_ref()
        && let Some(pot_amount) = event.community_pot_cm_per_grow()
    {
        add_to_community_pot(bot, event.id, pot_amount).await;
        boost_notes.push(format!("🌍 {} pot +{} cm", event.name, pot_amount));
    }

    let mut new_length = new_length;
    if let Some(streak_update) = update_growth_streak(bot, &user_id, &guild_id).await {
        if streak_update.used_streak_saver {
            boost_notes.push("🛟 Streak saver".to_string());
        }

        if let Some((streak_reward, streak_length)) =
            apply_streak_reward(bot, &user_id, &guild_id, streak_update.streak).await
        {
            boost_notes.push(format!(
                "🔥 {} streak +{} cm",
                pluralize(streak_update.streak, "day", "days"),
                streak_reward
            ));

            new_length = streak_length;
        }
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
    let cooldown_minutes = active_event
        .as_ref()
        .and_then(|event| event.grow_cooldown_minutes())
        .unwrap_or(60);
    let next_grow_unix = (last_grow + chrono::Duration::minutes(cooldown_minutes)).timestamp();
    let next_grow_discord = format!("<t:{}:R>", next_grow_unix);

    // Add boost indicators
    let boost_text = if boost_notes.is_empty() {
        String::new()
    } else {
        format!(" **({})**", boost_notes.join(", "))
    };

    // Create response with funny messages based on growth
    let (title, description, color) = if growth > 10 {
        (
            "🚀 INCREDIBLE GROWTH!",
            format!(
                "Holy moly! Your dick just grew by **{} cm**{} and is now a whopping **{} cm** long!\nYou are currently **{}{}** in the server.\n\nNext attempt: {}\n\nCareful, you might trip over it soon!",
                growth,
                boost_text,
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
                boost_text,
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
                boost_text,
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
                boost_text,
                new_length,
                position,
                ordinal_suffix(position),
                next_grow_discord
            ),
            0x99FF99, // Lighter green
        )
    };

    const FOOTERS: &[&str] = &[
        "Remember: it's not about the size, it's about... actually, it is about the size.",
        "Your ruler is judging you.",
        "Even tiny steps are still forward progress.",
        "The pen is mighty, but the dick is mightier.",
        "Grow slow, go low, stay low.",
        "Nature hates a vacuum, but loves a full one.",
        "A journey of a thousand miles begins with a single /grow.",
        "With great length comes great responsibility.",
        "You're doing great!",
        "If you can measure it, you can improve it.",
        "The early bird gets the worm. The big dick gets respect.",
        "Rome wasn't built in a day, and neither was your dick.",
        "Stay hungry, stay humble, stay growing.",
        "What goes up must come down... eventually.",
        "In a time of uncertainty, /grow.",
        "Trust the process. Trust the gains.",
        "Big things come to those who wait... and /grow daily.",
        "The only bad measurement is no measurement.",
        "Keep it between the sheets and in the database.",
        "Your dick is a garden. Water it daily.",
    ];
    let footer = FOOTERS[rand::rng().random_range(0..FOOTERS.len())];

    let builder = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new().add_embed(
            CreateEmbed::new()
                .title(title)
                .description(description)
                .color(color)
                .footer(CreateEmbedFooter::new(footer)),
        ),
    );
    return command.create_response(&ctx.http, builder).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streak_reward_curve() {
        let cases = [(1, 1), (3, 2), (7, 3), (14, 3), (30, 4), (60, 4), (100, 5)];
        for (streak, expected) in cases {
            let reward = (1.0 + (streak as f64).ln() * 0.8).round() as i64;
            let reward = reward.clamp(1, 5);
            assert_eq!(
                reward, expected,
                "streak {} expected {} got {}",
                streak, expected, reward
            );
        }
    }

    #[test]
    fn test_growth_distribution() {
        const ITERATIONS: usize = 10000;

        let mut values = Vec::new();
        for _ in 0..ITERATIONS {
            let growth = rand::rng().random_range(BASE_GROWTH_MIN_CM..=BASE_GROWTH_MAX_CM);
            values.push(growth);
        }

        let avg = values.iter().sum::<i64>() as f64 / ITERATIONS as f64;
        let min = values.iter().min().unwrap();
        let max = values.iter().max().unwrap();

        assert_eq!(*min, BASE_GROWTH_MIN_CM);
        assert_eq!(*max, BASE_GROWTH_MAX_CM);
        assert!((avg - 5.5).abs() < 0.3, "Average should be around 5.5");
    }
}
