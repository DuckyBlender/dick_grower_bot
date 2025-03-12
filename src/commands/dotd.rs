use crate::Bot;
use crate::time::check_utc_day_reset;
use chrono::NaiveDateTime;
use log::error;
use rand::Rng;
use serenity::all::{
    CommandInteraction, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use serenity::model::id::UserId;
use serenity::prelude::*;

pub async fn handle_dotd_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> CreateInteractionResponse {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    let guild_id = command.guild_id.unwrap().to_string();

    // Check if DOTD has been done today for this guild
    match sqlx::query!(
        "SELECT last_dotd FROM guild_settings WHERE guild_id = ?",
        guild_id
    )
    .fetch_optional(&bot.database)
    .await
    {
        Ok(Some(record)) => {
            let last_dotd = NaiveDateTime::parse_from_str(&record.last_dotd, "%Y-%m-%d %H:%M:%S")
                .unwrap_or_default();

            // Check if this is a new UTC day
            let time_left = check_utc_day_reset(&last_dotd);
            if time_left.is_zero() {
                return CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .add_embed(
                            CreateEmbed::new()
                                .title("⏰ Dick of the Day Already Awarded!")
                                .description(format!(
                                    "This server has already crowned a Dick of the Day today!\n\nNext Dick of the Day in **{}h {}m**",
                                    time_left.num_hours(),
                                    time_left.num_minutes() % 60
                                ))
                                .color(0xFF5733)
                        )
                );
            }

            // If we reach here, it's a new day and we can proceed
        }
        Ok(None) => {
            // New guild, create a record with a date far in the past
            if let Err(why) = sqlx::query!(
                "INSERT INTO guild_settings (guild_id, last_dotd)
                 VALUES (?, datetime('now', '-2 days'))",
                guild_id
            )
            .execute(&bot.database)
            .await
            {
                error!("Error creating guild settings: {:?}", why);
            }

            // No need to return an actual value, we can proceed
        }
        Err(why) => {
            error!("Database error: {:?}", why);
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Database Error")
                        .description("Failed to check when the last Dick of the Day was awarded.")
                        .color(0xFF0000),
                ),
            );
        }
    };
    // Get active users in the guild
    let active_users = match sqlx::query!(
        "SELECT user_id, length FROM dicks
         WHERE guild_id = ?
         AND last_grow > datetime('now', '-7 days')",
        guild_id
    )
    .fetch_all(&bot.database)
    .await
    {
        Ok(users) => users,
        Err(why) => {
            error!("Error fetching active users: {:?}", why);
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("🔍 No Active Users")
                            .description("There are no active users who have grown their dick in the last 7 days! Everyone needs to get growing!")
                            .color(0xAAAAAA)
                    )
            );
        }
    };

    // Get active users count
    if active_users.len() < 2 {
        return CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("🔍 Not Enough Active Users")
                        .description("There need to be at least 2 active users to award Dick of the Day! Get more people growing!")
                        .color(0xAAAAAA)
                )
        );
    }

    // Select a random winner
    let winner_idx = rand::rng().random_range(0..active_users.len());
    let winner = &active_users[winner_idx];

    // Award bonus (5-10 cm - more than the automated nightly event)
    let bonus = rand::rng().random_range(5..=10);

    // Update DB
    match sqlx::query!(
        "UPDATE dicks SET length = length + ?, dick_of_day_count = dick_of_day_count + 1
         WHERE user_id = ? AND guild_id = ?",
        bonus,
        winner.user_id,
        guild_id
    )
    .execute(&bot.database)
    .await
    {
        Ok(_) => (),
        Err(why) => {
            error!("Error updating winner: {:?}", why);
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Database Error")
                        .description("Failed to update the winner's length.")
                        .color(0xFF0000),
                ),
            );
        }
    };

    // Update guild's last DOTD time
    match sqlx::query!(
        "UPDATE guild_settings SET last_dotd = datetime('now')
         WHERE guild_id = ?",
        guild_id
    )
    .execute(&bot.database)
    .await
    {
        Ok(_) => (),
        Err(why) => {
            error!("Error updating guild settings: {:?}", why);
        }
    };

    // Get winner info
    let winner_user = match UserId::new(winner.user_id.parse::<u64>().unwrap_or_default())
        .to_user(&ctx)
        .await
    {
        Ok(user) => user,
        Err(why) => {
            error!("Error fetching user: {:?}", why);
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().add_embed(
                    CreateEmbed::new()
                        .title("⚠️ User Fetch Error")
                        .description("Failed to fetch the winner's information.")
                        .color(0xFF0000),
                ),
            );
        }
    };

    // Fun titles based on length
    let title = if winner.length + bonus <= 10 {
        "Tiny but Mighty"
    } else if winner.length + bonus <= 20 {
        "Rising Star"
    } else if winner.length + bonus <= 40 {
        "Impressive Member"
    } else if winner.length + bonus <= 60 {
        "Legendary Organ"
    } else {
        "GOD OF SCHLONGS"
    };

    CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .title("🏆 Today's Dick of the Day! 🏆")
                    .color(0xFFD700) // Gold
                    .description(format!(
                        "After careful consideration, the Dick of the Day award goes to... **{}**!\n\nThis \"**{}**\" has been awarded a bonus of **+{} cm**, bringing their total to **{} cm**!\n\nCongratulations on your outstanding achievement in the field of... length!",
                        winner_user.mention(), title, bonus, winner.length + bonus
                    ))
                    .thumbnail(winner_user.face())
                    .footer(CreateEmbedFooter::new("Stay tuned for tomorrow's competition!"))
            )
    )
}
