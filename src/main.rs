use commands::*;
use fern::colors::{Color, ColoredLevelConfig};
use log::{LevelFilter, error, info};
use presence::update_presence;
use serenity::all::{
    CreateCommand, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use serenity::async_trait;
use serenity::builder::CreateCommandOption;
use serenity::model::application::{CommandOptionType, Interaction};
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use sqlx::Row;
use sqlx::SqlitePool;
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tokio::sync::RwLock;
use tokio::time::Instant;
mod commands;
mod presence;
mod time;
mod utils;

struct Handler;

impl TypeMapKey for Bot {
    type Value = Arc<Bot>;
}

// Guild name cache duration in seconds (12 hours)
const GUILD_NAME_CACHE_DURATION: u64 = 12 * 60 * 60;

#[derive(Clone)]
pub struct GuildNameCache {
    pub name: String,
    pub cached_at: u64,
}

pub struct Bot {
    pub database: Pool<Sqlite>,
    pub pvp_challenges: RwLock<HashMap<String, PvpChallenge>>,
    pub guild_name_cache: RwLock<HashMap<u64, GuildNameCache>>,
}

async fn table_exists(database: &Pool<Sqlite>, table_name: &str) -> Result<bool, sqlx::Error> {
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?)",
    )
    .bind(table_name)
    .fetch_one(database)
    .await?;

    Ok(exists == 1)
}

async fn column_exists(
    database: &Pool<Sqlite>,
    table_name: &str,
    column_name: &str,
) -> Result<bool, sqlx::Error> {
    let pragma = format!("PRAGMA table_info({table_name})");
    let rows = sqlx::query(&pragma).fetch_all(database).await?;

    Ok(rows
        .iter()
        .any(|row| row.get::<String, _>("name") == column_name))
}

