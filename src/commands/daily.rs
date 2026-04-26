use crate::Bot;
use crate::time::check_utc_day_reset;
use chrono::{Duration, NaiveDateTime};
use log::{error, info};
use rand::RngExt;
use serenity::all::{
    CommandInteraction, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use serenity::prelude::*;
use sqlx::Row;

const NEXT_GROWTH_BOOST_PERCENT: i64 = 50;
const DAILY_BONUS_CM_MIN: i64 = 5;
const DAILY_BONUS_CM_MAX: i64 = 15;

const DAILY_BONUS_REWARD_WEIGHT: u32 = 1;
const NEXT_GROWTH_BOOST_REWARD_WEIGHT: u32 = 1;
const COOLDOWN_SKIP_REWARD_WEIGHT: u32 = 1;
const STREAK_SAVER_REWARD_WEIGHT: u32 = 1;
const LUCKY_ROLL_REWARD_WEIGHT: u32 = 1;

pub async fn handle_daily_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> Result<(), serenity::Error> {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    let user_id = command.user.id.to_string();
    let guild_id = command.guild_id.unwrap().to_string();

    ensure_user(bot, &user_id, &guild_id, &command.user.name).await;

    let last_claimed = match sqlx::query(
        "SELECT daily_last_claimed FROM dicks WHERE user_id = ? AND guild_id = ?",
    )
    .bind(&user_id)
    .bind(&guild_id)
    .fetch_optional(&bot.database)
    .await
    {
        Ok(Some(row)) => row
            .try_get::<Option<String>, _>("daily_last_claimed")
            .ok()
            .flatten(),
        Ok(None) => None,
        Err(why) => {
            error!("Database error checking daily reward: {:?}", why);
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Daily Error")
                        .description("Failed to check your daily reward. The prize box jammed.")
                        .color(0xFF0000),
                ),
            );
            return command.create_response(&ctx.http, builder).await;
        }
    };

    if let Some(last_claimed_str) = last_claimed
        && let Ok(last_claimed_time) =
            NaiveDateTime::parse_from_str(&last_claimed_str, "%Y-%m-%d %H:%M:%S")
    {
        let time_left = check_utc_day_reset(&last_claimed_time);
        if !time_left.is_zero() {
            let unix_timestamp = chrono::Utc::now().timestamp() + time_left.num_seconds();
            let discord_timestamp = format!("<t:{}:R>", unix_timestamp);

            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("🕒 Daily Already Claimed")
                            .description(format!(
                                "You've already grabbed today's daily reward.\n\nCome back {discord_timestamp} for another suspicious package."
                            ))
                            .color(0xFF5733),
                    )
                    .ephemeral(true),
            );
            return command.create_response(&ctx.http, builder).await;
        }
    }

    let now = chrono::Utc::now().naive_utc();
    let now_str = now.format("%Y-%m-%d %H:%M:%S").to_string();
    let next_growth_boost_cutoff = DAILY_BONUS_REWARD_WEIGHT + NEXT_GROWTH_BOOST_REWARD_WEIGHT;
    let cooldown_skip_cutoff = next_growth_boost_cutoff + COOLDOWN_SKIP_REWARD_WEIGHT;
    let streak_saver_cutoff = cooldown_skip_cutoff + STREAK_SAVER_REWARD_WEIGHT;
    let total_daily_reward_weight = streak_saver_cutoff + LUCKY_ROLL_REWARD_WEIGHT;
    let reward = rand::rng().random_range(0..total_daily_reward_weight);
    let (title, description, color) = if reward < DAILY_BONUS_REWARD_WEIGHT {
        let bonus = rand::rng().random_range(DAILY_BONUS_CM_MIN..=DAILY_BONUS_CM_MAX);
        if let Err(why) = sqlx::query(
            "UPDATE dicks
                 SET daily_last_claimed = ?, length = length + ?
                 WHERE user_id = ? AND guild_id = ?",
        )
        .bind(&now_str)
        .bind(bonus)
        .bind(&user_id)
        .bind(&guild_id)
        .execute(&bot.database)
        .await
        {
            error!("Error applying daily cm bonus: {:?}", why);
        }

        let new_length = fetch_length(bot, &user_id, &guild_id)
            .await
            .unwrap_or_default();
        log_length_history(bot, &user_id, &guild_id, new_length, bonus, "daily_bonus").await;

        (
            "🎁 Daily Bonus Claimed!",
            format!(
                "You found **+{} cm** in today's package!\n\nYour new length is **{} cm**.",
                bonus, new_length
            ),
            0x2ECC71,
        )
    } else if reward < next_growth_boost_cutoff {
        if let Err(why) = sqlx::query(
            "UPDATE dicks
             SET daily_last_claimed = ?, daily_growth_boost_percent = ?
             WHERE user_id = ? AND guild_id = ?",
        )
        .bind(&now_str)
        .bind(NEXT_GROWTH_BOOST_PERCENT)
        .bind(&user_id)
        .bind(&guild_id)
        .execute(&bot.database)
        .await
        {
            error!("Error applying daily growth boost: {:?}", why);
        }

        (
            "⚡ Daily Boost Claimed!",
            format!(
                "Your next /grow gets **+{}% growth**.",
                NEXT_GROWTH_BOOST_PERCENT
            ),
            0x3498DB,
        )
    } else if reward < cooldown_skip_cutoff {
        if let Err(why) =
            increment_daily_counter(bot, &user_id, &guild_id, "daily_cooldown_skips", &now_str)
                .await
        {
            error!("Error applying cooldown skip token: {:?}", why);
        }

        (
            "⏩ Cooldown Skip Claimed!",
            "Your next on-cooldown /grow will ignore the cooldown.".to_string(),
            0x1ABC9C,
        )
    } else if reward < streak_saver_cutoff {
        if let Err(why) =
            increment_daily_counter(bot, &user_id, &guild_id, "daily_streak_savers", &now_str).await
        {
            error!("Error applying streak saver: {:?}", why);
        }

        (
            "📅 Streak Saver Claimed!",
            "Your daily growth streak can survive one missed UTC day.".to_string(),
            0xE67E22,
        )
    } else {
        if let Err(why) =
            increment_daily_counter(bot, &user_id, &guild_id, "daily_lucky_rolls", &now_str).await
        {
            error!("Error applying lucky roll: {:?}", why);
        }

        (
            "🍀 Lucky Roll Claimed!",
            "Your next /grow rolls twice and keeps the better result.".to_string(),
            0x2ECC71,
        )
    };

    let builder = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new().add_embed(
            CreateEmbed::new()
                .title(title)
                .description(description)
                .color(color)
                .footer(CreateEmbedFooter::new(
                    "Daily rewards reset at midnight UTC.",
                )),
        ),
    );
    command.create_response(&ctx.http, builder).await
}

