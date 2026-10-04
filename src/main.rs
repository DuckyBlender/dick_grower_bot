mod commands;
mod db;
mod time;
mod utils;

use commands::pvp::{self, PvpChallenge};
use commands::{Cmd, events};
use fern::colors::{Color, ColoredLevelConfig};
use log::{LevelFilter, error, info};
use serenity::all::{
    CommandInteraction, Context, CreateInteractionResponse, CreateInteractionResponseFollowup,
    CreateInteractionResponseMessage, EventHandler, GatewayIntents, Interaction, Ready,
};
use serenity::{Client, async_trait};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tokio::sync::RwLock;
use utils::error_embed;

pub struct Bot {
    pub db: SqlitePool,
    /// Open PvP challenges keyed by the ID of the interaction that created them.
    pub pvp_challenges: RwLock<HashMap<u64, PvpChallenge>>,
}

struct Handler {
    bot: Arc<Bot>,
    /// `ready` fires again after every reconnect; background tasks must only start once.
    started: AtomicBool,
}

impl Handler {
    async fn handle_command(&self, ctx: &Context, command: &CommandInteraction) {
        let Some(guild_id) = command.guild_id else {
            // Commands are registered as guild-only; this only triggers for stale clients.
            let response = CreateInteractionResponseMessage::new()
                .embed(error_embed(
                    "⚠️ Server Only Bot",
                    "This bot can only be used in a server, not in direct messages.",
                ))
                .ephemeral(true);
            if let Err(why) = command
                .create_response(&ctx.http, CreateInteractionResponse::Message(response))
                .await
            {
                error!("Cannot respond to DM command: {why}");
            }
            return;
        };

        info!(
            "Command invoked: /{} by {} (ID: {}) in guild {}",
            command.data.name, command.user.name, command.user.id, guild_id
        );
        let started = Instant::now();
        let cmd = Cmd {
            ctx,
            bot: &self.bot,
            interaction: command,
            guild_id,
            user: command.user.id.to_string(),
            guild: guild_id.to_string(),
        };

        if let Err(why) = commands::dispatch(&cmd).await {
            error!("Error executing /{}: {why}", command.data.name);
            let embed = error_embed(
                "⚠️ Something Went Wrong",
                "The measuring tape broke. Please try again in a moment.",
            );
            let response = CreateInteractionResponseMessage::new()
                .embed(embed.clone())
                .ephemeral(true);
            // If the command already responded (or deferred), fall back to a follow-up.
            if command
                .create_response(&ctx.http, CreateInteractionResponse::Message(response))
                .await
                .is_err()
            {
                let followup = CreateInteractionResponseFollowup::new()
                    .embed(embed)
                    .ephemeral(true);
                if let Err(why) = command.create_followup(&ctx.http, followup).await {
                    error!("Cannot report command error to user: {why}");
                }
            }
        }

        info!(
            "Command /{} executed in {} ms",
            command.data.name,
            started.elapsed().as_millis()
        );
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(command) => self.handle_command(&ctx, &command).await,
            Interaction::Component(component) => {
                info!(
                    "Component interaction: {} by {}",
                    component.data.custom_id, component.user.id
                );
                if let Err(why) = pvp::handle_component(&ctx, &self.bot, &component).await {
                    error!(
                        "Error handling component {}: {why}",
                        component.data.custom_id
                    );
                    let response = CreateInteractionResponseMessage::new()
                        .content("Something went wrong processing your request")
                        .ephemeral(true);
                    if let Err(why) = component
                        .create_response(&ctx.http, CreateInteractionResponse::Message(response))
                        .await
                    {
                        error!("Error responding to component interaction: {why}");
                    }
                }
            }
            _ => {}
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
        events::update_presence(&ctx, &self.bot).await;

        if self.started.swap(true, Ordering::SeqCst) {
            return;
        }

        tokio::spawn(events::run_scheduler(ctx.clone(), Arc::clone(&self.bot)));

        if let Err(why) = ctx
            .http
            .create_global_commands(&commands::definitions())
            .await
        {
            error!("Error creating global commands: {why}");
        }
    }
}

fn init_logger() {
    let colors = ColoredLevelConfig::new()
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
                colors.color(record.level()),
                record.target(),
                message
            ))
        })
        .level(LevelFilter::Warn)
        .level_for(env!("CARGO_PKG_NAME"), LevelFilter::Info)
        .chain(std::io::stdout())
        .apply()
        .expect("Failed to initialize logger");
}

#[tokio::main]
async fn main() {
    init_logger();
    dotenvy::dotenv().ok();

    let token = std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN must be set");
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let db = SqlitePool::connect(&database_url)
        .await
        .expect("Couldn't connect to the SQLite database");
    db::ensure_schema(&db)
        .await
        .expect("Failed to ensure current database schema");

    let handler = Handler {
        bot: Arc::new(Bot {
            db,
            pvp_challenges: RwLock::new(HashMap::new()),
        }),
        started: AtomicBool::new(false),
    };

    let mut client = Client::builder(token, GatewayIntents::GUILDS)
        .event_handler(handler)
        .await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        error!("An error occurred while running the client: {why:?}");
    }
}
