use chrono::{Duration, NaiveDateTime, Utc};
use rand::Rng;
use serenity::all::{
    ButtonStyle, CommandInteraction, CreateActionRow, CreateButton, CreateCommand, CreateEmbed,
    CreateEmbedFooter, CreateInteractionResponse, CreateInteractionResponseMessage,
};
use serenity::async_trait;
use serenity::builder::CreateCommandOption;
use serenity::model::application::{CommandOptionType, Interaction};
use serenity::model::gateway::Ready;
use serenity::model::id::UserId;
use serenity::prelude::*;
use sqlx::SqlitePool;
use sqlx::{Pool, Sqlite};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

struct Handler;

impl TypeMapKey for Bot {
    type Value = Arc<Bot>;
}

struct PvpChallenge {
    bet: i64,
    created_at: u64,
    guild_id: u64,
}

struct Bot {
    database: Pool<Sqlite>,
    pvp_challenges: RwLock<HashMap<String, PvpChallenge>>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(command) => {
                if command.guild_id.is_none() {
                    // Return message notifying that the bot is only available in guilds
                    if let Err(why) = command.create_response(&ctx.http, 
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .add_embed(
                                    CreateEmbed::new()
                                        .title("⚠️ Server Only Bot")
                                        .description("This bot can only be used in a server, not in direct messages.")
                                        .color(0xFF5733)
                                        .footer(CreateEmbedFooter::new(
                                            "Please use this bot in a server where it is invited and begin your cucumber journey!",
                                        ))
                                )
                                .ephemeral(true)
                        )
                    ).await {
                        println!("Cannot respond to slash command for guild check: {why}");
                    }
                }
                // Check if interaction is in a guild
                let content = match command.data.name.as_str() {
                    "grow" => handle_grow_command(&ctx, &command).await,
                    "top" => handle_top_command(&ctx, &command).await,
                    "global" => handle_global_command(&ctx, &command).await,
                    "pvp" => handle_pvp_command(&ctx, &command).await,
                    "stats" => handle_stats_command(&ctx, &command).await,
                    "dickoftheday" => handle_dotd_command(&ctx, &command).await,
                    _ => CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("Not implemented")
                            .ephemeral(true),
                    ),
                };

                if let Err(why) = command.create_response(&ctx.http, content).await {
                    println!("Cannot respond to slash command: {why}");
                }
            }
            Interaction::Component(component) => {
                // Handle button interactions
                if component.data.custom_id.starts_with("pvp_accept:") {
                    if let Err(why) = handle_pvp_accept(&ctx, &component).await {
                        println!("Error handling PVP accept: {why}");
                        if let Err(e) = component
                            .create_response(
                                &ctx.http,
                                CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new()
                                        .content("Something went wrong processing your request")
                                        .ephemeral(true),
                                ),
                            )
                            .await
                        {
                            println!("Error responding to component interaction: {e}");
                        }
                    }
                }
            }
            _ => {}
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        // Register commands globally
        let commands = vec![
            CreateCommand::new("grow").description("Grow your cucumber daily"),
            CreateCommand::new("top")
                .description("Show the top players with the biggest weapons in this server"),
            CreateCommand::new("global")
                .description("Show the top players with the biggest weapons across all servers"),
            CreateCommand::new("pvp")
                .description("Start a dick measuring contest")
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::Integer,
                        "bet",
                        "The amount of cm you want to bet",
                    )
                    .required(true)
                    .min_int_value(1),
                ),
            CreateCommand::new("stats").description("View your dick stats"),
            CreateCommand::new("dickoftheday")
                .description("Randomly select a Dick of the Day (once per day per server)"),
        ];

        if let Err(why) = ctx.http.create_global_commands(&commands).await {
            println!("Error creating global commands: {why}");
        }
    }
}

