use crate::commands::{Cmd, CommandResult};
use crate::db::{self, History};
use crate::time;
use crate::utils::{colors, embed};
use chrono::Duration;
use serenity::all::{
    CommandOptionType, CreateCommand, CreateCommandOption, CreateEmbedFooter,
    CreateInteractionResponseMessage, Mentionable, ResolvedValue,
};
use sqlx::SqliteConnection;

/// Most cm a user can send, and separately receive, within the rolling window.
/// Capping both stops alt accounts from funnelling their growth into one main account.
pub const GIFT_LIMIT_CM: i64 = 50;
const GIFT_WINDOW_DAYS: i64 = 7;

struct GiftUsage {
    total: i64,
    /// When the oldest gift in the window ages out and frees up allowance.
    frees_up: Option<chrono::NaiveDateTime>,
}

impl GiftUsage {
    fn remaining(&self) -> i64 {
        (GIFT_LIMIT_CM - self.total).max(0)
    }

    fn frees_up_text(&self) -> String {
        self.frees_up
            .map(|time| format!(" More allowance frees up {}.", time::relative(time)))
            .unwrap_or_default()
    }
}

async fn gift_usage(
    conn: &mut SqliteConnection,
    user_id: &str,
    guild_id: &str,
    kind: History,
) -> sqlx::Result<GiftUsage> {
    let kind = kind.as_str();
    let window = format!("-{GIFT_WINDOW_DAYS} days");
    let row = sqlx::query!(
        r#"SELECT COALESCE(SUM(ABS(growth_amount)), 0) as "total!: i64", MIN(timestamp) as oldest
         FROM length_history
         WHERE user_id = ? AND guild_id = ? AND growth_type = ? AND timestamp > datetime('now', ?)"#,
        user_id,
        guild_id,
        kind,
        window
    )
    .fetch_one(conn)
    .await?;

    Ok(GiftUsage {
        total: row.total,
        frees_up: row
            .oldest
            .as_deref()
            .and_then(time::parse)
            .map(|oldest| oldest + Duration::days(GIFT_WINDOW_DAYS)),
    })
}

pub fn register() -> CreateCommand {
    CreateCommand::new("gift")
        .description(format!(
            "Gift some of your length to another user (max {GIFT_LIMIT_CM} cm per {GIFT_WINDOW_DAYS} days)"
        ))
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
    // The UPDATE above holds SQLite's write lock, so these limits can't race other gifts.
    let sent = gift_usage(&mut tx, &cmd.user, &cmd.guild, History::GiftSent).await?;
    if sent.total + amount > GIFT_LIMIT_CM {
        return cmd
            .reply_ephemeral(embed(
                "🛑 Weekly Gift Limit Reached",
                format!(
                    "You've already gifted **{}/{GIFT_LIMIT_CM} cm** in the last {GIFT_WINDOW_DAYS} days, so you can only gift **{} cm** right now.{}",
                    sent.total,
                    sent.remaining(),
                    sent.frees_up_text()
                ),
                colors::WARNING,
            ))
            .await;
    }
    let received = gift_usage(&mut tx, &recipient_id, &cmd.guild, History::GiftReceived).await?;
    if received.total + amount > GIFT_LIMIT_CM {
        return cmd
            .reply_ephemeral(embed(
                "🛑 Recipient Gift Limit Reached",
                format!(
                    "{} has already received **{}/{GIFT_LIMIT_CM} cm** in gifts in the last {GIFT_WINDOW_DAYS} days, so they can only accept **{} cm** right now.{}",
                    recipient.mention(),
                    received.total,
                    received.remaining(),
                    received.frees_up_text()
                ),
                colors::WARNING,
            ))
            .await;
    }

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
                .footer(CreateEmbedFooter::new(format!(
                    "You can gift {} more cm this week. Sharing is caring!",
                    sent.remaining() - amount
                ))),
            ),
    )
    .await
}
