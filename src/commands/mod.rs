pub mod daily;
pub mod dotd;
pub mod events;
pub mod gift;
pub mod global;
pub mod grow;
pub mod help;
pub mod pvp;
pub mod stats;
pub mod top;
pub mod viagra;

use crate::{Bot, llm};
use serenity::all::{
    CommandInteraction, Context, CreateCommand, CreateEmbed, CreateEmbedFooter,
    CreateInteractionResponse, CreateInteractionResponseMessage, EditInteractionResponse, GuildId,
    InteractionContext,
};
use std::fmt;

#[derive(Debug)]
pub enum Error {
    Database(sqlx::Error),
    // Boxed because serenity::Error is large enough to bloat every CommandResult.
    Discord(Box<serenity::Error>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Database(why) => write!(f, "database error: {why}"),
            Error::Discord(why) => write!(f, "discord error: {why}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<sqlx::Error> for Error {
    fn from(why: sqlx::Error) -> Self {
        Error::Database(why)
    }
}

impl From<serenity::Error> for Error {
    fn from(why: serenity::Error) -> Self {
        Error::Discord(Box::new(why))
    }
}

pub type CommandResult = Result<(), Error>;

/// A slash command invoked inside a guild, with the IDs every handler needs.
pub struct Cmd<'a> {
    pub ctx: &'a Context,
    pub bot: &'a Bot,
    pub interaction: &'a CommandInteraction,
    pub guild_id: GuildId,
    /// Invoking user's ID as stored in the database.
    pub user: String,
    /// Guild ID as stored in the database.
    pub guild: String,
}

impl Cmd<'_> {
    pub async fn respond(&self, message: CreateInteractionResponseMessage) -> CommandResult {
        self.interaction
            .create_response(&self.ctx.http, CreateInteractionResponse::Message(message))
            .await?;
        Ok(())
    }

    pub async fn reply(&self, embed: CreateEmbed) -> CommandResult {
        self.respond(CreateInteractionResponseMessage::new().embed(embed))
            .await
    }

    pub async fn reply_ephemeral(&self, embed: CreateEmbed) -> CommandResult {
        self.respond(
            CreateInteractionResponseMessage::new()
                .embed(embed)
                .ephemeral(true),
        )
        .await
    }

    pub async fn defer(&self) -> CommandResult {
        self.interaction.defer(&self.ctx.http).await?;
        Ok(())
    }

    /// Fills in a response previously started with [`Cmd::defer`].
    pub async fn edit(&self, embed: CreateEmbed) -> CommandResult {
        self.interaction
            .edit_response(&self.ctx.http, EditInteractionResponse::new().embed(embed))
            .await?;
        Ok(())
    }

    /// Replaces the footer of an embed that was already sent with a one-line LLM
    /// comment on `event`. Failures are only logged, so the original response
    /// stays as it was.
    pub async fn add_commentary(&self, embed: CreateEmbed, event: &str) {
        let Some(line) = llm::commentary(event).await else {
            return;
        };
        let edit = EditInteractionResponse::new().embed(embed.footer(CreateEmbedFooter::new(line)));
        if let Err(why) = self.interaction.edit_response(&self.ctx.http, edit).await {
            log::warn!(
                "Cannot add commentary to /{}: {why}",
                self.interaction.data.name
            );
        }
    }
}

pub async fn dispatch(cmd: &Cmd<'_>) -> CommandResult {
    match cmd.interaction.data.name.as_str() {
        "grow" => grow::run(cmd).await,
        "top" => top::run(cmd).await,
        "global" => global::run(cmd).await,
        "pvp" => pvp::run(cmd).await,
        "stats" => stats::run(cmd).await,
        "dickoftheday" => dotd::run(cmd).await,
        "help" => help::run(cmd).await,
        "gift" => gift::run(cmd).await,
        "viagra" => viagra::run(cmd).await,
        "daily" => daily::run(cmd).await,
        "event" => events::run(cmd).await,
        other => {
            log::warn!("Received unknown command /{other}");
            Ok(())
        }
    }
}

pub fn definitions() -> Vec<CreateCommand> {
    [
        grow::register(),
        top::register(),
        global::register(),
        pvp::register(),
        stats::register(),
        dotd::register(),
        help::register(),
        gift::register(),
        viagra::register(),
        daily::register(),
        events::register(),
    ]
    .into_iter()
    .map(|command| command.contexts(vec![InteractionContext::Guild]))
    .collect()
}