pub async fn consume_daily_growth_boost_percent(
    bot: &Bot,
    user_id: &str,
    guild_id: &str,
) -> Option<i64> {
    let row = sqlx::query(
        "SELECT daily_growth_boost_percent FROM dicks WHERE user_id = ? AND guild_id = ?",
    )
    .bind(user_id)
    .bind(guild_id)
    .fetch_optional(&bot.database)
    .await
    .ok()??;

    let boost_percent = row
        .try_get::<i64, _>("daily_growth_boost_percent")
        .unwrap_or_default();

    if boost_percent <= 0 {
        return None;
    }

    if let Err(why) = sqlx::query(
        "UPDATE dicks
         SET daily_growth_boost_percent = 0
         WHERE user_id = ? AND guild_id = ?",
    )
    .bind(user_id)
    .bind(guild_id)
    .execute(&bot.database)
    .await
    {
        error!("Error consuming daily growth boost: {:?}", why);
    }

    Some(boost_percent)
}

pub async fn consume_cooldown_skip(bot: &Bot, user_id: &str, guild_id: &str) -> bool {
    consume_daily_counter(bot, user_id, guild_id, "daily_cooldown_skips").await
}

pub async fn consume_lucky_roll(bot: &Bot, user_id: &str, guild_id: &str) -> bool {
    consume_daily_counter(bot, user_id, guild_id, "daily_lucky_rolls").await
}

pub struct GrowthStreakUpdate {
    pub streak: i64,
    pub used_streak_saver: bool,
}

