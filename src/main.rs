use chrono::Timelike;
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

pub struct Bot {
    pub database: Pool<Sqlite>,
    pub pvp_challenges: RwLock<HashMap<String, PvpChallenge>>,
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

async fn add_column_if_missing(
    database: &Pool<Sqlite>,
    table_name: &str,
    column_name: &str,
    column_definition: &str,
) -> Result<bool, sqlx::Error> {
    if !column_exists(database, table_name, column_name).await? {
        let query = format!("ALTER TABLE {table_name} ADD COLUMN {column_definition}");
        sqlx::query(&query).execute(database).await?;
        return Ok(true);
    }

    Ok(false)
}

async fn ensure_current_schema(database: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    if table_exists(database, "dicks").await? {
        add_column_if_missing(
            database,
            "dicks",
            "daily_last_claimed",
            "daily_last_claimed TEXT DEFAULT NULL",
        )
        .await?;
        add_column_if_missing(
            database,
            "dicks",
            "daily_growth_boost_percent",
            "daily_growth_boost_percent INTEGER NOT NULL DEFAULT 0",
        )
        .await?;
        add_column_if_missing(
            database,
            "dicks",
            "daily_cooldown_skips",
            "daily_cooldown_skips INTEGER NOT NULL DEFAULT 0",
        )
        .await?;
        add_column_if_missing(
            database,
            "dicks",
            "daily_streak_savers",
            "daily_streak_savers INTEGER NOT NULL DEFAULT 0",
        )
        .await?;
        add_column_if_missing(
            database,
            "dicks",
            "daily_lucky_rolls",
            "daily_lucky_rolls INTEGER NOT NULL DEFAULT 0",
        )
        .await?;
        add_column_if_missing(
            database,
            "dicks",
            "daily_streak",
            "daily_streak INTEGER NOT NULL DEFAULT 0",
        )
        .await?;
        add_column_if_missing(
            database,
            "dicks",
            "best_daily_streak",
            "best_daily_streak INTEGER NOT NULL DEFAULT 0",
        )
        .await?;
        add_column_if_missing(
            database,
            "dicks",
            "last_streak_date",
            "last_streak_date TEXT DEFAULT NULL",
        )
        .await?;
        add_column_if_missing(
            database,
            "dicks",
            "streak_last_claimed",
            "streak_last_claimed TEXT DEFAULT NULL",
        )
        .await?;
        add_column_if_missing(
            database,
            "dicks",
            "prestige_level",
            "prestige_level INTEGER NOT NULL DEFAULT 0",
        )
        .await?;
        add_column_if_missing(
            database,
            "dicks",
            "prestige_points",
            "prestige_points INTEGER NOT NULL DEFAULT 0",
        )
        .await?;
        let added_prestige_progress = add_column_if_missing(
            database,
            "dicks",
            "prestige_progress",
            "prestige_progress INTEGER NOT NULL DEFAULT 0",
        )
        .await?;
        if added_prestige_progress {
            sqlx::query(
                "UPDATE dicks
                 SET prestige_progress = CASE WHEN length > 0 THEN length ELSE 0 END",
            )
            .execute(database)
            .await?;
        }
    }

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS global_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            event_type TEXT NOT NULL,
            name TEXT NOT NULL,
            description TEXT NOT NULL,
            bonus_value INTEGER NOT NULL,
            pot_amount INTEGER NOT NULL DEFAULT 0,
            resolved_at TEXT DEFAULT NULL,
            started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            ends_at TEXT NOT NULL
        )",
    )
    .execute(database)
    .await?;

    add_column_if_missing(
        database,
        "global_events",
        "pot_amount",
        "pot_amount INTEGER NOT NULL DEFAULT 0",
    )
    .await?;
    add_column_if_missing(
        database,
        "global_events",
        "resolved_at",
        "resolved_at TEXT DEFAULT NULL",
    )
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_global_events_ends_at ON global_events(ends_at)")
        .execute(database)
        .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS seasons (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            season_number INTEGER NOT NULL UNIQUE,
            name TEXT NOT NULL,
            starts_at TEXT NOT NULL,
            ends_at TEXT NOT NULL,
            finalized_at TEXT DEFAULT NULL
        )",
    )
    .execute(database)
    .await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS season_scores (
            season_id INTEGER NOT NULL,
            user_id TEXT NOT NULL,
            guild_id TEXT NOT NULL,
            score INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (season_id, user_id, guild_id),
            FOREIGN KEY (season_id) REFERENCES seasons(id)
        )",
    )
    .execute(database)
    .await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS season_placements (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            season_id INTEGER NOT NULL,
            scope TEXT NOT NULL CHECK (scope IN ('server', 'global')),
            guild_id TEXT NOT NULL DEFAULT '',
            profile_guild_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            position INTEGER NOT NULL CHECK (position BETWEEN 1 AND 3),
            score INTEGER NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE (season_id, scope, guild_id, position),
            FOREIGN KEY (season_id) REFERENCES seasons(id)
        )",
    )
    .execute(database)
    .await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS prestige_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL,
            guild_id TEXT NOT NULL,
            prestige_level INTEGER NOT NULL,
            points_earned INTEGER NOT NULL,
            length_before_reset INTEGER NOT NULL,
            progress_before_reset INTEGER NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .execute(database)
    .await?;

    for index in [
        "CREATE INDEX IF NOT EXISTS idx_dicks_global_leaderboard ON dicks(length DESC, user_id ASC, guild_id ASC)",
        "CREATE INDEX IF NOT EXISTS idx_dicks_guild_leaderboard ON dicks(guild_id, length DESC, user_id ASC)",
        "CREATE INDEX IF NOT EXISTS idx_season_scores_global ON season_scores(season_id, score DESC, user_id ASC, guild_id ASC)",
        "CREATE INDEX IF NOT EXISTS idx_season_scores_guild ON season_scores(season_id, guild_id, score DESC, user_id ASC)",
        "CREATE INDEX IF NOT EXISTS idx_season_placements_profile ON season_placements(user_id, profile_guild_id, season_id DESC)",
        "CREATE INDEX IF NOT EXISTS idx_prestige_history_profile ON prestige_history(user_id, guild_id, created_at DESC)",
    ] {
        sqlx::query(index).execute(database).await?;
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
                    "season" => handle_season_command(&ctx, &command).await,
                    "prestige" => handle_prestige_command(&ctx, &command).await,
                    "pvp" => handle_pvp_command(&ctx, &command).await,
                    "stats" => handle_stats_command(&ctx, &command).await,
                    "dickoftheday" => handle_dotd_command(&ctx, &command).await,
                    "help" => handle_help_command(&ctx, &command).await,
                    "gift" => handle_gift_command(&ctx, &command).await,
                    "viagra" => handle_viagra_command(&ctx, &command).await,
                    "daily" => handle_daily_command(&ctx, &command).await,
                    "event" => handle_event_command(&ctx, &command).await,
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
            Interaction::Component(component)
                if component.data.custom_id.starts_with("prestige_confirm:")
                    || component.data.custom_id.starts_with("prestige_cancel:") =>
            {
                info!(
                    "Prestige component interaction: {}",
                    component.data.custom_id
                );
                if let Err(why) = handle_prestige_component(&ctx, &component).await {
                    error!("Error handling prestige component: {}", why);
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

        // Finalize ended seasons and create the next one at its UTC boundary.
        let ctx_clone = ctx.clone();
        tokio::spawn(async move {
            loop {
                let bot = {
                    let data = ctx_clone.data.read().await;
                    data.get::<Bot>().cloned()
                };
                let Some(bot) = bot else {
                    tokio::time::sleep(StdDuration::from_secs(60)).await;
                    continue;
                };
                match ensure_active_season(&bot.database, chrono::Utc::now().naive_utc()).await {
                    Ok(season) => {
                        let seconds = (season.ends_at - chrono::Utc::now().naive_utc())
                            .num_seconds()
                            .max(1) as u64;
                        tokio::time::sleep(StdDuration::from_secs(seconds)).await;
                    }
                    Err(why) => {
                        error!("Season rollover failed: {:?}", why);
                        tokio::time::sleep(StdDuration::from_secs(60)).await;
                    }
                }
            }
        });

        // Start a task to automatically rotate global events every 4 hours on UTC boundaries
        let ctx_clone = ctx.clone();
        tokio::spawn(async move {
            let now = chrono::Utc::now();
            let seconds_into_period =
                (now.hour() % 4) as u64 * 3600 + now.minute() as u64 * 60 + now.second() as u64;
            let seconds_until_next = 4 * 3600 - seconds_into_period;

            tokio::time::sleep(StdDuration::from_secs(seconds_until_next)).await;
            let mut interval = tokio::time::interval(StdDuration::from_secs(4 * 3600));

            loop {
                interval.tick().await;

                let data = ctx_clone.data.read().await;
                let bot = data.get::<Bot>().unwrap().clone();
                drop(data);

                let messages = tick_event_system(&bot).await;
                for msg in messages {
                    info!("Event system: {}", msg);
                }
            }
        });

        // Register commands globally
        let commands = vec![
            CreateCommand::new("grow").description("Grow your cucumber"),
            CreateCommand::new("top")
                .description("Show the top players with the biggest weapons in this server"),
            CreateCommand::new("global")
                .description("Show the top players with the biggest weapons across all servers"),
            CreateCommand::new("season")
                .description("View the current or previous seasonal leaderboard")
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "scope",
                        "Choose the server or global season leaderboard",
                    )
                    .required(false)
                    .add_string_choice("Server", "server")
                    .add_string_choice("Global", "global"),
                )
                .add_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "period",
                        "Choose the current or previous season",
                    )
                    .required(false)
                    .add_string_choice("Current", "current")
                    .add_string_choice("Previous", "previous"),
                ),
            CreateCommand::new("prestige")
                .description("Reset earned progress for a permanent /grow bonus"),
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
            CreateCommand::new("daily").description("Claim a once-a-day random perk"),
            CreateCommand::new("event").description("View the current global growth event"),
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

    ensure_current_schema(&database)
        .await
        .expect("Failed to ensure current database schema");
    ensure_active_season(&database, chrono::Utc::now().naive_utc())
        .await
        .expect("Failed to initialize the active season");

    // Initialize the bot
    let intents = GatewayIntents::GUILDS;
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
        error!("An error occurred while running the client: {:?}", why);
    }
}

