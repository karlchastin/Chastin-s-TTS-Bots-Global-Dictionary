use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashMap;

use super::cleaner::clean_tts_text;
use crate::get_dictionary;

lazy_static! {
    static ref MENTION_RE: Regex = Regex::new(r"<@!?\d+>").unwrap();
    static ref URL_RE: Regex = Regex::new(r"https?://[^\s]+").unwrap();
    static ref ALPHA_RE: Regex = Regex::new(r"[^a-zA-Z\s]").unwrap();
}

pub struct MessageMeta<'a> {
    pub content: &'a str,
    pub clean_content: &'a str,
    pub image_count: usize,
    pub mentions: &'a [String],
    pub codename: &'a str,
    pub is_consecutive: bool,
}

pub fn format_tts_prompt(meta: &MessageMeta) -> Option<(String, bool)> {
    let dict = get_dictionary();
    let raw_text = MENTION_RE.replace_all(meta.content, "").trim().to_string();

    if meta.clean_content.trim().is_empty() && meta.image_count > 0 {
        let text = if meta.image_count == 1 {
            if meta.is_consecutive {
                "sent an image".to_string()
            } else {
                format!("{} sent an image", meta.codename)
            }
        } else if meta.is_consecutive {
            "sent multiple images".to_string()
        } else {
            format!("{} sent multiple images", meta.codename)
        };
        return Some((text, false));
    }

    if raw_text.is_empty() && !meta.mentions.is_empty() {
        let text = if meta.mentions.len() == 1 {
            let target = &meta.mentions[0];
            if meta.is_consecutive {
                format!("mentioned {target}.")
            } else {
                format!("{} mentioned {target}.", meta.codename)
            }
        } else if meta.is_consecutive {
            "mentioned multiple users.".to_string()
        } else {
            format!("{} mentioned multiple users.", meta.codename)
        };
        return Some((text, false));
    }

    let base_text = if meta.mentions.len() >= 2 {
        raw_text.as_str()
    } else {
        meta.clean_content
    };

    let is_yelling = !base_text.is_empty()
        && base_text.chars().any(|c| c.is_alphabetic())
        && base_text == base_text.to_uppercase();

    let words: Vec<&str> = base_text.split_whitespace().collect();
    if !words.is_empty() && words.iter().all(|w| URL_RE.is_match(w)) {
        let text = if meta.is_consecutive {
            "sent a link".to_string()
        } else {
            format!("{} sent a link", meta.codename)
        };
        return Some((text, is_yelling));
    }

    let text_without_links = URL_RE.replace_all(base_text, "link").to_string();
    let clean_alpha = ALPHA_RE.replace_all(&text_without_links, "").to_lowercase();
    let significant_words: Vec<&str> = clean_alpha
        .split_whitespace()
        .filter(|w| w.len() > 3)
        .collect();

    let threshold = dict.limits.repetitive_word_threshold.max(2);
    let mut counts: HashMap<&str, usize> = HashMap::new();
    let mut is_repetitive = false;
    for w in significant_words {
        let count = counts.entry(w).or_insert(0);
        *count += 1;
        if *count >= threshold {
            is_repetitive = true;
            break;
        }
    }

    if text_without_links.len() > dict.limits.max_message_len {
        let text = if meta.is_consecutive {
            "sent a long message in the VC chat".to_string()
        } else {
            format!("{} sent a long message in the VC chat", meta.codename)
        };
        return Some((text, is_yelling));
    }

    if is_repetitive {
        let text = if meta.is_consecutive {
            "is spamming repetitive words".to_string()
        } else {
            format!("{} is spamming repetitive words", meta.codename)
        };
        return Some((text, is_yelling));
    }

    let (cleaned_text, is_pure_emoji) = clean_tts_text(&text_without_links);
    if cleaned_text.trim().is_empty() {
        return None;
    }

    let full_text = if is_pure_emoji {
        if meta.is_consecutive {
            format!("sent {cleaned_text}")
        } else {
            format!("{} sent {cleaned_text}", meta.codename)
        }
    } else if meta.mentions.len() >= 2 {
        if meta.is_consecutive {
            format!("mentioned multiple users and said: {cleaned_text}")
        } else {
            format!(
                "{} mentioned multiple users and said: {cleaned_text}",
                meta.codename
            )
        }
    } else if meta.is_consecutive {
        cleaned_text
    } else {
        format!("{} said: {cleaned_text}", meta.codename)
    };

    Some((full_text, is_yelling))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_attachment_prompt() {
        let meta = MessageMeta {
            content: "",
            clean_content: "",
            image_count: 1,
            mentions: &[],
            codename: "Ghost",
            is_consecutive: false,
        };
        let (prompt, _) = format_tts_prompt(&meta).unwrap();
        assert_eq!(prompt, "Ghost sent an image");
    }

    #[test]
    fn test_consecutive_speech() {
        let meta = MessageMeta {
            content: "Hello team",
            clean_content: "Hello team",
            image_count: 0,
            mentions: &[],
            codename: "Ghost",
            is_consecutive: true,
        };
        let (prompt, _) = format_tts_prompt(&meta).unwrap();
        assert_eq!(prompt, "Hello team");
    }

    #[test]
    fn test_non_consecutive_speech() {
        let meta = MessageMeta {
            content: "Hello team",
            clean_content: "Hello team",
            image_count: 0,
            mentions: &[],
            codename: "Ghost",
            is_consecutive: false,
        };
        let (prompt, _) = format_tts_prompt(&meta).unwrap();
        assert_eq!(prompt, "Ghost said: Hello team");
    }

    #[test]
    fn test_url_detection() {
        let meta = MessageMeta {
            content: "https://google.com",
            clean_content: "https://google.com",
            image_count: 0,
            mentions: &[],
            codename: "Ghost",
            is_consecutive: false,
        };
        let (prompt, _) = format_tts_prompt(&meta).unwrap();
        assert_eq!(prompt, "Ghost sent a link");
    }

    #[test]
    fn test_mention_only_single_and_multiple() {
        let single = MessageMeta {
            content: "<@123>",
            clean_content: "@Chahs",
            image_count: 0,
            mentions: &["Chahs".to_string()],
            codename: "Ghost",
            is_consecutive: false,
        };
        assert_eq!(
            format_tts_prompt(&single).unwrap().0,
            "Ghost mentioned Chahs."
        );

        let multiple = MessageMeta {
            content: "<@123> <@456>",
            clean_content: "@Chahs @Viper",
            image_count: 0,
            mentions: &["Chahs".to_string(), "Viper".to_string()],
            codename: "Ghost",
            is_consecutive: false,
        };
        assert_eq!(
            format_tts_prompt(&multiple).unwrap().0,
            "Ghost mentioned multiple users."
        );

        let multiple_consecutive = MessageMeta {
            content: "<@123> <@456>",
            clean_content: "@Chahs @Viper",
            image_count: 0,
            mentions: &["Chahs".to_string(), "Viper".to_string()],
            codename: "Ghost",
            is_consecutive: true,
        };
        assert_eq!(
            format_tts_prompt(&multiple_consecutive).unwrap().0,
            "mentioned multiple users."
        );
    }

    #[test]
    fn test_multiple_mentions_with_text() {
        let prefix = MessageMeta {
            content: "<@123> <@456> are you ready?",
            clean_content: "@Chahs @Viper are you ready?",
            image_count: 0,
            mentions: &["Chahs".to_string(), "Viper".to_string()],
            codename: "Ghost",
            is_consecutive: false,
        };
        assert_eq!(
            format_tts_prompt(&prefix).unwrap().0,
            "Ghost mentioned multiple users and said: are you ready?"
        );

        let suffix = MessageMeta {
            content: "are you ready? <@123> <@456>",
            clean_content: "are you ready? @Chahs @Viper",
            image_count: 0,
            mentions: &["Chahs".to_string(), "Viper".to_string()],
            codename: "Ghost",
            is_consecutive: false,
        };
        assert_eq!(
            format_tts_prompt(&suffix).unwrap().0,
            "Ghost mentioned multiple users and said: are you ready?"
        );

        let consecutive = MessageMeta {
            content: "<@123> <@456> hurry up",
            clean_content: "@Chahs @Viper hurry up",
            image_count: 0,
            mentions: &["Chahs".to_string(), "Viper".to_string()],
            codename: "Ghost",
            is_consecutive: true,
        };
        assert_eq!(
            format_tts_prompt(&consecutive).unwrap().0,
            "mentioned multiple users and said: hurry up"
        );
    }
}