async fn handle_grow_command(
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
            let now = Utc::now().naive_utc();

            // If less than 24 hours have passed
            if now.signed_duration_since(last_grow) < Duration::days(1) {
                let next_grow = last_grow + Duration::days(1);
                let time_left = next_grow.signed_duration_since(now);

                return CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .add_embed(
                            CreateEmbed::new()
                                .title("🕒 Hold up, speedy!")
                                .description(format!(
                                    "Your dick needs more time to recover! Try again in **{}h {}m**.\n\nExcessive stimulation might cause injuries, you know?",
                                    time_left.num_hours(),
                                    time_left.num_minutes() % 60
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
                Err(why) => println!("Error creating user: {:?}", why),
            };

            Utc::now().naive_utc()
        }
        Err(why) => {
            println!("Database error: {:?}", why);
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
    let growth = rand::thread_rng().gen_range(-5..=10);

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
            println!("Error updating length: {:?}", why);
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
            println!("Error fetching length: {:?}", why);
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

async fn handle_top_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> CreateInteractionResponse {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    let guild_id = command.guild_id.unwrap().to_string();

    // Get top 10 users in this server
    let top_users = match sqlx::query!(
        "SELECT user_id, length FROM dicks 
         WHERE guild_id = ? 
         ORDER BY length DESC LIMIT 10",
        guild_id
    )
    .fetch_all(&bot.database)
    .await
    {
        Ok(users) => users,
        Err(why) => {
            println!("Error fetching top users: {:?}", why);
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Leaderboard Error")
                        .description(
                            "Failed to measure all the dicks. Some were too small to find.",
                        )
                        .color(0xFF0000),
                ),
            );
        }
    };

    if top_users.is_empty() {
        return CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().add_embed(
                CreateEmbed::new()
                    .title("👀 No Dicks Found")
                    .description(
                        "Nobody has grown their dick in this server yet. Be the first one!",
                    )
                    .color(0xAAAAAA),
            ),
        );
    }

    // Build the leaderboard
    let mut description = "Here are the biggest dicks in this server:\n\n".to_string();

    for (i, user) in top_users.iter().enumerate() {
        let medal = match i {
            0 => "🥇",
            1 => "🥈",
            2 => "🥉",
            _ => "🔹",
        };

        let username = match UserId::new(user.user_id.parse::<u64>().unwrap_or_default())
            .to_user(&ctx)
            .await
        {
            Ok(user) => user.name,
            Err(_) => "Unknown User".to_string(),
        };

        let emoji = if i == 0 {
            "🍆 "
        } else if user.length <= 0 {
            "🥜 "
        } else if user.length > 30 {
            "🌵 "
        } else {
            ""
        };

        description.push_str(&format!(
            "{} **{}. {}{}**: {} cm\n",
            medal,
            i + 1,
            emoji,
            username,
            user.length
        ));
    }

    // Add funny comment about the winner
    if !top_users.is_empty() {
        let winner_name = match UserId::new(top_users[0].user_id.parse::<u64>().unwrap_or_default())
            .to_user(&ctx)
            .await
        {
            Ok(user) => user.name,
            Err(_) => "Unknown User".to_string(),
        };

        let length = top_users[0].length;
        let winner_comment = if length > 50 {
            format!(
                "Holy moly! {}' dick is so big it needs its own ZIP code!",
                winner_name
            )
        } else if length > 30 {
            format!(
                "Beware of {} in tight spaces. That thing is a lethal weapon!",
                winner_name
            )
        } else if length > 15 {
            format!(
                "{} is doing quite well. Impressive... most impressive.",
                winner_name
            )
        } else if length > 0 {
            format!(
                "{} is trying their best, though. Gold star for effort!",
                winner_name
            )
        } else {
            format!(
                "Poor {}... we need a microscope to find their dick.",
                winner_name
            )
        };

        description.push_str(&format!("\n\n{}", winner_comment));
    }

    let guild_name = match command.guild_id.unwrap().to_partial_guild(&ctx).await {
        Ok(guild) => guild.name,
        Err(_) => "This Server".to_string(),
    };

    CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new().add_embed(
            CreateEmbed::new()
                .title(format!("🍆 Dick Leaderboard: {} 🏆", guild_name))
                .description(description)
                .color(0x9B59B6) // Purple
                .footer(CreateEmbedFooter::new(
                    "Use /grow daily to increase your length!",
                )),
        ),
    )
}

