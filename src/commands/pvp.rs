use crate::Bot;
use chrono::Duration;
use log::{error, info};
use rand::Rng;
use serenity::all::{
    ButtonStyle, CommandInteraction, CreateActionRow, CreateButton, CreateEmbed, CreateEmbedFooter,
    CreateInteractionResponse, CreateInteractionResponseMessage,
};
use serenity::model::id::UserId;
use serenity::prelude::*;
use std::cmp::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct PvpChallenge {
    bet: i64,
    created_at: u64,
    guild_id: u64,
}

pub async fn handle_pvp_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> Result<(), serenity::Error>  {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    let options = &command.data.options;
    let bet = options[0].value.as_i64().unwrap();

    let challenger_id = command.user.id;
    let guild_id = command.guild_id.unwrap();

    // Validate bet
    if bet <= 0 {
        let builder = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("❌ Invalid Bet")
                        .description("You need to bet at least 1 cm! Don't be so stingy with your centimeters.")
                        .color(0xFF0000),
                )
                .ephemeral(true),
        );
        return command.create_response(&ctx.http, builder).await;
    }

    // Check if challenger has enough length
    let challenger_id_str = challenger_id.to_string();
    let guild_id_str = guild_id.to_string();
    let challenger_length = match sqlx::query!(
        "SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?",
        challenger_id_str,
        guild_id_str
    )
    .fetch_optional(&bot.database)
    .await
    {
        Ok(Some(record)) => record.length,
        Ok(None) => {
            // Create new user
            info!(
                "New user detected, adding user {} ({}) in guild id {} to database",
                command.user.name, challenger_id, guild_id
            );
            match sqlx::query!(
                "INSERT INTO dicks (user_id, guild_id, length, last_grow, dick_of_day_count, 
                                   pvp_wins, pvp_losses, pvp_max_streak, pvp_current_streak,
                                   cm_won, cm_lost)
                 VALUES (?, ?, 0, datetime('now', '-2 days'), 0, 0, 0, 0, 0, 0, 0)",
                challenger_id_str,
                guild_id_str
            )
            .execute(&bot.database)
            .await
            {
                Ok(_) => 0,
                Err(why) => {
                    error!("Error creating user: {:?}", why);
                    0
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
                            .description("Failed to check your length. The measuring tape broke.")
                            .color(0xFF0000),
                    )
                    .ephemeral(true),
            );
            return command.create_response(&ctx.http, builder).await;
        }
    };

    if challenger_length < bet {
        let builder = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("❌ Insufficient Length")
                        .description(format!(
                            "You only have **{} cm** but you're trying to bet **{} cm**!\n\nYou can't bet what you don't have, buddy. Your ambition outweighs your equipment.",
                            challenger_length, bet
                        ))
                        .color(0xFF0000),
                )
                .ephemeral(true),
        );
        return command.create_response(&ctx.http, builder).await;
    }

    // Create PVP challenge
    let challenge_id = format!("ch:{}", challenger_id);
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut pvp_challenges = bot.pvp_challenges.write().await;
    pvp_challenges.insert(
        challenge_id.clone(),
        PvpChallenge {
            bet,
            created_at: current_time,
            guild_id: guild_id.get(),
        },
    );

    // Get challenger username
    let challenger = match ctx.http.get_user(challenger_id).await {
        Ok(user) => user.name,
        Err(_) => "Unknown User".to_string(),
    };

    // Create accept button
    let accept_button = CreateButton::new(format!("pvp_accept:{}", challenger_id))
        .label("Accept Challenge")
        .style(ButtonStyle::Success)
        .emoji('🔥');

    let components = vec![CreateActionRow::Buttons(vec![accept_button])];

    // Create bet description based on size
    let bet_description = if bet >= 50 {
        "**HOLY MOLY!** This is a high-stakes dick measuring contest!"
    } else if bet >= 25 {
        "That's quite a sizeable wager! Someone's feeling confident!"
    } else if bet >= 10 {
        "A decent bet! More than a day's growth on the line."
    } else if bet >= 5 {
        "A reasonable bet for a friendly competition."
    } else {
        "A cautious bet. Not everyone's ready to risk their precious centimeters!"
    };

    let builder = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .title("🥊 Dick Battle!")
                    .description(format!(
                        "**{}** has started a dick battle!\n\nBet amount: **{} cm**\n\n{}\n\nAnyone can accept this challenge by clicking the button below",
                        challenger, bet, bet_description
                    ))
                    .color(0x3498DB) // Blue
                    .footer(CreateEmbedFooter::new("May the strongest dong win!")),
            )
            .components(components),
    );
    return command.create_response(&ctx.http, builder).await;
}

