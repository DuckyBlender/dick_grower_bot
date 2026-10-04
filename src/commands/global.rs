use crate::commands::top::display_names;
use crate::commands::{Cmd, CommandResult};
use crate::utils::{colors, embed, escape_markdown, medal};
use rand::seq::IndexedRandom;
use serenity::all::{Cache, CreateCommand, CreateEmbedFooter, GuildId};

pub fn register() -> CreateCommand {
    CreateCommand::new("global")
        .description("Show the top players with the biggest weapons across all servers")
}

/// Only Community servers are named publicly; everything else stays anonymous.
fn guild_label(cache: &Cache, raw_id: &str) -> String {
    let guild = raw_id
        .parse::<u64>()
        .ok()
        .filter(|&id| id != 0)
        .and_then(|id| cache.guild(GuildId::new(id)));
    match guild {
        Some(guild) if guild.features.iter().any(|feature| feature == "COMMUNITY") => {
            escape_markdown(&guild.name)
        }
        Some(_) => "a private server".to_string(),
        None => "an unknown server".to_string(),
    }
}

pub async fn run(cmd: &Cmd<'_>) -> CommandResult {
    cmd.defer().await?;

    let top = sqlx::query!(
        "SELECT user_id, guild_id, length FROM dicks
         ORDER BY length DESC, user_id ASC
         LIMIT 10"
    )
    .fetch_all(&cmd.bot.db)
    .await?;

    if top.is_empty() {
        return cmd
            .edit(embed(
                "👀 No Dicks Found",
                "Nobody has grown their dick anywhere yet. The world awaits a pioneer!",
                colors::NEUTRAL,
            ))
            .await;
    }

    let total_dicks = sqlx::query_scalar!("SELECT COUNT(*) FROM dicks")
        .fetch_one(&cmd.bot.db)
        .await?;
    let names = display_names(cmd.ctx, top.iter().map(|row| row.user_id.as_str())).await;

    let mut description = String::from("Here are the biggest dicks in the entire world:\n\n");
    for (i, (row, name)) in top.iter().zip(&names).enumerate() {
        description.push_str(&format!(
            "{} **{}. {name}**: {} cm (from {})\n",
            medal(i),
            i + 1,
            row.length,
            guild_label(&cmd.ctx.cache, &row.guild_id)
        ));
    }

    let champion = &names[0];
    let comments = [
        format!("NASA wants to study {champion}'s dick as a possible space elevator!"),
        format!("{champion} must need a special permit to carry that thing around!"),
        format!("{champion} is making the rest of the world feel inadequate!"),
        format!("{champion} is the global champion..."),
    ];
    let comment = comments
        .choose(&mut rand::rng())
        .expect("comments are not empty");
    description.push_str(&format!("\n{comment}"));

    cmd.edit(
        embed("🌍 Global Dick Leaderboard 🏆", description, colors::PURPLE).footer(
            CreateEmbedFooter::new(format!(
                "🌐 {} servers · 🍆 {total_dicks} dicks · World domination starts with /grow!",
                cmd.ctx.cache.guild_count()
            )),
        ),
    )
    .await
}
