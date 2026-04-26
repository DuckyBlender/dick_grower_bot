use crate::Bot;
use crate::time::check_utc_day_reset;
use crate::utils::pluralize;
use chrono::NaiveDateTime;
use log::error;
use serenity::all::{
    CommandInteraction, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use serenity::prelude::*;
use sqlx::Row;

pub async fn handle_streak_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> Result<(), serenity::Error> {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    let user_id = command.user.id.to_string();
    let guild_id = command.guild_id.unwrap().to_string();

    let row = match sqlx::query(
        "SELECT daily_streak, best_daily_streak, last_streak_date, streak_last_claimed, length
         FROM dicks
         WHERE user_id = ? AND guild_id = ?",
    )
    .bind(&user_id)
    .bind(&guild_id)
    .fetch_optional(&bot.database)
    .await
    {
        Ok(Some(row)) => row,
        Ok(None) => {
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("❓ No Streak Yet")
                            .description(
                                "Use /grow today before trying to cash in a streak reward.",
                            )
                            .color(0xAAAAAA),
                    )
                    .ephemeral(true),
            );
            return command.create_response(&ctx.http, builder).await;
        }
        Err(why) => {
            error!("Database error checking streak: {:?}", why);
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Streak Error")
                        .description("Failed to check your streak. The calendar caught fire.")
                        .color(0xFF0000),
                ),
            );
            return command.create_response(&ctx.http, builder).await;
        }
    };

    let today = chrono::Utc::now().date_naive();
    let last_streak_date = row
        .try_get::<Option<String>, _>("last_streak_date")
        .ok()
        .flatten()
        .and_then(|date| chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d").ok());

    if last_streak_date != Some(today) {
        let builder = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("📅 Grow First")
                        .description("Your streak reward only unlocks after today's /grow.")
                        .color(0xFF9900),
                )
                .ephemeral(true),
        );
        return command.create_response(&ctx.http, builder).await;
    }

    if let Some(last_claimed_str) = row
        .try_get::<Option<String>, _>("streak_last_claimed")
        .ok()
        .flatten()
        && let Ok(last_claimed) =
            NaiveDateTime::parse_from_str(&last_claimed_str, "%Y-%m-%d %H:%M:%S")
    {
        let time_left = check_utc_day_reset(&last_claimed);
        if !time_left.is_zero() {
            let unix_timestamp = chrono::Utc::now().timestamp() + time_left.num_seconds();
            let discord_timestamp = format!("<t:{}:R>", unix_timestamp);
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("🕒 Streak Already Claimed")
                            .description(format!(
                                "You've already claimed today's streak reward.\n\nNext reward: {discord_timestamp}"
                            ))
                            .color(0xFF5733),
                    )
                    .ephemeral(true),
            );
            return command.create_response(&ctx.http, builder).await;
        }
    }

    let streak = row.try_get::<i64, _>("daily_streak").unwrap_or_default();
    let best_streak = row
        .try_get::<i64, _>("best_daily_streak")
        .unwrap_or_default();
    let reward = (streak * 2).clamp(2, 30);
    let now_str = chrono::Utc::now()
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    if let Err(why) = sqlx::query(
        "UPDATE dicks
         SET length = length + ?, streak_last_claimed = ?
         WHERE user_id = ? AND guild_id = ?",
    )
    .bind(reward)
    .bind(now_str)
    .bind(&user_id)
    .bind(&guild_id)
    .execute(&bot.database)
    .await
    {
        error!("Error applying streak reward: {:?}", why);
        let builder = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().add_embed(
                CreateEmbed::new()
                    .title("⚠️ Streak Error")
                    .description("Failed to apply your streak reward. The calendar lied.")
                    .color(0xFF0000),
            ),
        );
        return command.create_response(&ctx.http, builder).await;
    }

    let new_length = row.try_get::<i64, _>("length").unwrap_or_default() + reward;
    if let Err(why) = sqlx::query(
        "INSERT INTO length_history (user_id, guild_id, length, growth_amount, growth_type)
         VALUES (?, ?, ?, ?, 'streak')",
    )
    .bind(&user_id)
    .bind(&guild_id)
    .bind(new_length)
    .bind(reward)
    .execute(&bot.database)
    .await
    {
        error!("Error logging streak reward: {:?}", why);
    }

    let builder = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new().add_embed(
            CreateEmbed::new()
                .title("🔥 Streak Reward Claimed!")
                .description(format!(
                    "Your **{}** growth streak paid out **+{} cm**.\n\nCurrent length: **{} cm**\nBest streak: **{}**",
                    pluralize(streak, "day", "days"),
                    reward,
                    new_length,
                    pluralize(best_streak, "day", "days")
                ))
                .color(0xE67E22)
                .footer(CreateEmbedFooter::new(
                    "Keep using /grow every UTC day to keep the streak alive.",
                )),
        ),
    );
    command.create_response(&ctx.http, builder).await
}
