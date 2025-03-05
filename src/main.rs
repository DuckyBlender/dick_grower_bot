use chrono::{Duration, Local, NaiveDateTime, Utc};
use rand::Rng;
use serenity::all::{
    CreateCommand, CreateEmbed, CreateInteractionResponse, CreateInteractionResponseMessage,
};
use serenity::async_trait;
use serenity::builder::CreateCommandOption;
use serenity::model::application::{CommandInteraction, CommandOptionType, Interaction};
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use sqlx::{migrate::MigrateDatabase, Connection, Sqlite, SqliteConnection};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::{self, sleep};

const DATABASE_URL: &str = "sqlite://dick_growth.db";
const ONE_DAY: u64 = 86400; // seconds in a day

struct Handler;

struct PvpRequest {
    challenger_id: u64,
    challenged_id: u64,
    bet: i32,
    created_at: u64,
}

struct Bot {
    database: RwLock<SqliteConnection>,
    pvp_requests: RwLock<HashMap<String, PvpRequest>>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            let content = match command.data.name.as_str() {
                "grow" => handle_grow_command(&ctx, &command).await,
                "top" => handle_top_command(&ctx, &command).await,
                "pvp" => handle_pvp_command(&ctx, &command).await,
                "accept" => handle_accept_command(&ctx, &command).await,
                "decline" => handle_decline_command(&ctx, &command).await,
                "stats" => handle_stats_command(&ctx, &command).await,
                _ => "Not implemented".to_string(),
            };

            if let Err(why) = command
                .create_response(&ctx.http, content)
                .await
            {
                println!("Cannot respond to slash command: {why}");
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        // Register commands
        let commands = vec![
            CreateCommand::new("grow")
                .description("Grow your cucumber daily"),
            CreateCommand::new("top")
                .description("Show the top players with the biggest weapons"),
            CreateCommand::new("pvp")
                .description("Challenge a user to a dick measuring contest")
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::User,
                        "user",
                        "The user you want to challenge",
                    )
                    .required(true),
                )
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::Integer,
                        "bet",
                        "The amount of cm you want to bet",
                    )
                    .required(true),
                ),
            CreateCommand::new("accept")
                .description("Accept a PvP challenge"),
            CreateCommand::new("decline")
                .description("Decline a PvP challenge"),
            CreateCommand::new("stats")
                .description("View your dick stats"),
        ];

        for guild in ready.guilds {
            if let Err(why) = serenity::builder::CreateApplicationCommands::set_global_commands(&ctx.http, commands.clone()).await {
                println!("Failed to set command for guild {}: {}", guild.id, why);
            }
        }

        // Start daily dick of the day election
        tokio::spawn(daily_dick_of_the_day(ctx.clone()));
    }
}