async fn handle_global_command(ctx: &Context, _: &CommandInteraction) -> CreateInteractionResponse {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    // Get top 10 users globally
    let top_users = match sqlx::query!(
        "SELECT user_id, length, guild_id FROM dicks 
         ORDER BY length DESC LIMIT 10"
    )
    .fetch_all(&bot.database)
    .await
    {
        Ok(users) => users,
        Err(why) => {
            println!("Error fetching global top users: {:?}", why);
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Global Leaderboard Error")
                        .description(
                            "Failed to measure all the world's dicks. The server is overwhelmed.",
                        )
                        .color(0xFF0000),
                ),
            );
        }
    };

    if top_users.is_empty() {
        return CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().add_embed(
                CreateEmbed::new()
                    .title("👀 No Dicks Found")
                    .description(
                        "Nobody has grown their dick anywhere yet. The world awaits a pioneer!",
                    )
                    .color(0xAAAAAA),
            ),
        );
    }

    // Build the global leaderboard
    let mut description = "Here are the biggest dicks in the entire world:\n\n".to_string();

    for (i, user) in top_users.iter().enumerate() {
        let medal = match i {
            0 => "🥇",
            1 => "🥈",
            2 => "🥉",
            _ => "🔹",
        };

        let username = match UserId::new(user.user_id.parse::<u64>().unwrap_or_default())
            .to_user(&ctx)
            .await
        {
            Ok(user) => user.name,
            Err(_) => "Unknown User".to_string(),
        };

        let guild_name = match user.guild_id.parse::<u64>() {
            Ok(id) => match ctx.http.get_guild(id.into()).await {
                Ok(guild) => guild.name,
                Err(_) => "Unknown Server".to_string(),
            },
            Err(_) => "Unknown Server".to_string(),
        };

        let emoji = if i == 0 {
            "🌎 "
        } else if user.length <= 0 {
            "🥜 "
        } else if user.length > 50 {
            "🚀 "
        } else if user.length > 30 {
            "🌵 "
        } else {
            "🍆 "
        };

        description.push_str(&format!(
            "{} **{}. {}{}**: {} cm (from {})\n",
            medal,
            i + 1,
            emoji,
            username,
            user.length,
            guild_name
        ));
    }

    // Add funny comment about the global champion
    if !top_users.is_empty() {
        let winner_name = match UserId::new(top_users[0].user_id.parse::<u64>().unwrap_or_default())
            .to_user(&ctx)
            .await
        {
            Ok(user) => user.name,
            Err(_) => "Unknown User".to_string(),
        };

        let length = top_users[0].length;
        let winner_comment = if length > 60 {
            format!(
                "NASA wants to study {}'s dick as a possible space elevator!",
                winner_name
            )
        } else if length > 40 {
            format!(
                "{} must need a special permit to carry that thing around!",
                winner_name
            )
        } else if length > 20 {
            format!(
                "{} is making the rest of the world feel inadequate!",
                winner_name
            )
        } else {
            format!(
                "{} is the global champion... though the bar seems pretty low, honestly.",
                winner_name
            )
        };

        description.push_str(&format!("\n\n{}", winner_comment));
    }

    CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new().add_embed(
            CreateEmbed::new()
                .title("🌍 Global Dick Leaderboard 🏆")
                .description(description)
                .color(0x9B59B6) // Purple
                .footer(CreateEmbedFooter::new(
                    "World domination starts with your dick. Use /grow daily!",
                )),
        ),
    )
}

async fn handle_pvp_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> CreateInteractionResponse {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    let options = &command.data.options;
    let bet = options[0].value.as_i64().unwrap();

    let challenger_id = command.user.id;
    let guild_id = command.guild_id.unwrap();

    // Validate bet
    if bet <= 0 {
        return CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("❌ Invalid Bet")
                        .description("You need to bet at least 1 cm! Don't be so stingy with your centimeters.")
                        .color(0xFF0000),
                )
                .ephemeral(true),
        );
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
                    println!("Error creating user: {:?}", why);
                    return CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .add_embed(
                                CreateEmbed::new()
                                    .title("⚠️ Database Error")
                                    .description("Failed to create an account for you.")
                                    .color(0xFF0000),
                            )
                            .ephemeral(true),
                    );
                }
            }
        }
        Err(why) => {
            println!("Database error: {:?}", why);
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("⚠️ Database Error")
                            .description("Failed to check your length. The measuring tape broke.")
                            .color(0xFF0000),
                    )
                    .ephemeral(true),
            );
        }
    };

    if challenger_length < bet {
        return CreateInteractionResponse::Message(
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

    CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .title("🥊 Dick Measuring Contest Challenge!")
                    .description(format!(
                        "**{}** has started a dick measuring contest!\n\nBet amount: **{} cm**\n\nAnyone can accept this challenge by clicking the button below",
                        challenger, bet
                    ))
                    .color(0x3498DB) // Blue
                    .footer(CreateEmbedFooter::new("May the longest dong win!")),
            )
            .components(components),
    )
}