pub async fn handle_pvp_accept(
    ctx: &Context,
    component: &serenity::model::application::ComponentInteraction,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    let custom_id = &component.data.custom_id;
    let challenger_id_str = custom_id.split(':').nth(1).unwrap_or_default();
    let challenger_id = UserId::new(challenger_id_str.parse::<u64>().unwrap_or_default());
    let challenged_id = component.user.id;

    // Check if user is trying to accept their own challenge
    if challenger_id == challenged_id {
        component
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .add_embed(
                            CreateEmbed::new()
                                .title("🤨 Self-Challenge Detected")
                                .description(
                                    "You can't accept your own challenge! That would be... weird.",
                                )
                                .color(0xFF9900),
                        )
                        .ephemeral(true),
                ),
            )
            .await?;
        return Ok(());
    }

    // Get the challenge
    let mut pvp_challenges = bot.pvp_challenges.write().await;

    let challenge_id = format!("ch:{}", challenger_id);
    let challenge = match pvp_challenges.get(&challenge_id) {
        Some(c) => c,
        None => {
            component.create_response(
                &ctx.http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .add_embed(
                            CreateEmbed::new()
                                .title("❓ No Active Challenge")
                                .description("This challenge no longer exists. It might have expired or been accepted by someone else.")
                                .color(0xAAAAAA),
                        )
                        .ephemeral(true),
                ),
            ).await?;
            return Ok(());
        }
    };

    // Check if challenge is expired (1 hour)
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    if current_time - challenge.created_at > Duration::hours(24).num_seconds() as u64 {
        info!("Challenge expired: {}", challenge_id);

        // Remove expired challenge
        pvp_challenges.remove(&challenge_id);

        component
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .add_embed(
                            CreateEmbed::new()
                                .title("⏰ Challenge Expired")
                                .description(
                                    "This challenge has expired after 24h. You took too long to accept!",
                                )
                                .color(0xAAAAAA),
                        )
                        .ephemeral(true),
                ),
            )
            .await?;
        return Ok(());
    }

    let guild_id = challenge.guild_id;
    let bet = challenge.bet;

    // Check if challenger still has enough length
    let challenger_id_str = challenger_id.to_string();
    let guild_id_str = guild_id.to_string();
    let challenger_length = match sqlx::query!(
        "SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?",
        challenger_id_str,
        guild_id_str
    )
    .fetch_optional(&bot.database)
    .await
    {
        Ok(Some(record)) => record.length,
        Ok(None) => 0, // Should not happen
        Err(why) => {
            error!("Database error: {:?}", why);
            component
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .add_embed(
                                CreateEmbed::new()
                                    .title("⚠️ Database Error")
                                    .description("Failed to check challenger's length.")
                                    .color(0xFF0000),
                            )
                            .ephemeral(true),
                    ),
                )
                .await?;
            return Ok(());
        }
    };

    if challenger_length < bet {
        component.create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("❌ Challenger Has Insufficient Length")
                            .description(format!(
                                "The challenger only has **{} cm** left but is trying to bet **{} cm**!\n\nThey can't cover the bet anymore. Challenge declined.",
                                challenger_length, bet
                            ))
                            .color(0xFF0000),
                    )
                    .ephemeral(true),
            ),
        ).await?;
        pvp_challenges.remove(&challenge_id);
        return Ok(());
    }

    // Check if challenged user has enough length
    let challenged_id_str = challenged_id.to_string();
    let guild_id_str = guild_id.to_string();
    let challenged_length = match sqlx::query!(
        "SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?",
        challenged_id_str,
        guild_id_str
    )
    .fetch_optional(&bot.database)
    .await
    {
        Ok(Some(record)) => record.length,
        Ok(None) => {
            // Create new user
            info!(
                "New user detected, adding user {} ({}) in guild id {} to database",
                component.user.name, challenged_id, guild_id
            );
            match sqlx::query!(
                "INSERT INTO dicks (user_id, guild_id, length, last_grow, dick_of_day_count, 
                                   pvp_wins, pvp_losses, pvp_max_streak, pvp_current_streak,
                                   cm_won, cm_lost)
                 VALUES (?, ?, 0, datetime('now', '-2 days'), 0, 0, 0, 0, 0, 0, 0)",
                challenged_id_str,
                guild_id_str
            )
            .execute(&bot.database)
            .await
            {
                Ok(_) => 0,
                Err(why) => {
                    error!("Error creating user: {:?}", why);
                    0
                }
            }
        }
        Err(why) => {
            error!("Database error: {:?}", why);
            component
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .add_embed(
                                CreateEmbed::new()
                                    .title("⚠️ Database Error")
                                    .description("Failed to check your length.")
                                    .color(0xFF0000),
                            )
                            .ephemeral(true),
                    ),
                )
                .await?;
            return Ok(());
        }
    };

    if challenged_length < bet {
        info!(
            "Challenged user has insufficient length: {} < {}",
            challenged_length, bet
        );
        component.create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("❌ Insufficient Length")
                            .description(format!(
                                "You only have **{} cm** left but you're trying to accept a bet of **{} cm**!\n\nYou can't compete with what you don't have. Grow a bit more first.",
                                challenged_length, bet
                            ))
                            .color(0xFF0000),
                    )
                    .ephemeral(true),
            ),
        ).await?;
        return Ok(()); // Don't remove the challenge, let others accept it
    }

    // Get challenger info
    pvp_challenges.remove(&challenge_id).unwrap();

    // Drop the lock before making async calls
    drop(pvp_challenges);

    // Get usernames
    let challenger = match ctx.http.get_user(challenger_id).await {
        Ok(user) => user.name,
        Err(_) => "Unknown User".to_string(),
    };

    let challenged = component.user.name.clone();

    // Roll for both users
    let challenger_roll = rand::rng().random_range(1..=100);
    let challenged_roll = rand::rng().random_range(1..=100);

    let (winner_id, loser_id, winner_name, loser_name, winner_roll, loser_roll) =
        match challenger_roll.cmp(&challenged_roll) {
            Ordering::Greater => (
                challenger_id,
                challenged_id,
                challenger.clone(),
                challenged.clone(),
                challenger_roll,
                challenged_roll,
            ),
            Ordering::Less => (
                challenged_id,
                challenger_id,
                challenged.clone(),
                challenger.clone(),
                challenged_roll,
                challenger_roll,
            ),
            Ordering::Equal => {
                // It's a tie! Handle this special case
                let tie_comment = if bet >= 30 {
                    format!(
                        "A {} cm bet and it ends in a tie?! The dick gods must be laughing!",
                        bet
                    )
                } else if bet >= 15 {
                    "Insanity! Neither dick emerged victorious today!".to_string()
                } else {
                    "What are the odds?! Both measuring exactly the same!".to_string()
                };

                component.create_response(
                    &ctx.http,
                    CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new()
                            .add_embed(
                                CreateEmbed::new()
                                    .title("🤯 INCREDIBLE! It's a Tie!")
                                    .description(format!(
                                        "The contest has concluded with an unbelievable outcome!\n\n**{}** rolled **{}**\n**{}** rolled **{}**\n\n{}\n\nBoth dicks measured EXACTLY the same! The bet has been returned to both competitors. No winners, no losers today!",
                                        challenger, challenger_roll,
                                        challenged, challenged_roll,
                                        tie_comment
                                    ))
                                    .color(0x9b59b6) // Purple for a tie
                                    .footer(CreateEmbedFooter::new("A moment that will go down in dick-measuring history!"))
                            )
                            .components(vec![]), // Remove the button
                    ),
                ).await?;

                return Ok(());
            }
        };

    // Get previous streak
    let winner_id_str = winner_id.to_string();
    let guild_id_str = guild_id.to_string();
    let loser_id_str = loser_id.to_string();
    let winner_streak = match sqlx::query!(
        "SELECT pvp_current_streak FROM dicks WHERE user_id = ? AND guild_id = ?",
        winner_id_str,
        guild_id_str
    )
    .fetch_optional(&bot.database)
    .await
    {
        Ok(Some(record)) => record.pvp_current_streak,
        Ok(None) => 0,
        Err(why) => {
            error!("Error getting streak: {:?}", why);
            0
        }
    };

    let new_winner_streak = winner_streak + 1;

    // Update the database for winner
    match sqlx::query!(
        "UPDATE dicks SET length = length + ?, 
         pvp_wins = pvp_wins + 1,
         pvp_current_streak = ?,
         pvp_max_streak = CASE WHEN ? > pvp_max_streak THEN ? ELSE pvp_max_streak END,
         cm_won = cm_won + ?
         WHERE user_id = ? AND guild_id = ?",
        bet,
        new_winner_streak,
        new_winner_streak,
        new_winner_streak,
        bet,
        winner_id_str,
        guild_id_str
    )
    .execute(&bot.database)
    .await
    {
        Ok(_) => (),
        Err(why) => error!("Error updating winner: {:?}", why),
    };

    // Update the database for loser
    match sqlx::query!(
        "UPDATE dicks SET 
         length = length - ?,
         pvp_losses = pvp_losses + 1,
         pvp_current_streak = 0,
         cm_lost = cm_lost + ?
         WHERE user_id = ? AND guild_id = ?",
        bet,
        bet,
        loser_id_str,
        guild_id_str
    )
    .execute(&bot.database)
    .await
    {
        Ok(_) => (),
        Err(why) => error!("Error updating loser: {:?}", why),
    };

    // Get updated lengths
    let winner_length = match sqlx::query!(
        "SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?",
        winner_id_str,
        guild_id_str
    )
    .fetch_one(&bot.database)
    .await
    {
        Ok(record) => record.length,
        Err(_) => 0,
    };

    let loser_length = match sqlx::query!(
        "SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?",
        loser_id_str,
        guild_id_str
    )
    .fetch_one(&bot.database)
    .await
    {
        Ok(record) => record.length,
        Err(_) => 0,
    };

    // Create a funny taunt based on margin of victory and bet size
    let taunt = if winner_roll - loser_roll > 50 {
        if bet >= 30 {
            format!(
                "💀 It wasn't even close! {}'s dick absolutely DEMOLISHED {}'s in a historic beatdown! Those {} centimeters will be remembered for generations! 📜",
                winner_name, loser_name, bet
            )
        } else {
            format!(
                "💀 It wasn't even close! {}'s dick destroyed {}'s in an absolute massacre! ⚰️",
                winner_name, loser_name
            )
        }
    } else if winner_roll - loser_roll > 20 {
        if bet >= 20 {
            format!(
                "🏆 {}'s dick clearly outclassed {}'s in this epic showdown! That's {} cm of pride changing hands!",
                winner_name, loser_name, bet
            )
        } else {
            format!(
                "🏆 {}'s dick clearly outclassed {}'s in this epic showdown!",
                winner_name, loser_name
            )
        }
    } else if winner_roll - loser_roll > 5 {
        if bet >= 15 {
            format!(
                "🥇 A close match, but {}'s dick had just enough extra length to claim victory and snatch those {} valuable centimeters!",
                winner_name, bet
            )
        } else {
            format!(
                "🥇 A close match, but {}'s dick had just enough extra length to claim victory!",
                winner_name
            )
        }
    } else if bet >= 25 {
        format!(
            "😱 WHAT A NAIL-BITER! {}'s dick barely edged out {}'s by a hair's width! Those {} centimeters were almost too close to call!",
            winner_name, loser_name, bet
        )
    } else {
        format!(
            "😮 That was incredibly close! {}'s dick barely edged out {}'s by a hair's width!",
            winner_name, loser_name
        )
    };

    // Add a comment on the size of the bet
    let bet_comment = if bet >= 50 {
        format!(
            "\n\n💰 **MASSIVE BET!** {} cm is roughly a week's worth of growth! Talk about high stakes!",
            bet
        )
    } else if bet >= 30 {
        format!(
            "\n\n💰 A **huge {} cm bet**! That's several days of growth on the line!",
            bet
        )
    } else if bet >= 15 {
        format!(
            "\n\n💰 A solid **{} cm bet** - more than a day's worth of growth!",
            bet
        )
    } else if bet >= 10 {
        "\n\n💰 A respectable wager, putting a full day's growth at stake!".to_string()
    } else {
        "".to_string() // No special comment for smaller bets
    };

    // Streak comment
    let streak_comment = if new_winner_streak >= 5 {
        format!(
            "\n\n🔥 **{}** is on a **{}-win streak**! Absolutely dominating! 👑",
            winner_name, new_winner_streak
        )
    } else if new_winner_streak >= 3 {
        format!(
            "\n\n🔥 **{}** is on a **{}-win streak**! 📈",
            winner_name, new_winner_streak
        )
    } else {
        "".to_string()
    };

    component.create_response(
        &ctx.http,
        CreateInteractionResponse::UpdateMessage(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("🏆 Dick Battle Results!")
                        .description(format!(
                            "The contest has concluded!\n\n**{}** rolled **{}**\n**{}** rolled **{}**\n\n**{}** wins **{} cm**!\n\nNew lengths:\n**{}**: {} cm\n**{}**: {} cm\n\n{}{}{}",
                            challenger, challenger_roll,
                            challenged, challenged_roll,
                            winner_name, bet,
                            winner_name, winner_length,
                            loser_name, loser_length,
                            taunt,
                            bet_comment,
                            streak_comment
                        ))
                        .color(0x2ECC71) // Green
                        .footer(CreateEmbedFooter::new("Size DOES matter after all!"))
                )
                .components(vec![]), // Remove the button
        ),
    ).await?;

    Ok(())
}
