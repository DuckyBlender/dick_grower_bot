use crate::commands::{Cmd, CommandResult};
use crate::db::{self, History};
use crate::utils::{colors, embed};
use serenity::all::{
    CommandOptionType, CreateCommand, CreateCommandOption, CreateEmbedFooter,
    CreateInteractionResponseMessage, Mentionable, ResolvedValue,
};

pub fn register() -> CreateCommand {
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
        )
}

pub async fn run(cmd: &Cmd<'_>) -> CommandResult {
    let mut recipient = None;
    let mut amount = 0;
    for option in cmd.interaction.data.options() {
        match option.value {
            ResolvedValue::User(user, _) => recipient = Some(user),
            ResolvedValue::Integer(value) => amount = value,
            _ => {}
        }
    }
    let Some(recipient) = recipient else {
        return Ok(());
    };

    let rejection = if amount < 1 {
        Some((
            "❌ Invalid Gift Amount",
            "You need to gift at least 1 cm! Don't be so stingy with your length.",
        ))
    } else if recipient.id == cmd.interaction.user.id {
        Some((
            "🤨 Self-Gift Detected",
            "You can't gift yourself! That would defeat the purpose of generosity.",
        ))
    } else if recipient.bot {
        Some((
            "🤖 Bots Don't Grow",
            "Bots have no use for your centimeters. Gift them to a human instead!",
        ))
    } else {
        None
    };
    if let Some((title, description)) = rejection {
        return cmd
            .reply_ephemeral(embed(title, description, colors::WARNING))
            .await;
    }

    let db = &cmd.bot.db;
    let recipient_id = recipient.id.to_string();
    db::ensure_user(db, &cmd.user, &cmd.guild).await?;
    db::ensure_user(db, &recipient_id, &cmd.guild).await?;

    let mut tx = db.begin().await?;
    // The balance check is part of the UPDATE so concurrent gifts can't overdraw.
    let giver_length = sqlx::query_scalar!(
        "UPDATE dicks SET length = length - ?
         WHERE user_id = ? AND guild_id = ? AND length >= ?
         RETURNING length",
        amount,
        cmd.user,
        cmd.guild,
        amount
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some(giver_length) = giver_length else {
        drop(tx);
        let length = db::length(db, &cmd.user, &cmd.guild).await?;
        return cmd
            .reply_ephemeral(embed(
                "❌ Insufficient Length",
                format!(
                    "You only have **{length} cm** but you're trying to gift **{amount} cm**!\n\nYou can't give what you don't have. Grow more first!"
                ),
                colors::ERROR,
            ))
            .await;
    };
    let recipient_length = sqlx::query_scalar!(
        "UPDATE dicks SET length = length + ? WHERE user_id = ? AND guild_id = ? RETURNING length",
        amount,
        recipient_id,
        cmd.guild
    )
    .fetch_one(&mut *tx)
    .await?;
    db::log_history(
        &mut *tx,
        &cmd.user,
        &cmd.guild,
        giver_length,
        -amount,
        History::GiftSent,
    )
    .await?;
    db::log_history(
        &mut *tx,
        &recipient_id,
        &cmd.guild,
        recipient_length,
        amount,
        History::GiftReceived,
    )
    .await?;
    tx.commit().await?;

    let comment = match amount {
        50.. => {
            "What an incredibly generous donation! This kind of philanthropy will go down in history!"
        }
        25.. => "That's a substantial gift! Your generosity knows no bounds!",
        10.. => "A respectable gift! The recipient will surely appreciate your kindness.",
        5.. => "A modest but thoughtful gift. Every centimeter counts!",
        _ => "A small token of appreciation. It's the thought that counts... right?",
    };
    let giver = cmd.interaction.user.mention();
    let recipient = recipient.mention();

    cmd.respond(
        CreateInteractionResponseMessage::new()
            .content(recipient.to_string())
            .embed(
                embed(
                    "🎁 Gift Sent!",
                    format!(
                        "{giver} has generously gifted **{amount} cm** to {recipient}!\n\n{comment}"
                    ),
                    colors::SUCCESS,
                )
                .field(
                    "📏 New Lengths",
                    format!(
                        "{giver}: **{giver_length} cm**\n{recipient}: **{recipient_length} cm**"
                    ),
                    false,
                )
                .footer(CreateEmbedFooter::new(
                    "Sharing is caring! Spread the love (and the length)!",
                )),
            ),
    )
    .await
}