async fn daily_dick_of_the_day(ctx: Context) {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    loop {
        // Schedule for midnight
        let now = Local::now();
        let next_midnight = (now + Duration::days(1))
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let duration_until_midnight = next_midnight
            .signed_duration_since(now.naive_local())
            .to_std()
            .unwrap();

        sleep(duration_until_midnight).await;

        // Get all guilds
        let guilds = match ctx.http.get_guilds(None, None).await {
            Ok(guilds) => guilds,
            Err(why) => {
                println!("Error getting guilds: {:?}", why);
                continue;
            }
        };

        for guild in guilds {
            let mut conn = bot.database.write().await;
            
            // Get active users in the guild
            let active_users = match sqlx::query!(
                "SELECT user_id, length FROM dicks
                 WHERE guild_id = ?
                 AND last_grow > datetime('now', '-7 days')",
                guild.id.to_string()
            )
            .fetch_all(&mut *conn)
            .await {
                Ok(users) => users,
                Err(why) => {
                    println!("Error fetching active users: {:?}", why);
                    continue;
                }
            };

            if active_users.is_empty() {
                continue;
            }

            // Select a random winner
            let winner_idx = rand::thread_rng().gen_range(0..active_users.len());
            let winner = &active_users[winner_idx];
            
            // Award bonus (3-7 cm)
            let bonus = rand::thread_rng().gen_range(3..=7);
            
            // Update DB
            match sqlx::query!(
                "UPDATE dicks SET length = length + ?, dick_of_day_count = dick_of_day_count + 1
                 WHERE user_id = ? AND guild_id = ?",
                bonus,
                winner.user_id,
                guild.id.to_string()
            )
            .execute(&mut *conn)
            .await {
                Ok(_) => (),
                Err(why) => {
                    println!("Error updating winner: {:?}", why);
                    continue;
                }
            };
            
            // Announce the winner
            let winner_user = match ctx.http.get_user(winner.user_id.parse::<u64>().unwrap_or(0)).await {
                Ok(user) => user,
                Err(why) => {
                    println!("Error fetching user: {:?}", why);
                    continue;
                }
            };
            
            let default_channel = match guild.id.to_guild_cached(&ctx).await {
                Some(guild) => guild.system_channel_id.unwrap_or_else(|| guild.channels.keys().next().copied().unwrap()),
                None => continue,
            };
            
            let embed = CreateEmbed::new()
                .title("🏆 Dick of the Day! 🏆")
                .color(0xFFD700) // Gold
                .description(format!(
                    "Congratulations to **{}**! Their magnificent member has been crowned Dick of the Day!\n\n",
                    winner_user.name
                ))
                .field("Bonus Growth", format!("+{} cm", bonus), true)
                .field("New Total", format!("{} cm", winner.length + bonus), true)
                .thumbnail(winner_user.face())
                .footer(|f| f.text("May your schlong be long and strong!"));
                
            if let Err(why) = default_channel
                .send_message(&ctx.http, |m| m.set_embed(embed))
                .await
            {
                println!("Error sending Dick of the Day announcement: {:?}", why);
            }
        }
    }
}

