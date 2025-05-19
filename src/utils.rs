use crate::Bot;
use serenity::prelude::*;

pub fn escape_markdown(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('`', "\\`")
        .replace('~', "\\~")
        .replace('|', "\\|")
}

#[derive(Debug)]
pub struct BotStats {
    pub server_count: usize,
    pub dick_count: i64,
}

pub async fn get_bot_stats(ctx: &Context, bot: &Bot) -> Result<BotStats, sqlx::Error> {
    let server_count = ctx.cache.guilds().len();

    let dick_count_result = sqlx::query!("SELECT COUNT(*) as count FROM dicks")
        .fetch_one(&bot.database)
        .await?;

    let dick_count = dick_count_result.count;

    Ok(BotStats {
        server_count,
        dick_count,
    })
}

pub fn get_fun_title_by_rank(rank: usize) -> &'static str {
    match rank {
        1 => "GOD OF SCHLONGS",
        2 => "Legendary Organ",
        3 => "Impressive Member",
        4..=10 => "Rising Star",
        _ => "Tiny but Mighty",
    }
}
