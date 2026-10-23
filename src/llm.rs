use log::warn;
use serde_json::{Value, json};
use std::time::Duration;

const DEFAULT_LLM_URL: &str = "http://host.docker.internal:8081/v1/chat/completions";
// The result is already posted; the commentary is only worth adding while people still look at it.
const TIMEOUT: Duration = Duration::from_secs(10);
const MAX_CHARS: usize = 250;

const SYSTEM_PROMPT: &str = "You write one line of commentary for a silly Discord game where users grow a virtual dick and compete on a server leaderboard. Be crude, funny and short: at most 25 words. No emojis, no hashtags, no quotes, no @ symbols. Only mention the players named in the event, and never invent other people.";

/// Asks the local LLM for a one-line comment on a game event. Returns `None`
/// when the server is unreachable, slow or disabled with an empty `LLM_URL`;
/// callers just keep their static message then.
pub async fn commentary(event: &str) -> Option<String> {
    let url = std::env::var("LLM_URL").unwrap_or_else(|_| DEFAULT_LLM_URL.to_string());
    if url.is_empty() {
        return None;
    }

    let request = json!({
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": event},
        ],
        "temperature": 1.0,
        "top_p": 0.95,
        "max_tokens": 80,
        "chat_template_kwargs": {"enable_thinking": false},
    });

    let response = reqwest::Client::new()
        .post(&url)
        .timeout(TIMEOUT)
        .json(&request)
        .send()
        .await
        .and_then(|res| res.error_for_status());
    let body: Value = match response {
        Ok(res) => match res.json().await {
            Ok(body) => body,
            Err(why) => {
                warn!("Unreadable LLM response: {why}");
                return None;
            }
        },
        Err(why) => {
            warn!("LLM commentary unavailable: {why}");
            return None;
        }
    };

    let text = body["choices"][0]["message"]["content"].as_str()?;
    clean(text)
}

/// Pictographs, symbol emoji and the joiners and selectors that build them.
/// The prompt already asks for no emojis; this catches the ones the model
/// adds anyway.
fn is_emoji(c: char) -> bool {
    matches!(
        c as u32,
        0x1F000..=0x1FAFF
            | 0x2300..=0x23FF
            | 0x2600..=0x27BF
            | 0x2B00..=0x2BFF
            | 0xFE00..=0xFE0F
            | 0x200D
            | 0x20E3
            | 0xE0020..=0xE007F
    )
}

fn clean(text: &str) -> Option<String> {
    let without_emoji: String = text.chars().filter(|&c| !is_emoji(c)).collect();
    let line = without_emoji
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches('"')
        .replace('@', "")
        // A stripped emoji right before punctuation leaves a stray space.
        .replace(" !", "!")
        .replace(" ?", "?")
        .replace(" .", ".")
        .replace(" ,", ",");
    if line.is_empty() {
        return None;
    }
    if line.chars().count() <= MAX_CHARS {
        return Some(line);
    }
    let mut cut: String = line.chars().take(MAX_CHARS).collect();
    cut.push('…');
    Some(cut)
}

#[cfg(test)]
mod tests {
    use super::clean;

    #[test]
    fn flattens_and_strips_mentions() {
        assert_eq!(
            clean(" \"@everyone look:\n kuba wins\" ").as_deref(),
            Some("everyone look: kuba wins")
        );
    }

    #[test]
    fn strips_emojis() {
        assert_eq!(
            clean("Kuba is king 👑 and ania 🍆 is ❤️ close 👨‍💻!").as_deref(),
            Some("Kuba is king and ania is close!")
        );
        assert_eq!(clean("🍆🔥"), None);
    }

    #[test]
    fn rejects_empty_and_caps_length() {
        assert_eq!(clean("  \n "), None);
        let long = clean(&"a".repeat(400)).unwrap();
        assert_eq!(long.chars().count(), 251);
    }
}