async fn handle_grow_command(ctx: &Context, command: &CommandInteraction) -> CreateInteractionResponse {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();
    let mut conn = bot.database.write().await;
    
    let user_id = command.user.id.to_string();
    let guild_id = command.guild_id.unwrap().to_string();
    
    // Check if the user has grown today
    let last_grow = match sqlx::query!(
        "SELECT last_grow FROM dicks WHERE user_id = ? AND guild_id = ?",
        user_id, guild_id
    )
    .fetch_optional(&mut *conn)
    .await {
        Ok(Some(record)) => {
            let last_grow = NaiveDateTime::parse_from_str(&record.last_grow, "%Y-%m-%d %H:%M:%S").unwrap_or_default();
            let now = Utc::now().naive_utc();
            
            // If less than 24 hours have passed
            if now.signed_duration_since(last_grow).num_seconds() < ONE_DAY as i64 {
                let next_grow = last_grow + Duration::seconds(ONE_DAY as i64);
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
                                .footer(|f| f.text("Patience is a virtue... especially for your little buddy."))
                        )
                );
            }
            
            last_grow
        },
        Ok(None) => {
            // New user, create a record
            match sqlx::query!(
                "INSERT INTO dicks (user_id, guild_id, length, last_grow, dick_of_day_count)
                 VALUES (?, ?, 0, datetime('now'), 0)",
                user_id, guild_id
            )
            .execute(&mut *conn)
            .await {
                Ok(_) => (),
                Err(why) => println!("Error creating user: {:?}", why),
            };
            
            Utc::now().naive_utc()
        },
        Err(why) => {
            println!("Database error: {:?}", why);
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("⚠️ Database Error")
                            .description("Something went wrong with your dick growth. Maybe the universe is telling you something?")
                            .color(0xFF0000)
                    )
            );
        }
    };
    
    // Generate growth amount (-5 to 10 cm)
    let growth = rand::thread_rng().gen_range(-5..=10);
    
    // Update the database
    match sqlx::query!(
        "UPDATE dicks SET length = length + ?, last_grow = datetime('now')
         WHERE user_id = ? AND guild_id = ?",
        growth, user_id, guild_id
    )
    .execute(&mut *conn)
    .await {
        Ok(_) => (),
        Err(why) => {
            println!("Error updating length: {:?}", why);
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("⚠️ Growth Error")
                            .description("Your dick refused to cooperate with the database.")
                            .color(0xFF0000)
                    )
            );
        }
    };
    
    // Get new length
    let new_length = match sqlx::query!(
        "SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?",
        user_id, guild_id
    )
    .fetch_one(&mut *conn)
    .await {
        Ok(record) => record.length,
        Err(why) => {
            println!("Error fetching length: {:?}", why);
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("⚠️ Length Measurement Error")
                            .description("We couldn't measure your updated length. The measuring tape broke.")
                            .color(0xFF0000)
                    )
            );
        }
    };
    
    // Create response with funny messages based on growth
    let (title, description, color) = if growth > 7 {
        (
            "🚀 INCREDIBLE GROWTH!",
            format!("Holy moly! Your dick grew by **{} cm**!\nYour new length: **{} cm**\n\nThat's some supernatural growth! Are you using some kind of black magic?", growth, new_length),
            0x00FF00 // Bright green
        )
    } else if growth > 3 {
        (
            "🔥 Impressive Growth!",
            format!("Nice! Your dick grew by **{} cm**!\nYour new length: **{} cm**\n\nKeep it up, that's some serious growth!", growth, new_length),
            0x33FF33 // Green
        )
    } else if growth > 0 {
        (
            "🌱 Growth Achieved",
            format!("Your dick grew by **{} cm**.\nYour new length: **{} cm**\n\nSlow and steady wins the race, right?", growth, new_length),
            0x66FF66 // Light green
        )
    } else if growth == 0 {
        (
            "😐 No Change",
            format!("Your dick didn't grow at all today.\nYour length: **{} cm**\n\nMaybe try some positive affirmations?", new_length),
            0xFFFF33 // Yellow
        )
    } else if growth >= -3 {
        (
            "📉 Minor Shrinkage",
            format!("Uh oh! Your dick shrank by **{} cm**.\nYour new length: **{} cm**\n\nDid you take a cold shower?", -growth, new_length),
            0xFF9933 // Orange
        )
    } else {
        (
            "💀 CATASTROPHIC SHRINKAGE!",
            format!("DISASTER! Your dick shrank by **{} cm**!\nYour new length: **{} cm**\n\nWhatever you're doing, STOP IMMEDIATELY!", -growth, new_length),
            0xFF3333 // Red
        )
    };
    
    CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .title(title)
                    .description(description)
                    .color(color)
                    .footer(|f| f.text("Remember: it's not about the size, it's about... actually, it is about the size."))
            )
    )
}

async fn handle_top_command(ctx: &Context, command: &CommandInteraction) -> CreateInteractionResponse {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();
    let mut conn = bot.database.write().await;
    
    let guild_id = command.guild_id.unwrap().to_string();
    
    // Get top 10 users
    let top_users = match sqlx::query!(
        "SELECT user_id, length FROM dicks 
         WHERE guild_id = ? 
         ORDER BY length DESC LIMIT 10",
        guild_id
    )
    .fetch_all(&mut *conn)
    .await {
        Ok(users) => users,
        Err(why) => {
            println!("Error fetching top users: {:?}", why);
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("⚠️ Leaderboard Error")
                            .description("Failed to measure all the dicks. Some were too small to find.")
                            .color(0xFF0000)
                    )
            );
        }
    };
    
    if top_users.is_empty() {
        return CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("👀 No Dicks Found")
                        .description("Nobody has grown their dick in this server yet. Be the first one!")
                        .color(0xAAAAAA)
                )
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
        
        let username = match ctx.http.get_user(user.user_id.parse::<u64>().unwrap_or(0)).await {
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
            medal, i+1, emoji, username, user.length
        ));
    }
    
    // Add funny comment about the winner
    if !top_users.is_empty() {
        let winner_name = match ctx.http.get_user(top_users[0].user_id.parse::<u64>().unwrap_or(0)).await {
            Ok(user) => user.name,
            Err(_) => "Unknown User".to_string(),
        };
        
        let length = top_users[0].length;
        let winner_comment = if length > 50 {
            format!("Holy moly! {}' dick is so big it needs its own ZIP code!", winner_name)
        } else if length > 30 {
            format!("Beware of {} in tight spaces. That thing is a lethal weapon!", winner_name)
        } else if length > 15 {
            format!("{} is doing quite well. Impressive... most impressive.", winner_name)
        } else if length > 0 {
            format!("{} is trying their best, though. Gold star for effort!", winner_name)
        } else {
            format!("Poor {}... we need a microscope to find their dick.", winner_name)
        };
        
        description.push_str(&format!("\n\n{}", winner_comment));
    }
    
    CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .title("🍆 Dick Leaderboard 🏆")
                    .description(description)
                    .color(0x9B59B6) // Purple
                    .footer(|f| f.text("Use /grow daily to increase your length!"))
            )
    )
}

