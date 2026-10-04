use crate::commands::{Cmd, CommandResult};
use crate::db;
use crate::utils::{colors, embed, escape_markdown, medal, ordinal};
use serenity::all::{Context, CreateCommand, CreateEmbedFooter, UserId};
use serenity::futures::future::join_all;

pub fn register() -> CreateCommand {
    CreateCommand::new("top")
        .description("Show the top players with the biggest weapons in this server")
}

/// Looks up display names for stored user IDs concurrently.
pub async fn display_names<'a>(
    ctx: &Context,
    user_ids: impl IntoIterator<Item = &'a str>,
) -> Vec<String> {
    let lookups = user_ids.into_iter().map(|raw_id| async move {
        let user = match raw_id.parse::<u64>() {
            Ok(id) if id != 0 => UserId::new(id).to_user(ctx).await.ok(),
            _ => None,
        };
        user.map_or_else(
            || "Unknown User".to_string(),
            |user| escape_markdown(user.display_name()),
        )
    });
    join_all(lookups).await
}

pub async fn run(cmd: &Cmd<'_>) -> CommandResult {
    // Resolving names can take longer than Discord's 3 second response window.
    cmd.defer().await?;

    let top = sqlx::query!(
        "SELECT user_id, length FROM dicks
         WHERE guild_id = ?
         ORDER BY length DESC, user_id ASC
         LIMIT 10",
        cmd.guild
    )
    .fetch_all(&cmd.bot.db)
    .await?;

    let Some(leader) = top.first() else {
        return cmd
            .edit(embed(
                "👀 No Dicks Found",
                "Nobody has grown their dick in this server yet. Be the first one!",
                colors::NEUTRAL,
            ))
            .await;
    };

    let names = display_names(cmd.ctx, top.iter().map(|row| row.user_id.as_str())).await;
    let mut description = String::from("Here are the biggest dicks in this server:\n\n");
    for (i, (row, name)) in top.iter().zip(&names).enumerate() {
        let you = if row.user_id == cmd.user {
            " ← you"
        } else {
            ""
        };
        description.push_str(&format!(
            "{} **{}. {name}**: {} cm{you}\n",
            medal(i),
            i + 1,
            row.length
        ));
    }

    if !top.iter().any(|row| row.user_id == cmd.user)
        && let Some(length) = sqlx::query_scalar!(
            "SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?",
            cmd.user,
            cmd.guild
        )
        .fetch_optional(&cmd.bot.db)
        .await?
    {
        let rank = db::guild_rank(&cmd.bot.db, &cmd.guild, length).await?;
        description.push_str(&format!(
            "⋯\n📍 **You: {}** with {length} cm\n",
            ordinal(rank)
        ));
    }

    let leader_name = &names[0];
    let comment = match leader.length {
        51.. => format!("Holy moly! {leader_name}'s dick is so big it needs its own ZIP code!"),
        31.. => format!("Beware of {leader_name} in tight spaces. That thing is a lethal weapon!"),
        16.. => format!("{leader_name} is doing quite well. Impressive... most impressive."),
        1.. => format!("{leader_name} is trying their best, though. Gold star for effort!"),
        _ => format!("Poor {leader_name}... we need a microscope to find their dick."),
    };
    description.push_str(&format!("\n{comment}"));

    let guild_name = cmd
        .guild_id
        .name(&cmd.ctx.cache)
        .map_or_else(|| "This Server".to_string(), |name| escape_markdown(&name));

    cmd.edit(
        embed(
            format!("🍆 Dick Leaderboard: {guild_name} 🏆"),
            description,
            colors::PURPLE,
        )
        .footer(CreateEmbedFooter::new(
            "Use /grow daily to increase your length!",
        )),
    )
    .await
}
