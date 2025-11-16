use crate::Bot;
use crate::commands::escape_markdown;
use crate::utils::get_bot_stats;
use crate::{GuildNameCache, GUILD_NAME_CACHE_DURATION};
use log::{error, info};
use rand::seq::IndexedRandom;
use serenity::all::{
    CommandInteraction, CreateEmbed, CreateEmbedFooter, CreateInteractionResponseFollowup,
};
use serenity::model::id::UserId;
use serenity::prelude::*;
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn handle_global_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> Result<(), serenity::Error> {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    // Defer the command to avoid timeout
    // This is important for commands that take a while to process
    command.defer(&ctx.http).await?;

    // Get top 10 users globally
    let top_users = match sqlx::query!(
        "SELECT user_id, length, guild_id FROM plants 
         ORDER BY length DESC LIMIT 10"
    )
    .fetch_all(&bot.database)
    .await
    {
        Ok(users) => users,
        Err(why) => {
            error!("Error fetching global top users: {:?}", why);
            command.create_followup(&ctx.http,
                CreateInteractionResponseFollowup::new().add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Global Leaderboard Error")
                        .description(
                            "Failed to measure all the world's plants. The greenhouse sensors are overwhelmed!",
                        )
                        .color(0xFF0000),
                ),
            ).await?;
            return Ok(());
        }
    };

    if top_users.is_empty() {
        command
            .create_followup(
                &ctx.http,
                CreateInteractionResponseFollowup::new().add_embed(
                    CreateEmbed::new()
                        .title("🌱 No Plants Found")
                        .description(
                            "No plants have been grown yet in any server. Be the first to grow one with /grow!",
                        )
                        .color(0xAAAAAA),
                ),
            )
            .await?;
        return Ok(());
    }

    // Fetch bot stats
    let (server_count_str, plant_count_str) = match get_bot_stats(ctx, bot).await {
        Ok(stats) => (stats.server_count.to_string(), stats.plant_count.to_string()),
        Err(why) => {
            error!("Error fetching bot stats for global command: {:?}", why);
            ("?".to_string(), "?".to_string()) // Use "?" on error
        }
    };

    // Build the global leaderboard
    let mut description = "Here are the biggest plants in the entire world:\n\n".to_string();

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
            Ok(user) => escape_markdown(&user.name),
            Err(_) => "Unknown User".to_string(),
        };

        let guild_name = match user.guild_id.parse::<u64>() {
            Ok(guild_id) => {
                let current_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                // Check cache first
                {
                    let cache = bot.guild_name_cache.read().await;
                    if let Some(cached) = cache.get(&guild_id) {
                        if current_time - cached.cached_at < GUILD_NAME_CACHE_DURATION {
                            info!("Using cached guild name for {}", cached.name);
                            cached.name.clone()
                        } else {
                            info!("Cache expired for guild {}, fetching new name", cached.name);
                            drop(cache); // Release read lock before acquiring write lock
                            
                            // Cache expired, fetch new name
                            match ctx.http.get_guild(guild_id.into()).await {
                                Ok(guild) => {
                                    let name = if guild.features.contains(&"COMMUNITY".to_string()) {
                                        escape_markdown(&guild.name)
                                    } else {
                                        "private server".to_string()
                                    };
                                    
                                    // Update cache
                                    let mut cache = bot.guild_name_cache.write().await;
                                    cache.insert(guild_id, GuildNameCache {
                                        name: name.clone(),
                                        cached_at: current_time,
                                    });
                                    
                                    name
                                }
                                Err(_) => "secret garden".to_string(),
                            }
                        }
                    } else {
                        info!("No cached guild name for {}, fetching new name", guild_id);
                        drop(cache); // Release read lock before acquiring write lock
                        
                        // Not in cache, fetch and cache
                        match ctx.http.get_guild(guild_id.into()).await {
                            Ok(guild) => {
                                let name = if guild.features.contains(&"COMMUNITY".to_string()) {
                                    escape_markdown(&guild.name)
                                } else {
                                    "private server".to_string()
                                };
                                
                                // Add to cache
                                let mut cache = bot.guild_name_cache.write().await;
                                cache.insert(guild_id, GuildNameCache {
                                    name: name.clone(),
                                    cached_at: current_time,
                                });
                                
                                name
                            }
                            Err(_) => "unknown server".to_string(),
                        }
                    }
                }
            }
            Err(_) => "unknown server".to_string(),
        };

        description.push_str(&format!(
            "{} **{}. {}**: {} cm (from {})\n",
            medal,
            i + 1,
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
            Ok(user) => escape_markdown(&user.name),
            Err(_) => "Unknown User".to_string(),
        };

        let comments = [
            format!(
                "NASA wants to study {}'s plant as a possible oxygen source for Mars!",
                winner_name
            ),
            format!(
                "{}'s plant is so big it needs its own greenhouse!",
                winner_name
            ),
            format!(
                "{} is making botanists worldwide jealous!",
                winner_name
            ),
            format!("{}'s plant is the envy of gardeners worldwide!", winner_name),
        ];

        // Select random comment
        let winner_comment = comments.choose(&mut rand::rng()).unwrap();

        description.push_str(&format!("\n\n{}", winner_comment));
    }

    command.create_followup(&ctx.http,
        CreateInteractionResponseFollowup::new().add_embed(
            CreateEmbed::new()
                    .title("🌿 Global Plant Growth Leaderboard 🌍")
                    .description(description)
                    .color(0x9B59B6) // Purple
                    .footer(CreateEmbedFooter::new(
                        format!(
                            "🌐 {} servers | � {} plants growing | Nurture your plant daily with /grow!",
                            server_count_str, plant_count_str
                    )
                )),
        ),
    ).await?;

    Ok(())
}