async fn handle_pvp_command(ctx: &Context, command: &CommandInteraction) -> CreateInteractionResponse {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();
    
    let challenger_id = command.user.id.0;
    let challenged_id = command.data.options[0].value.as_user_id().unwrap().0;
    let bet = command.data.options[1].value.as_i64().unwrap() as i32;
    
    // Validate bet
    if bet <= 0 {
        return CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("❌ Invalid Bet")
                        .description("You need to bet at least 1 cm! Don't be so stingy with your centimeters.")
                        .color(0xFF0000)
                )
        );
    }
    
    if challenger_id == challenged_id {
        return CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("🤨 Self-Challenge Detected")
                        .description("You can't challenge yourself! We know you're desperate to make it bigger, but that's not how it works.")
                        .color(0xFF9900)
                )
        );
    }
    
    // Check if challenger has enough length
    let mut conn = bot.database.write().await;
    let challenger_length = match sqlx::query!(
        "SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?",
        challenger_id.to_string(), command.guild_id.unwrap().to_string()
    )
    .fetch_optional(&mut *conn)
    .await {
        Ok(Some(record)) => record.length,
        Ok(None) => 0, // New user
        Err(why) => {
            println!("Database error: {:?}", why);
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("⚠️ Database Error")
                            .description("Failed to check your length. The measuring tape broke.")
                            .color(0xFF0000)
                    )
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
                        .color(0xFF0000)
                )
        );
    }
    
    // Check if challenged user exists
    let challenged_length = match sqlx::query!(
        "SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?",
        challenged_id.to_string(), command.guild_id.unwrap().to_string()
    )
    .fetch_optional(&mut *conn)
    .await {
        Ok(Some(record)) => record.length,
        Ok(None) => {
            // Auto-create the user
            match sqlx::query!(
                "INSERT INTO dicks (user_id, guild_id, length, last_grow, dick_of_day_count)
                 VALUES (?, ?, 0, datetime('now', '-2 days'), 0)",
                challenged_id.to_string(), command.guild_id.unwrap().to_string()
            )
            .execute(&mut *conn)
            .await {
                Ok(_) => 0,
                Err(why) => {
                    println!("Error creating user: {:?}", why);
                    return CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .add_embed(
                                CreateEmbed::new()
                                    .title("⚠️ Database Error")
                                    .description("Failed to create an account for the challenged user.")
                                    .color(0xFF0000)
                            )
                    );
                }
            }
        },
        Err(why) => {
            println!("Database error: {:?}", why);
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("⚠️ Database Error")
                            .description("Failed to check challenged user's length.")
                            .color(0xFF0000)
                    )
            );
        }
    };
    
    if challenged_length < bet {
        return CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("❌ Opponent Has Insufficient Length")
                        .description(format!(
                            "Your opponent only has **{} cm** but you're trying to bet **{} cm**!\n\nThey can't cover the bet. Pick on someone your own size... literally.",
                            challenged_length, bet
                        ))
                        .color(0xFF0000)
                )
        );
    }
    
    // Create PVP request
    let request_id = format!("{}:{}", challenger_id, challenged_id);
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let mut pvp_requests = bot.pvp_requests.write().await;
    pvp_requests.insert(
        request_id.clone(),
        PvpRequest {
            challenger_id,
            challenged_id,
            bet,
            created_at: current_time,
        },
    );
    
    // Get usernames
    let challenger = match ctx.http.get_user(challenger_id).await {
        Ok(user) => user.name,
        Err(_) => "Unknown User".to_string(),
    };
    
    let challenged = match ctx.http.get_user(challenged_id).await {
        Ok(user) => user.name,
        Err(_) => "Unknown User".to_string(),
    };
    
    CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .title("🥊 Dick Measuring Contest Challenge!")
                    .description(format!(
                        "**{}** has challenged **{}** to a dick measuring contest!\n\nBet amount: **{} cm**\n\nTo accept this challenge, the challenged user must use `/accept`\nTo decline, use `/decline`\n\nThe challenge will expire in 5 minutes.",
                        challenger, challenged, bet
                    ))
                    .color(0x3498DB) // Blue
                    .footer(|f| f.text("May the longest dong win!"))
            )
    )
}

