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

/// Size description for AI prompts, from the player's position among the
/// server's `players` who have grown. Lengths on a server span orders of
/// magnitude, so 50 cm is big on one server and small on another; the
/// position is what matters. Coarse on purpose, so a prepared comment built
/// on it stays valid for several grows.
pub fn size_class(length: i64, rank: usize, players: i64) -> &'static str {
    if length <= 0 {
        return "practically an innie";
    }
    if rank == 1 {
        return "the biggest on this server";
    }
    // Share of the server that is bigger than this player.
    let bigger = (rank - 1) as f64 / players.max(1) as f64;
    match bigger {
        b if b < 0.05 => "one of the biggest on this server",
        b if b < 0.10 => "big for this server",
        b if b < 0.75 => "about average for this server",
        _ => "small for this server",
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
    fn size_class_follows_the_server_position() {
        assert_eq!(size_class(20814, 1, 55), "the biggest on this server");
        assert_eq!(
            size_class(5000, 3, 100),
            "one of the biggest on this server"
        );
        assert_eq!(size_class(50, 8, 100), "big for this server");
        assert_eq!(size_class(50, 20, 100), "about average for this server");
        assert_eq!(size_class(50, 50, 100), "about average for this server");
        assert_eq!(size_class(5, 90, 100), "small for this server");
        assert_eq!(size_class(0, 99, 100), "practically an innie");
        // Ranked against players who never grew, so the rank can exceed the count.
        assert_eq!(size_class(3, 7, 4), "small for this server");
    }

    #[test]
    fn escapes_markdown() {
        assert_eq!(escape_markdown("a_b*c"), "a\\_b\\*c");
        assert_eq!(escape_markdown("plain"), "plain");
    }
}
