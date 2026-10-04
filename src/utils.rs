use serenity::all::CreateEmbed;

pub mod colors {
    pub const ERROR: u32 = 0xE74C3C;
    pub const WARNING: u32 = 0xFF5733;
    pub const NEUTRAL: u32 = 0xAAAAAA;
    pub const SUCCESS: u32 = 0x2ECC71;
    pub const INFO: u32 = 0x3498DB;
    pub const PURPLE: u32 = 0x9B59B6;
    pub const GOLD: u32 = 0xF1C40F;
}

pub fn embed(title: impl Into<String>, description: impl Into<String>, color: u32) -> CreateEmbed {
    CreateEmbed::new()
        .title(title)
        .description(description)
        .color(color)
}

pub fn error_embed(title: &str, description: impl Into<String>) -> CreateEmbed {
    embed(title, description, colors::ERROR)
}

pub fn rank_title(rank: usize) -> &'static str {
    match rank {
        1 => "GOD OF DICKS",
        2 => "Legendary Organ",
        3 => "Impressive Member",
        4..=10 => "Rising Star",
        _ => "Tiny but Mighty",
    }
}

pub fn medal(index: usize) -> &'static str {
    match index {
        0 => "🥇",
        1 => "🥈",
        2 => "🥉",
        _ => "🔹",
    }
}

/// Formats a number with its English ordinal suffix, e.g. `1st`, `12th`, `23rd`.
pub fn ordinal(n: usize) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

pub fn pluralize(count: i64, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("{count} {singular}")
    } else {
        format!("{count} {plural}")
    }
}

pub fn escape_markdown(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '\\' | '*' | '_' | '`' | '~' | '|') {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinals() {
        let cases = [
            (1, "1st"),
            (2, "2nd"),
            (3, "3rd"),
            (4, "4th"),
            (11, "11th"),
            (12, "12th"),
            (13, "13th"),
            (21, "21st"),
            (102, "102nd"),
            (111, "111th"),
        ];
        for (n, expected) in cases {
            assert_eq!(ordinal(n), expected);
        }
    }

    #[test]
    fn escapes_markdown() {
        assert_eq!(escape_markdown("a_b*c"), "a\\_b\\*c");
        assert_eq!(escape_markdown("plain"), "plain");
    }
}