async fn run_legacy_schema_migrations(database: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    if table_exists(database, "schlongs").await? && !table_exists(database, "dicks").await? {
        info!("Renaming legacy schlongs table to dicks");
        sqlx::query("ALTER TABLE schlongs RENAME TO dicks")
            .execute(database)
            .await?;
    }

    if table_exists(database, "guild_schlong_settings").await?
        && !table_exists(database, "guild_settings").await?
    {
        info!("Renaming legacy guild_schlong_settings table to guild_settings");
        sqlx::query("ALTER TABLE guild_schlong_settings RENAME TO guild_settings")
            .execute(database)
            .await?;
    }

    if table_exists(database, "dicks").await? {
        let has_legacy_count = column_exists(database, "dicks", "schlong_of_day_count").await?;
        let has_current_count = column_exists(database, "dicks", "dick_of_day_count").await?;

        if has_legacy_count && !has_current_count {
            info!("Renaming legacy schlong_of_day_count column to dick_of_day_count");
            sqlx::query(
                "ALTER TABLE dicks RENAME COLUMN schlong_of_day_count TO dick_of_day_count",
            )
            .execute(database)
            .await?;
        } else if has_legacy_count && has_current_count {
            sqlx::query(
                "UPDATE dicks
                 SET dick_of_day_count = schlong_of_day_count
                 WHERE dick_of_day_count = 0 AND schlong_of_day_count > 0",
            )
            .execute(database)
            .await?;
        }
    }

    if table_exists(database, "guild_settings").await? {
        let has_legacy_dotd = column_exists(database, "guild_settings", "last_sotd").await?;
        let has_current_dotd = column_exists(database, "guild_settings", "last_dotd").await?;

        if has_legacy_dotd && !has_current_dotd {
            info!("Renaming legacy last_sotd column to last_dotd");
            sqlx::query("ALTER TABLE guild_settings RENAME COLUMN last_sotd TO last_dotd")
                .execute(database)
                .await?;
        } else if has_legacy_dotd && has_current_dotd {
            sqlx::query(
                "UPDATE guild_settings
                 SET last_dotd = last_sotd
                 WHERE last_dotd IS NULL OR last_dotd = ''",
            )
            .execute(database)
            .await?;
        }
    }

    if table_exists(database, "length_history").await?
        && column_exists(database, "length_history", "growth_type").await?
    {
        sqlx::query(
            "UPDATE length_history
             SET growth_type = 'dotd'
             WHERE growth_type IN ('sotd', 'schlong_of_day', 'schlongoftheday')",
        )
        .execute(database)
        .await?;
    }

    Ok(())
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(command) => {
                // Log command invocation

                if command.guild_id.is_none() {
                    // Return message notifying that the bot is only available in guilds
                    info!(
                        "Command invoked in DM: /{} by {} (ID: {})",
                        command.data.name, command.user.name, command.user.id
                    );
                    // Respond with an ephemeral message
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
                        error!("Cannot respond to slash command for guild check: {}", why);
                    }
                    return;
                }

                info!(
                    "Command invoked: /{} by {} (ID: {}) in guild {}",
                    command.data.name,
                    command.user.name,
                    command.user.id,
                    command.guild_id.unwrap_or_default()
                );

                // Execute the command directly
                let now = Instant::now();
                let result = match command.data.name.as_str() {
                    "grow" => handle_grow_command(&ctx, &command).await,
                    "top" => handle_top_command(&ctx, &command).await,
                    "global" => handle_global_command(&ctx, &command).await,
                    "pvp" => handle_pvp_command(&ctx, &command).await,
                    "stats" => handle_stats_command(&ctx, &command).await,
                    "dickoftheday" => handle_dotd_command(&ctx, &command).await,
                    "help" => handle_help_command(&ctx, &command).await,
                    "gift" => handle_gift_command(&ctx, &command).await,
                    "viagra" => handle_viagra_command(&ctx, &command).await,
                    _ => {
                        // For unimplemented commands, respond directly here
                        command
                            .create_response(
                                &ctx.http,
                                CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new()
                                        .content("Not implemented")
                                        .ephemeral(true),
                                ),
                            )
                            .await
                    }
                };

                if let Err(why) = result {
                    error!("Error executing command {}: {}", command.data.name, why);
                }

                let elapsed = now.elapsed();
                info!(
                    "Command /{} executed in {} ms",
                    command.data.name,
                    elapsed.as_millis()
                );
            }
            Interaction::Component(component)
                if component.data.custom_id.starts_with("pvp_accept:") =>
            {
                // Handle button interactions
                info!("Component interaction: {}", component.data.custom_id);
                if let Err(why) = handle_pvp_accept(&ctx, &component).await {
                    error!("Error handling PVP accept: {}", why);
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
                        error!("Error responding to component interaction: {}", e);
                    }
                }
            }
            _ => {}
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        // Start a task to periodically update the presence
        let ctx_clone = ctx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(StdDuration::from_secs(300)); // Update every 5 minutes

            loop {
                // Wait for the next interval
                interval.tick().await;

                // Update presence
                update_presence(&ctx_clone).await;
            }
        });

        // Register commands globally
        let commands = vec![
            CreateCommand::new("grow").description("Grow your cucumber"),
            CreateCommand::new("top")
                .description("Show the top players with the biggest weapons in this server"),
            CreateCommand::new("global")
                .description("Show the top players with the biggest weapons across all servers"),
            CreateCommand::new("pvp")
                .description("Start a dick battle")
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::Integer,
                        "bet",
                        "The amount of cm you want to bet",
                    )
                    .required(true)
                    .min_int_value(1),
                ),
            CreateCommand::new("stats")
                .description("View your or another user's stats")
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::User,
                        "user",
                        "The user whose stats you want to view",
                    )
                    .required(false),
                ),
            CreateCommand::new("dickoftheday").description("Randomly select a Dick of the Day"),
            CreateCommand::new("help").description("Show help information about the bot commands"),
            CreateCommand::new("gift")
                .description("Gift some of your length to another user")
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::User,
                        "user",
                        "The user you want to gift length to",
                    )
                    .required(true),
                )
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::Integer,
                        "amount",
                        "The amount of cm you want to gift",
                    )
                    .required(true)
                    .min_int_value(1),
                ),
            CreateCommand::new("viagra")
                .description("Boost your growth by 20% for 6 hours (20 hour cooldown)"),
        ];

        if let Err(why) = ctx.http.create_global_commands(&commands).await {
            error!("Error creating global commands: {}", why);
        }
    }
}

#[tokio::main]
async fn main() {
    // Initialize logger
    let colors_line = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        .info(Color::Green)
        .debug(Color::BrightCyan)
        .trace(Color::BrightBlack);

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "[{} {} {}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                colors_line.color(record.level()),
                record.target(),
                message
            ))
        })
        .level(LevelFilter::Warn)
        .level_for(env!("CARGO_PKG_NAME"), LevelFilter::Info)
        .chain(std::io::stdout())
        .apply()
        .expect("Failed to initialize logger");

    // Load environment variables
    dotenv::dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("Expected a discord token in the environment");

    // Connect to the database using a connection pool
    let database = SqlitePool::connect(&env::var("DATABASE_URL").unwrap())
        .await
        .expect("Coudn't connect to the sqlite database");

    run_legacy_schema_migrations(&database)
        .await
        .expect("Failed to migrate legacy database schema");

    // Initialize the bot
    let intents = GatewayIntents::GUILDS;

    let bot_data = Arc::new(Bot {
        database,
        pvp_challenges: RwLock::new(HashMap::new()),
        guild_name_cache: RwLock::new(HashMap::new()),
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
        error!("An error occurred while running the client: {:?}", why);
    }
}
