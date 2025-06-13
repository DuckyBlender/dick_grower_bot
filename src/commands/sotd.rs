use crate::Bot;
use crate::time::check_utc_day_reset;
use crate::utils::{get_fun_title_by_rank, ordinal_suffix};
use chrono::NaiveDateTime;
use log::{error, info};
use rand::Rng;
use serenity::all::{
    CommandInteraction, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage, Mentionable,
};
use serenity::model::id::UserId;
use serenity::prelude::*;

/// Selects a random winner index from the list of active users.
pub fn choose_sotd_winner<R: Rng>(rng: &mut R, active_users_len: usize) -> usize {
    rng.random_range(0..active_users_len)
}

pub async fn handle_sotd_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> Result<(), serenity::Error> {
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
            let unix_timestamp = chrono::Utc::now().timestamp() + time_left.num_seconds();
            let discord_timestamp = format!("<t:{}:R>", unix_timestamp);

            if !time_left.is_zero() {
                let builder = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .add_embed(
                            CreateEmbed::new()
                                .title("⏰ Dick of the Day Already Awarded!")
                                .description(format!(
                                    "This server has already crowned a Dick of the Day today!\n\nNext Dick of the Day in {discord_timestamp}",
                                ))
                                .color(0xFF5733)
                        )
                );
                return command.create_response(&ctx.http, builder).await;
            }

            // If we reach here, it's a new day and we can proceed
        }
        Ok(None) => {
            // New guild, create a record with a date far in the past
            info!("New guild detected, adding guild {} to database", guild_id);
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
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Database Error")
                        .description("Failed to check when the last Dick of the Day was awarded.")
                        .color(0xFF0000),
                ),
            );
            return command.create_response(&ctx.http, builder).await;
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
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("🔍 No Active Users")
                            .description("There are no active users who have grown their dick in the last 7 days! Everyone needs to get growing!")
                            .color(0xAAAAAA)
                    )
            );
            return command.create_response(&ctx.http, builder).await;
        }
    };

    // Get active users count
    if active_users.len() < 2 {
        let builder = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("🔍 Not Enough Active Users")
                        .description("There need to be at least 2 active users to award Dick of the Day! Get more people growing!")
                        .color(0xAAAAAA)
                )
        );
        return command.create_response(&ctx.http, builder).await;
    }

    // Log all potential winners
    info!(
        "Potential SOTD candidates: {:?}",
        active_users.iter().map(|u| &u.user_id).collect::<Vec<_>>()
    );

    // Select a random winner BEFORE any .await after this point
    let winner_idx = {
        let mut rng = rand::rng();
        choose_sotd_winner(&mut rng, active_users.len())
    };
    info!(
        "Generating a random number between 0 and {}: {}",
        active_users.len() - 1,
        winner_idx
    );
    let winner = &active_users[winner_idx];

    // Award bonus
    let bonus = rand::rng().random_range(10..=25);

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
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Database Error")
                        .description("Failed to update the winner's length.")
                        .color(0xFF0000),
                ),
            );
            return command.create_response(&ctx.http, builder).await;
        }
    };

    // Log the DOTD bonus in length_history
    let winner_total_length = winner.length + bonus;
    if let Err(why) = sqlx::query!(
        "INSERT INTO length_history (user_id, guild_id, length, growth_amount, growth_type)
         VALUES (?, ?, ?, ?, 'dotd')",
        winner.user_id,
        guild_id,
        winner_total_length,
        bonus
    )
    .execute(&bot.database)
    .await
    {
        error!("Error logging DOTD history: {:?}", why);
    }

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
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().add_embed(
                    CreateEmbed::new()
                        .title("⚠️ User Fetch Error")
                        .description("Failed to fetch the winner's information.")
                        .color(0xFF0000),
                ),
            );
            return command.create_response(&ctx.http, builder).await;
        }
    };

    let winner_mention = winner_user.mention();

    // Get winner's position in server top  
    let position = match sqlx::query!(
        "SELECT COUNT(*) as pos FROM dicks WHERE guild_id = ? AND length > ?",
        guild_id,
        winner_total_length
    )
    .fetch_one(&bot.database)
    .await
    {
        Ok(record) => (record.pos + 1) as usize,
        Err(_) => 0,
    };

    // Fun titles based on server rank
    let title = get_fun_title_by_rank(position);

    // Calculate next DOTD time (cooldown)
    let next_dotd_unix = (chrono::Utc::now() + chrono::Duration::days(1)).timestamp();
    let next_dotd_discord = format!("<t:{}:R>", next_dotd_unix);

    let builder = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .content(winner_mention.to_string())
            .add_embed(
                CreateEmbed::new()
                    .title("🏆 Today's Dick of the Day! 🏆")
                    .color(0xFFD700) // Gold
                    .description(format!(
                        "After careful consideration, the Dick of the Day award goes to... **{}**!\n\nThis \"**{}**\" has been awarded a bonus of **+{} cm**, bringing their total to **{} cm**!\n\nYou are currently **{}{}** in the server.\n\nNext Dick of the Day: {}\n\nCongratulations on your outstanding achievement in the field of... length!",
                        winner_mention, title, bonus, winner.length + bonus, position, ordinal_suffix(position), next_dotd_discord
                    ))
                    .thumbnail(winner_user.face())
                    .footer(CreateEmbedFooter::new("Stay tuned for tomorrow's competition! (and don't forget to /grow)"))
            )
    );
    return command.create_response(&ctx.http, builder).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn test_choose_dotd_winner_distribution() {
        let mut counts = [0; 3];
        let mut rng = StdRng::seed_from_u64(42);
        let runs = 1000;
        for _ in 0..runs {
            let idx = choose_sotd_winner(&mut rng, 3);
            counts[idx] += 1;
        }
        for (i, &count) in counts.iter().enumerate() {
            let percent = count as f64 / runs as f64;
            println!("User {}: {} times ({:.1}%)", i, count, percent * 100.0);
            assert!(
                percent > 0.3,
                "User {} was chosen less than 30% of the time",
                i
            );
        }
    }
}