async fn handle_accept_command(ctx: &Context, command: &CommandInteraction) -> CreateInteractionResponse {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();
    let user_id = command.user.id.0;
    
    // Check if there's a challenge for this user
    let mut pvp_requests = bot.pvp_requests.write().await;
    
    let request = pvp_requests.iter()
        .find(|(_, req)| req.challenged_id == user_id)
        .map(|(k, v)| (k.clone(), v.clone()));
    
    if let Some((request_id, request)) = request {
        // Check if the request is still valid (less than 5 minutes old)
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        if current_time - request.created_at > 300 { // 5 minutes
            pvp_requests.remove(&request_id);
            
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("⏰ Challenge Expired")
                            .description("This challenge has expired. You took too long to accept!")
                            .color(0xAAAAAA)
                    )
            );
        }
        
        // Remove the request
        pvp_requests.remove(&request_id);
        
        // Get usernames
        let challenger = match ctx.http.get_user(request.challenger_id).await {
            Ok(user) => user.name,
            Err(_) => "Unknown User".to_string(),
        };
        
        let challenged = match ctx.http.get_user(user_id).await {
            Ok(user) => user.name,
            Err(_) => "Unknown User".to_string(),
        };
        
        // Roll for both users
        let challenger_roll = rand::thread_rng().gen_range(1..=100);
        let challenged_roll = rand::thread_rng().gen_range(1..=100);
        
        let (winner_id, loser_id, winner_name, loser_name, winner_roll, loser_roll) = 
            if challenger_roll > challenged_roll {
                (request.challenger_id, request.challenged_id, challenger, challenged, challenger_roll, challenged_roll)
            } else {
                (request.challenged_id, request.challenger_id, challenged, challenger, challenged_roll, challenger_roll)
            };
        
        // Update the database
        let mut conn = bot.database.write().await;
        match sqlx::query!(
            "UPDATE dicks SET length = length + ? WHERE user_id = ? AND guild_id = ?",
            request.bet, winner_id.to_string(), command.guild_id.unwrap().to_string()
        )
        .execute(&mut *conn)
        .await {
            Ok(_) => (),
            Err(why) => println!("Error updating winner: {:?}", why),
        };
        
        match sqlx::query!(
            "UPDATE dicks SET length = length - ? WHERE user_id = ? AND guild_id = ?",
            request.bet, loser_id.to_string(), command.guild_id.unwrap().to_string()
        )
        .execute(&mut *conn)
        .await {
            Ok(_) => (),
            Err(why) => println!("Error updating loser: {:?}", why),
        };
        
        // Get updated lengths
        let winner_length = match sqlx::query!(
            "SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?",
            winner_id.to_string(), command.guild_id.unwrap().to_string()
        )
        .fetch_one(&mut *conn)
        .await {
            Ok(record) => record.length,
            Err(_) => 0,
        };
        
        let loser_length = match sqlx::query!(
            "SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?",
            loser_id.to_string(), command.guild_id.unwrap().to_string()
        )
        .fetch_one(&mut *conn)
        .await {
            Ok(record) => record.length,
            Err(_) => 0,
        };
        
        // Create a funny taunt
        let taunt = if winner_roll - loser_roll > 50 {
            format!("It wasn't even close! {}'s dick destroyed {}'s in an absolute massacre!", winner_name, loser_name)
        } else if winner_roll - loser_roll > 20 {
            format!("{}'s dick clearly outclassed {}'s in this epic showdown!", winner_name, loser_name)
        } else if winner_roll - loser_roll > 5 {
            format!("A close match, but {}'s dick had just enough extra length to claim victory!", winner_name, loser_name)
        } else {
            format!("That was incredibly close! {}'s dick barely edged out {}'s by a hair's width!", winner_name, loser_name)
        };
        
        CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("🏆 Dick Measuring Contest Results!")
                        .description(format!(
                            "The contest has concluded!\n\n**{}** rolled **{}**\n**{}** rolled **{}**\n\n**{}** wins **{} cm**!\n\nNew lengths:\n**{}**: {} cm\n**{}**: {} cm\n\n{}",
                            challenger, challenger_roll,
                            challenged, challenged_roll,
                            winner_name, request.bet,
                            winner_name, winner_length,
                            loser_name, loser_length,
                            taunt
                        ))
                        .color(0x2ECC71) // Green
                        .footer(|f| f.text("Size DOES matter after all!"))
                )
        )
    } else {
        CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("❓ No Active Challenges")
                        .description("You don't have any active challenges to accept. Maybe they expired, or you're just feeling overeager?")
                        .color(0xAAAAAA)
                )
        )
    }
}

