use crate::commands::{Cmd, CommandResult};
use crate::db::{self, History};
use crate::time;
use crate::utils::{colors, embed, ordinal, rank_title};
use log::info;
use rand::RngExt;
use rand::seq::IndexedRandom;
use serenity::all::{
    CreateCommand, CreateEmbedFooter, CreateInteractionResponseMessage, Mentionable, UserId,
};

const BONUS_CM_MIN: i64 = 10;
const BONUS_CM_MAX: i64 = 25;
const ACTIVE_DAYS: i64 = 7;

pub fn register() -> CreateCommand {
    CreateCommand::new("dickoftheday").description("Randomly select a Dick of the Day")
}

pub async fn run(cmd: &Cmd<'_>) -> CommandResult {
    let db = &cmd.bot.db;
    sqlx::query!(
        "INSERT OR IGNORE INTO guild_settings (guild_id, last_dotd)
         VALUES (?, '1970-01-01 00:00:00')",
        cmd.guild
    )
    .execute(db)
    .await?;

    let already_awarded = sqlx::query_scalar!(
        r#"SELECT date(last_dotd) >= date('now') as "awarded!: bool"
         FROM guild_settings WHERE guild_id = ?"#,
        cmd.guild
    )
    .fetch_one(db)
    .await?;
    if already_awarded {
        return reply_already_awarded(cmd).await;
    }

    let active_window = format!("-{ACTIVE_DAYS} days");
    let candidates = sqlx::query_scalar!(
        "SELECT user_id FROM dicks WHERE guild_id = ? AND last_grow > datetime('now', ?)",
        cmd.guild,
        active_window
    )
    .fetch_all(db)
    .await?;
    if candidates.len() < 2 {
        return cmd
            .reply(embed(
                "🔍 Not Enough Active Users",
                format!(
                    "At least 2 people need to have grown in the last {ACTIVE_DAYS} days to award Dick of the Day! Get more people growing!"
                ),
                colors::NEUTRAL,
            ))
            .await;
    }

    // Claim today's award atomically so two simultaneous invocations can't both pay out.
    let claimed = sqlx::query!(
        "UPDATE guild_settings SET last_dotd = datetime('now')
         WHERE guild_id = ? AND date(last_dotd) < date('now')",
        cmd.guild
    )
    .execute(db)
    .await?
    .rows_affected()
        > 0;
    if !claimed {
        return reply_already_awarded(cmd).await;
    }

    let (winner, bonus) = {
        let mut rng = rand::rng();
        let winner = candidates
            .choose(&mut rng)
            .expect("at least two candidates");
        (winner, rng.random_range(BONUS_CM_MIN..=BONUS_CM_MAX))
    };
    info!(
        "DOTD in guild {}: {winner} won {bonus} cm out of {} candidates",
        cmd.guild,
        candidates.len()
    );

    let new_length = sqlx::query_scalar!(
        "UPDATE dicks SET length = length + ?, dick_of_day_count = dick_of_day_count + 1
         WHERE user_id = ? AND guild_id = ?
         RETURNING length",
        bonus,
        winner,
        cmd.guild
    )
    .fetch_one(db)
    .await?;
    db::log_history(db, winner, &cmd.guild, new_length, bonus, History::Dotd).await?;
    let rank = db::guild_rank(db, &cmd.guild, new_length).await?;

    let winner_id = UserId::new(winner.parse().unwrap_or(1));
    let mention = winner_id.mention();
    let mut announcement = embed(
        "🏆 Today's Dick of the Day! 🏆",
        format!(
            "After careful consideration, the Dick of the Day award goes to... {mention}!\n\n\
             This \"**{}**\" has been awarded a bonus of **+{bonus} cm**, bringing their total to **{new_length} cm**!\n\n\
             🏅 Server rank: **{}**\n\
             ⏰ Next Dick of the Day: {}\n\n\
             Congratulations on your outstanding achievement in the field of... length!",
            rank_title(rank),
            ordinal(rank),
            time::relative(time::next_utc_midnight())
        ),
        colors::GOLD,
    )
    .footer(CreateEmbedFooter::new(
        "Stay tuned for tomorrow's competition! (and don't forget to /grow)",
    ));
    if let Ok(user) = winner_id.to_user(cmd.ctx).await {
        announcement = announcement.thumbnail(user.face());
    }

    cmd.respond(
        CreateInteractionResponseMessage::new()
            .content(mention.to_string())
            .embed(announcement),
    )
    .await
}

async fn reply_already_awarded(cmd: &Cmd<'_>) -> CommandResult {
    cmd.reply_ephemeral(embed(
        "⏰ Dick of the Day Already Awarded!",
        format!(
            "This server has already crowned a Dick of the Day today!\n\nNext Dick of the Day {}",
            time::relative(time::next_utc_midnight())
        ),
        colors::WARNING,
    ))
    .await
}