#[cfg(test)]
mod schema_tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn prestige_progress_backfills_once_from_existing_length() {
        let database = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE dicks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id TEXT NOT NULL, guild_id TEXT NOT NULL,
                length INTEGER NOT NULL DEFAULT 0,
                UNIQUE(user_id, guild_id)
            )",
        )
        .execute(&database)
        .await
        .unwrap();
        sqlx::query("INSERT INTO dicks (user_id, guild_id, length) VALUES ('u1', 'g1', 700)")
            .execute(&database)
            .await
            .unwrap();

        ensure_current_schema(&database).await.unwrap();
        let backfilled = sqlx::query_scalar::<_, i64>(
            "SELECT prestige_progress FROM dicks WHERE user_id = 'u1'",
        )
        .fetch_one(&database)
        .await
        .unwrap();
        assert_eq!(backfilled, 700);

        sqlx::query("UPDATE dicks SET prestige_progress = 12 WHERE user_id = 'u1'")
            .execute(&database)
            .await
            .unwrap();
        ensure_current_schema(&database).await.unwrap();
        let unchanged = sqlx::query_scalar::<_, i64>(
            "SELECT prestige_progress FROM dicks WHERE user_id = 'u1'",
        )
        .fetch_one(&database)
        .await
        .unwrap();
        assert_eq!(unchanged, 12);
    }
}