async fn handle_pvp_accept(
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

    if current_time - challenge.created_at > 3600 {
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
                                    "This challenge has expired. You took too long to accept!",
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
            println!("Database error: {:?}", why);
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
                    ),
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
                    println!("Error creating user: {:?}", why);
                    component
                        .create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .add_embed(
                                        CreateEmbed::new()
                                            .title("⚠️ Database Error")
                                            .description("Failed to create an account for you.")
                                            .color(0xFF0000),
                                    )
                                    .ephemeral(true),
                            ),
                        )
                        .await?;
                    return Ok(());
                }
            }
        }
        Err(why) => {
            println!("Database error: {:?}", why);
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
        component.create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("❌ Insufficient Length")
                            .description(format!(
                                "You only have **{} cm** but you're trying to accept a bet of **{} cm**!\n\nYou can't compete with what you don't have. Grow a bit more first.",
                                challenged_length, bet
                            ))
                            .color(0xFF0000),
                    )
                    .ephemeral(true),
            ),
        ).await?;
        return Ok(());
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
    let challenger_roll = rand::thread_rng().gen_range(1..=100);
    let challenged_roll = rand::thread_rng().gen_range(1..=100);

    let (winner_id, loser_id, winner_name, loser_name, winner_roll, loser_roll) = match challenger_roll.cmp(&challenged_roll) {
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
            // It's a tie! Let's handle this with a special case
            component.create_response(
                &ctx.http,
                CreateInteractionResponse::UpdateMessage(
                    CreateInteractionResponseMessage::new()
                        .add_embed(
                            CreateEmbed::new()
                                .title("🤯 INCREDIBLE! It's a tie!")
                                .description(format!(
                                    "The contest has concluded with an unbelievable outcome!\n\n**{}** rolled **{}**\n**{}** rolled **{}**\n\nBoth dicks measured EXACTLY the same! What are the odds?!\n\nThe bet has been returned to both competitors. No winners, no losers today!",
                                    challenger, challenger_roll,
                                    challenged, challenged_roll
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
            println!("Error getting streak: {:?}", why);
            0
        }
    };

    let new_winner_streak = winner_streak + 1;

    // Update the database for winner
    match sqlx::query!(
        "UPDATE dicks SET 
         length = length + ?, 
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
        Err(why) => println!("Error updating winner: {:?}", why),
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
        Err(why) => println!("Error updating loser: {:?}", why),
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

    // Create a funny taunt
    let taunt = if winner_roll - loser_roll > 50 {
        format!(
            "It wasn't even close! {}'s dick destroyed {}'s in an absolute massacre!",
            winner_name, loser_name
        )
    } else if winner_roll - loser_roll > 20 {
        format!(
            "{}'s dick clearly outclassed {}'s in this epic showdown!",
            winner_name, loser_name
        )
    } else if winner_roll - loser_roll > 5 {
        format!(
            "A close match, but {}'s dick had just enough extra length to claim victory!",
            winner_name
        )
    } else {
        format!(
            "That was incredibly close! {}'s dick barely edged out {}'s by a hair's width!",
            winner_name, loser_name
        )
    };

    // Streak comment
    let streak_comment = if new_winner_streak >= 5 {
        format!(
            "\n\n🔥 **{}** is on a **{}-win streak**! Absolutely dominating!",
            winner_name, new_winner_streak
        )
    } else if new_winner_streak >= 3 {
        format!(
            "\n\n🔥 **{}** is on a **{}-win streak**!",
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
                        .title("🏆 Dick Measuring Contest Results!")
                        .description(format!(
                            "The contest has concluded!\n\n**{}** rolled **{}**\n**{}** rolled **{}**\n\n**{}** wins **{} cm**!\n\nNew lengths:\n**{}**: {} cm\n**{}**: {} cm\n\n{}{}",
                            challenger, challenger_roll,
                            challenged, challenged_roll,
                            winner_name, bet,
                            winner_name, winner_length,
                            loser_name, loser_length,
                            taunt,
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

async fn handle_stats_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> CreateInteractionResponse {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    let user_id = command.user.id.to_string();
    let guild_id = command.guild_id.unwrap().to_string();

    // Get user stats
    let user_stats = match sqlx::query!(
        "SELECT length, dick_of_day_count, last_grow, 
                pvp_wins, pvp_losses, pvp_max_streak, pvp_current_streak,
                cm_won, cm_lost
         FROM dicks 
         WHERE user_id = ? AND guild_id = ?",
        user_id,
        guild_id
    )
    .fetch_optional(&bot.database)
    .await
    {
        Ok(Some(stats)) => stats,
        Ok(None) => {
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("❓ No Stats Found")
                            .description(
                                "You haven't started growing your dick yet! Use /grow to begin your journey to greatness.",
                            )
                            .color(0xAAAAAA),
                    )
                    .ephemeral(true),
            );
        }
        Err(why) => {
            println!("Database error: {:?}", why);
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("⚠️ Database Error")
                            .description("Failed to retrieve your stats. The server's ruler broke.")
                            .color(0xFF0000),
                    )
                    .ephemeral(true),
            );
        }
    };

    // Get rank
    let rank = match sqlx::query!(
        "SELECT COUNT(*) as rank FROM dicks 
         WHERE guild_id = ? AND length > (
            SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?
         )",
        guild_id,
        user_id,
        guild_id
    )
    .fetch_one(&bot.database)
    .await
    {
        Ok(record) => record.rank + 1, // +1 because we're counting users with MORE length
        Err(why) => {
            println!("Error fetching rank: {:?}", why);
            0
        }
    };

    // Calculate next growth time
    let last_grow = NaiveDateTime::parse_from_str(&user_stats.last_grow, "%Y-%m-%d %H:%M:%S")
        .unwrap_or_default();
    let now = Utc::now().naive_utc();
    let next_grow = last_grow + Duration::days(1);

    let can_grow = now.signed_duration_since(last_grow) >= Duration::days(1);
    let growth_status = if can_grow {
        "✅ You can grow now! Use /grow".to_string()
    } else {
        let time_left = next_grow.signed_duration_since(now);
        format!(
            "⏰ Next growth in: {}h {}m",
            time_left.num_hours(),
            time_left.num_minutes() % 60
        )
    };

    // Calculate win rate
    let total_fights = user_stats.pvp_wins + user_stats.pvp_losses;
    let win_rate = if total_fights > 0 {
        (user_stats.pvp_wins as f64 / total_fights as f64) * 100.0
    } else {
        0.0
    };

    // Funny comment based on length
    let length_comment = if user_stats.length <= 0 {
        "Your dick is practically an innie at this point. Keep trying!"
    } else if user_stats.length < 10 {
        "It's... cute? At least that's what they'll say to be nice."
    } else if user_stats.length < 20 {
        "Not bad! You're in the average zone. But who wants to be average?"
    } else if user_stats.length < 30 {
        "Impressive length! You're packing some serious heat down there."
    } else if user_stats.length < 50 {
        "WOW! That's a third leg, not a dick! Do you need special pants?"
    } else {
        "LEGENDARY! Scientists want to study your mutation. BEWARE!"
    };

    CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .title(format!("🍆 {}'s Dick Stats", command.user.name))
                    .description(
                        "Here's everything you wanted to know about your cucumber (and probably some things you didn't):".to_string(),
                    )
                    .color(0x9B59B6) // Purple
                    .field("Current Length", format!("**{} cm**", user_stats.length), true)
                    .field("Server Rank", format!("**#{}**", rank), true)
                    .field(
                        "Dick of the Day",
                        format!("**{} time(s)**", user_stats.dick_of_day_count),
                        true,
                    )
                    .field("Growth Status", growth_status, false)
                    .field(
                        "Battle Stats",
                        format!(
                            "Win rate: **{:.2}%**\nFights: **{}**\nWins: **{}**\nMax win streak: **{}**\nCurrent streak: **{}**\nAcquired length: **{} cm**\nLost length: **{} cm**",
                            win_rate,
                            total_fights,
                            user_stats.pvp_wins,
                            user_stats.pvp_max_streak,
                            user_stats.pvp_current_streak,
                            user_stats.cm_won,
                            user_stats.cm_lost
                        ),
                        false
                    )
                    .field("Professional Assessment", length_comment, false)
                    .thumbnail(command.user.face())
                    .footer(CreateEmbedFooter::new("Remember to /grow every day for maximum results!")),
            )
            .ephemeral(true),
    )
}

async fn handle_dotd_command(
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
            let now = Utc::now().naive_utc();

            // If less than 24 hours have passed
            if now.signed_duration_since(last_dotd) < Duration::days(1) {
                let next_dotd = last_dotd + Duration::days(1);
                let time_left = next_dotd.signed_duration_since(now);

                return CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .add_embed(
                            CreateEmbed::new()
                                .title("⏰ Dick of the Day Already Awarded!")
                                .description(format!(
                                    "This server has already crowned a Dick of the Day today!\n\nNext Dick of the Day in: **{}h {}m**",
                                    time_left.num_hours(),
                                    time_left.num_minutes() % 60
                                ))
                                .color(0xFF5733)
                        )
                );
            }

            // If we reach here, enough time has passed and we can proceed
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
                println!("Error creating guild settings: {:?}", why);
            }

            // No need to return an actual value, we can proceed
        }
        Err(why) => {
            println!("Database error: {:?}", why);
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
            println!("Error fetching active users: {:?}", why);
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

    if active_users.is_empty() {
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

    // Select a random winner
    let winner_idx = rand::thread_rng().gen_range(0..active_users.len());
    let winner = &active_users[winner_idx];

    // Award bonus (5-10 cm - more than the automated nightly event)
    let bonus = rand::thread_rng().gen_range(5..=10);

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
            println!("Error updating winner: {:?}", why);
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
        Err(why) => println!("Error updating guild settings: {:?}", why),
    };

    // Get winner info
    let winner_user = match UserId::new(winner.user_id.parse::<u64>().unwrap_or_default())
        .to_user(&ctx)
        .await
    {
        Ok(user) => user,
        Err(why) => {
            println!("Error fetching user: {:?}", why);
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

#[tokio::main]
async fn main() {
    // Load environment variables
    dotenv::dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("Expected a discord token in the environment");

    // Connect to the database using a connection pool
    let database = SqlitePool::connect(&env::var("DATABASE_URL").unwrap())
        .await
        .expect("Coudn't connect to the sqlite database");

    // Run migrations
    match sqlx::query(
        "CREATE TABLE IF NOT EXISTS dicks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL,
            guild_id TEXT NOT NULL,
            length INTEGER NOT NULL DEFAULT 0,
            last_grow TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            dick_of_day_count INTEGER NOT NULL DEFAULT 0,
            pvp_wins INTEGER NOT NULL DEFAULT 0,
            pvp_losses INTEGER NOT NULL DEFAULT 0,
            pvp_max_streak INTEGER NOT NULL DEFAULT 0,
            pvp_current_streak INTEGER NOT NULL DEFAULT 0,
            cm_won INTEGER NOT NULL DEFAULT 0,
            cm_lost INTEGER NOT NULL DEFAULT 0,
            UNIQUE(user_id, guild_id)
        )",
    )
    .execute(&database)
    .await
    {
        Ok(_) => println!("Created dicks table"),
        Err(e) => panic!("Error creating dicks table: {}", e),
    };

    match sqlx::query(
        "CREATE TABLE IF NOT EXISTS guild_settings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            guild_id TEXT NOT NULL UNIQUE,
            last_dotd TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .execute(&database)
    .await
    {
        Ok(_) => println!("Created guild_settings table"),
        Err(e) => panic!("Error creating guild_settings table: {}", e),
    };

    // Initialize the bot
    let intents =
        GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT | GatewayIntents::GUILDS;

    let bot_data = Arc::new(Bot {
        database,
        pvp_challenges: RwLock::new(HashMap::new()),
    });

    let mut client = Client::builder(token, intents)
        .event_handler(Handler)
        .await
        .expect("Error creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<Bot>(bot_data);
    }

    // Start the bot
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}
