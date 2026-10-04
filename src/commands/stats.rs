use crate::commands::events::active_event;
use crate::commands::{Cmd, CommandResult, grow, viagra};
use crate::db;
use crate::time;
use crate::utils::{colors, embed, escape_markdown, ordinal, pluralize, rank_title};
use chrono::Duration;
use serenity::all::{
    CommandOptionType, CreateCommand, CreateCommandOption, CreateEmbedFooter, ResolvedValue,
};

pub fn register() -> CreateCommand {
    CreateCommand::new("stats")
        .description("View your or another user's stats")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::User,
                "user",
                "The user whose stats you want to view",
            )
            .required(false),
        )
}

pub async fn run(cmd: &Cmd<'_>) -> CommandResult {
    let target = cmd
        .interaction
        .data
        .options()
        .into_iter()
        .find_map(|option| match option.value {
            ResolvedValue::User(user, _) => Some(user),
            _ => None,
        })
        .unwrap_or(&cmd.interaction.user);
    let is_self = target.id == cmd.interaction.user.id;
    let user_id = target.id.to_string();

    let Some(stats) = sqlx::query!(
        "SELECT length, dick_of_day_count, last_grow, growth_count,
                pvp_wins, pvp_losses, pvp_max_streak, pvp_current_streak, cm_won, cm_lost,
                daily_streak, best_daily_streak, viagra_last_used, viagra_active_until,
                daily_growth_boost_percent, daily_cooldown_skips, daily_streak_savers,
                daily_lucky_rolls
         FROM dicks
         WHERE user_id = ? AND guild_id = ?",
        user_id,
        cmd.guild
    )
    .fetch_optional(&cmd.bot.db)
    .await?
    else {
        let message = if is_self {
            "You haven't started growing your dick yet! Use /grow to begin your journey to greatness."
        } else {
            "This user hasn't started growing their dick yet!"
        };
        return cmd
            .reply_ephemeral(embed("❓ No Stats Found", message, colors::NEUTRAL))
            .await;
    };

    let rank = db::guild_rank(&cmd.bot.db, &cmd.guild, stats.length).await?;
    let event = active_event(cmd.bot).await?;
    let cooldown = grow::cooldown_minutes(event.as_ref());
    let ready_at = time::parse(&stats.last_grow)
        .map(|last| last + Duration::minutes(cooldown))
        .filter(|&ready| ready > time::now());
    let growth_status = match (ready_at, is_self) {
        (None, true) => "✅ Ready! Use /grow".to_string(),
        (None, false) => "✅ Ready to grow".to_string(),
        (Some(ready), _) => format!("⏰ Next grow {}", time::relative(ready)),
    };

    let viagra_status =
        if let Some(until) = viagra::active_until(stats.viagra_active_until.as_deref()) {
            format!("💊 **Active**, wears off {}", time::relative(until))
        } else if let Some(ready) = viagra::cooldown_ends(stats.viagra_last_used.as_deref()) {
            format!("⏳ Available {}", time::relative(ready))
        } else {
            "✅ Available now".to_string()
        };

    let mut perks = Vec::new();
    if stats.daily_growth_boost_percent > 0 {
        perks.push(format!(
            "⚡ Next grow +{}%",
            stats.daily_growth_boost_percent
        ));
    }
    for (count, label) in [
        (stats.daily_cooldown_skips, "⏩ Cooldown skip"),
        (stats.daily_lucky_rolls, "🍀 Lucky roll"),
        (stats.daily_streak_savers, "🛟 Streak saver"),
    ] {
        if count > 0 {
            perks.push(format!("{label} ×{count}"));
        }
    }
    let perks = if perks.is_empty() {
        "None. Try /daily!".to_string()
    } else {
        perks.join("\n")
    };

    let fights = stats.pvp_wins + stats.pvp_losses;
    let win_rate = if fights > 0 {
        stats.pvp_wins as f64 / fights as f64 * 100.0
    } else {
        0.0
    };

    let assessment = match stats.length {
        ..=0 if is_self => "Your dick is practically an innie at this point. Keep trying!",
        ..=0 => "Their dick is practically an innie at this point. Tragic!",
        1..50 => "It's... cute? At least that's what they'll say to be nice.",
        50..100 => "Not bad! In the average zone. But who wants to be average?",
        100..150 => "Impressive length! That's some serious heat down there.",
        150..200 => "WOW! That's a third leg, not a dick! Special pants required?",
        _ => "LEGENDARY! Scientists want to study this mutation. BEWARE!",
    };

    let name = escape_markdown(target.display_name());
    let description = if is_self {
        "Here's everything you wanted to know about your cucumber (and probably some things you didn't):".to_string()
    } else {
        format!("Here's everything to know about {name}'s cucumber:")
    };
    let footer = if is_self {
        "Remember to /grow every day for maximum results!"
    } else {
        "Use /stats without parameters to see your own stats!"
    };

    cmd.reply(
        embed(
            format!("🍆 {name}'s Dick Stats"),
            description,
            colors::PURPLE,
        )
        .field("📏 Length", format!("**{} cm**", stats.length), true)
        .field("🏅 Server Rank", format!("**{}**", ordinal(rank)), true)
        .field("👑 Title", rank_title(rank), true)
        .field(
            "🌱 Growth",
            format!(
                "{}\n{growth_status}",
                pluralize(stats.growth_count, "grow", "grows")
            ),
            true,
        )
        .field(
            "🔥 Daily Streak",
            format!(
                "**{}**\nBest: {}",
                pluralize(stats.daily_streak, "day", "days"),
                pluralize(stats.best_daily_streak, "day", "days")
            ),
            true,
        )
        .field(
            "🏆 Dick of the Day",
            pluralize(stats.dick_of_day_count, "time", "times"),
            true,
        )
        .field("💊 Viagra", viagra_status, true)
        .field("🎒 Perks", perks, true)
        .field(
            "⚔️ Battle Stats",
            format!(
                "**{}W / {}L** ({win_rate:.1}% win rate)\n\
                     Current streak: **{}** · Best streak: **{}**\n\
                     Won: **{} cm** · Lost: **{} cm**",
                stats.pvp_wins,
                stats.pvp_losses,
                stats.pvp_current_streak,
                stats.pvp_max_streak,
                stats.cm_won,
                stats.cm_lost
            ),
            false,
        )
        .field("🩺 Professional Assessment", assessment, false)
        .thumbnail(target.face())
        .footer(CreateEmbedFooter::new(footer)),
    )
    .await
}