async fn handle_decline_command(ctx: &Context, command: &CommandInteraction) -> CreateInteractionResponse {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();
    let user_id = command.user.id.0;
    
    // Check if there's a challenge for this user
    let mut pvp_requests = bot.pvp_requests.write().await;
    
    let request = pvp_requests.iter()
        .find(|(_, req)| req.challenged_id == user_id)
        .map(|(k, v)| (k.clone(), v.clone()));
    
    if let Some((request_id, request)) = request {
        // Remove the request
        pvp_requests.remove(&request_id);
        
        // Get usernames
        let challenger = match ctx.http.get_user(request.challenger_id).await {
            Ok(user) => user.name,
            Err(_) => "Unknown User".to_string(),
        };
        
        CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("🚫 Challenge Declined")
                        .description(format!(
                            "You've declined the dick measuring contest from **{}**.\n\nSmart move... or are you just scared?",
                            challenger
                        ))
                        .color(0xE74C3C) // Red
                        .footer(|f| f.text("Sometimes discretion is the better part of valor."))
                )
        )
    } else {
        CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("❓ No Active Challenges")
                        .description("You don't have any active challenges to decline. Are you practicing rejection in advance?")
                        .color(0xAAAAAA)
                )
        )
    }
}

async fn handle_stats_command(ctx: &Context, command: &CommandInteraction) -> CreateInteractionResponse {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();
    let mut conn = bot.database.write().await;
    
    let user_id = command.user.id.to_string();
    let guild_id = command.guild_id.unwrap().to_string();
    
    // Get user stats
    let user_stats = match sqlx::query!(
        "SELECT length, dick_of_day_count, last_grow FROM dicks 
         WHERE user_id = ? AND guild_id = ?",
        user_id, guild_id
    )
    .fetch_optional(&mut *conn)
    .await {
        Ok(Some(stats)) => stats,
        Ok(None) => {
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("❓ No Stats Found")
                            .description("You haven't started growing your dick yet! Use /grow to begin your journey to greatness.")
                            .color(0xAAAAAA)
                    )
            );
        },
        Err(why) => {
            println!("Database error: {:?}", why);
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("⚠️ Database Error")
                            .description("Failed to retrieve your stats. The server's ruler broke.")
                            .color(0xFF0000)
                    )
            );
        }
    };
    
    // Get rank
    let rank = match sqlx::query!(
        "SELECT COUNT(*) as rank FROM dicks 
         WHERE guild_id = ? AND length > (
            SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?
         )",
        guild_id, user_id, guild_id
    )
    .fetch_one(&mut *conn)
    .await {
        Ok(record) => record.rank + 1, // +1 because we're counting users with MORE length
        Err(why) => {
            println!("Error fetching rank: {:?}", why);
            0
        }
    };
    
    // Calculate next growth time
    let last_grow = NaiveDateTime::parse_from_str(&user_stats.last_grow, "%Y-%m-%d %H:%M:%S").unwrap_or_default();
    let now = Utc::now().naive_utc();
    let next_grow = last_grow + Duration::seconds(ONE_DAY as i64);
    
    let can_grow = now.signed_duration_since(last_grow).num_seconds() >= ONE_DAY as i64;
    let growth_status = if can_grow {
        "✅ You can grow now! Use /grow".to_string()
    } else {
        let time_left = next_grow.signed_duration_since(now);
        format!("⏰ Next growth in: {}h {}m", time_left.num_hours(), time_left.num_minutes() % 60)
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
                    .description(format!(
                        "Here's everything you wanted to know about your cucumber (and probably some things you didn't):"
                    ))
                    .color(0x9B59B6) // Purple
                    .field("Current Length", format!("**{} cm**", user_stats.length), true)
                    .field("Server Rank", format!("**#{}**", rank), true)
                    .field("Dick of the Day", format!("**{} time(s)**", user_stats.dick_of_day_count), true)
                    .field("Growth Status", growth_status, false)
                    .field("Professional Assessment", length_comment, false)
                    .footer(|f| f.text("Remember to /grow daily for maximum results!"))
            )
    )
}