pub async fn update_growth_streak(
    bot: &Bot,
    user_id: &str,
    guild_id: &str,
) -> Option<GrowthStreakUpdate> {
    let row = match sqlx::query(
        "SELECT daily_streak, last_streak_date, daily_streak_savers
         FROM dicks
         WHERE user_id = ? AND guild_id = ?",
    )
    .bind(user_id)
    .bind(guild_id)
    .fetch_optional(&bot.database)
    .await
    {
        Ok(row) => row,
        Err(why) => {
            error!("Error fetching growth streak: {:?}", why);
            return None;
        }
    };

    let row = row?;

    let today = chrono::Utc::now().date_naive();
    let current_streak = row.try_get::<i64, _>("daily_streak").unwrap_or_default();
    let last_date = row
        .try_get::<Option<String>, _>("last_streak_date")
        .ok()
        .flatten()
        .and_then(|date| chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d").ok());

    if last_date == Some(today) {
        return None;
    }

    let streak_savers = row
        .try_get::<i64, _>("daily_streak_savers")
        .unwrap_or_default();
    let mut used_streak_saver = false;
    let new_streak = if last_date == Some(today - Duration::days(1)) {
        current_streak + 1
    } else if last_date == Some(today - Duration::days(2))
        && current_streak > 0
        && streak_savers > 0
    {
        used_streak_saver = true;
        if let Err(why) =
            decrement_daily_counter(bot, user_id, guild_id, "daily_streak_savers").await
        {
            error!("Error consuming streak saver: {:?}", why);
        }
        current_streak + 1
    } else {
        1
    };
    let today_str = today.format("%Y-%m-%d").to_string();

    if let Err(why) = sqlx::query(
        "UPDATE dicks
         SET daily_streak = ?,
             best_daily_streak = CASE WHEN ? > best_daily_streak THEN ? ELSE best_daily_streak END,
             last_streak_date = ?
         WHERE user_id = ? AND guild_id = ?",
    )
    .bind(new_streak)
    .bind(new_streak)
    .bind(new_streak)
    .bind(today_str)
    .bind(user_id)
    .bind(guild_id)
    .execute(&bot.database)
    .await
    {
        error!("Error updating growth streak: {:?}", why);
        return None;
    }

    Some(GrowthStreakUpdate {
        streak: new_streak,
        used_streak_saver,
    })
}

async fn ensure_user(bot: &Bot, user_id: &str, guild_id: &str, user_name: &str) {
    let exists = sqlx::query("SELECT 1 FROM dicks WHERE user_id = ? AND guild_id = ?")
        .bind(user_id)
        .bind(guild_id)
        .fetch_optional(&bot.database)
        .await;

    if matches!(exists, Ok(Some(_))) {
        return;
    }

    info!(
        "New user detected for daily, adding user {} ({}) in guild id {} to database",
        user_name, user_id, guild_id
    );

    if let Err(why) = sqlx::query(
        "INSERT INTO dicks (user_id, guild_id, length, last_grow, growth_count, dick_of_day_count,
                           pvp_wins, pvp_losses, pvp_max_streak, pvp_current_streak,
                           cm_won, cm_lost)
         VALUES (?, ?, 0, datetime('now', '-2 days'), 0, 0, 0, 0, 0, 0, 0, 0)",
    )
    .bind(user_id)
    .bind(guild_id)
    .execute(&bot.database)
    .await
    {
        error!("Error creating user for daily: {:?}", why);
    }
}

async fn fetch_length(bot: &Bot, user_id: &str, guild_id: &str) -> Option<i64> {
    let row = sqlx::query("SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?")
        .bind(user_id)
        .bind(guild_id)
        .fetch_optional(&bot.database)
        .await
        .ok()??;

    row.try_get("length").ok()
}

async fn log_length_history(
    bot: &Bot,
    user_id: &str,
    guild_id: &str,
    length: i64,
    growth_amount: i64,
    growth_type: &str,
) {
    if let Err(why) = sqlx::query(
        "INSERT INTO length_history (user_id, guild_id, length, growth_amount, growth_type)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(guild_id)
    .bind(length)
    .bind(growth_amount)
    .bind(growth_type)
    .execute(&bot.database)
    .await
    {
        error!("Error logging daily history: {:?}", why);
    }
}

async fn increment_daily_counter(
    bot: &Bot,
    user_id: &str,
    guild_id: &str,
    column_name: &str,
    now_str: &str,
) -> Result<(), sqlx::Error> {
    let query = format!(
        "UPDATE dicks
         SET daily_last_claimed = ?, {column_name} = {column_name} + 1
         WHERE user_id = ? AND guild_id = ?"
    );

    sqlx::query(&query)
        .bind(now_str)
        .bind(user_id)
        .bind(guild_id)
        .execute(&bot.database)
        .await?;

    Ok(())
}

async fn consume_daily_counter(
    bot: &Bot,
    user_id: &str,
    guild_id: &str,
    column_name: &str,
) -> bool {
    let query = format!("SELECT {column_name} FROM dicks WHERE user_id = ? AND guild_id = ?");
    let count = match sqlx::query(&query)
        .bind(user_id)
        .bind(guild_id)
        .fetch_optional(&bot.database)
        .await
    {
        Ok(Some(row)) => row.try_get::<i64, _>(column_name).unwrap_or_default(),
        Ok(None) => 0,
        Err(why) => {
            error!("Error checking daily counter {}: {:?}", column_name, why);
            0
        }
    };

    if count <= 0 {
        return false;
    }

    if let Err(why) = decrement_daily_counter(bot, user_id, guild_id, column_name).await {
        error!("Error consuming daily counter {}: {:?}", column_name, why);
    }

    true
}

async fn decrement_daily_counter(
    bot: &Bot,
    user_id: &str,
    guild_id: &str,
    column_name: &str,
) -> Result<(), sqlx::Error> {
    let query = format!(
        "UPDATE dicks
         SET {column_name} = CASE WHEN {column_name} > 0 THEN {column_name} - 1 ELSE 0 END
         WHERE user_id = ? AND guild_id = ?"
    );

    sqlx::query(&query)
        .bind(user_id)
        .bind(guild_id)
        .execute(&bot.database)
        .await?;

    Ok(())
}
