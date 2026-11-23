use crate::commands::grow::DEFAULT_COOLDOWN_MINUTES;
use crate::commands::{Cmd, CommandResult};
use crate::utils::{colors, embed};
use serenity::all::{CreateCommand, CreateEmbedFooter};

pub fn register() -> CreateCommand {
    CreateCommand::new("help").description("Show help information about the bot commands")
}

pub async fn run(cmd: &Cmd<'_>) -> CommandResult {
    let growing = format!(
        "`/grow` - Grow your dick (every {DEFAULT_COOLDOWN_MINUTES} minutes)\n\
         `/daily` - Claim a random perk once per UTC day\n\
         `/viagra` - Daily pill: next grow doubled, or it backfires\n\
         `/event` - View the current global event\n\
         `/season` - This month's leaderboard of freshly grown cm"
    );

    cmd.reply(
        embed(
            "🍆 Dick Grower Bot Help",
            "Grow daily, keep your streak alive and battle your friends for the biggest dick in town!",
            colors::SUCCESS,
        )
        .field("🌱 Growing", growing, false)
        .field(
            "⚔️ Competing",
            "`/pvp <bet>` - Challenge anyone to a dick battle\n\
             `/dickoftheday` - Crown a random active grower (once per day)\n\
             `/gift <user> <amount>` - Give length to someone smaller than you",
            false,
        )
        .field(
            "📊 Info",
            "`/stats [user]` - Your (or someone's) stats and perks\n\
             `/top` - Server leaderboard\n\
             `/global` - Global leaderboard",
            false,
        )
        .field(
            "🔔 Updates & Community",
            "Join our Discord for announcements and other projects: [Discord Server](https://discord.gg/39nqUzYGbe)",
            false,
        )
        .footer(CreateEmbedFooter::new(
            "Compete with friends for the biggest dick in town!",
        )),
    )
    .await
}