#[tokio::main]
async fn main() {
    // Load environment variables
    dotenv::dotenv().ok();
    let token = std::env::var("DISCORD_TOKEN").expect("Expected a discord token in the environment");
    
    // Set up database
    if !Sqlite::database_exists(DATABASE_URL).await.unwrap_or(false) {
        match Sqlite::create_database(DATABASE_URL).await {
            Ok(_) => println!("Created database"),
            Err(e) => panic!("Error creating database: {}", e),
        }
    }
    
    let db = match SqliteConnection::connect(DATABASE_URL).await {
        Ok(db) => db,
        Err(e) => panic!("Error connecting to database: {}", e),
    };
    
    // Create table if it doesn't exist
    match sqlx::query(
        "CREATE TABLE IF NOT EXISTS dicks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL,
            guild_id TEXT NOT NULL,
            length INTEGER NOT NULL DEFAULT 0,
            last_grow TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            dick_of_day_count INTEGER NOT NULL DEFAULT 0,
            UNIQUE(user_id, guild_id)
        )"
    )
    .execute(&db)
    .await {
        Ok(_) => println!("Created table"),
        Err(e) => panic!("Error creating table: {}", e),
    }
    
    // Initialize the bot
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
    
    let mut client = Client::builder(token, intents)
        .event_handler(Handler)
        .await
        .expect("Error creating client");
    
    {
        let mut data = client.data.write().await;
        data.insert::<Bot>(Arc::new(Bot {
            database: RwLock::new(db),
            pvp_requests: RwLock::new(HashMap::new()),
        }));
    }
    
    // Start the bot
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}

#[derive(Debug, Clone)]
struct DailyTask;

impl TypeMapKey for Bot {
    type Value = Arc<Bot>;
}
