use lazy_static::lazy_static;
use regex::Regex;

use super::slang::apply_slang;
use crate::{get_dictionary, KeyboardSmashConfig};

lazy_static! {
    static ref MARKDOWN_RE: Regex = Regex::new(r"[*_~|`]").unwrap();
    static ref CUSTOM_EMOJI_RE: Regex = Regex::new(r"<a?:\w+:\d+>").unwrap();
}

pub fn clean_tts_text(raw_input: &str) -> (String, bool) {
    let dict = get_dictionary();

    let mut text = dict
        .huge_num_re
        .replace_all(raw_input, dict.limits.huge_number_replacement.as_str())
        .to_string();
    text = MARKDOWN_RE.replace_all(&text, "").to_string();

    for (re, replacement) in &dict.phonetic_pre {
        text = re.replace_all(&text, replacement.as_str()).to_string();
    }

    text = replace_keyboard_spam(&text, &dict.keyboard_smash);
    text = collapse_trailing_e(&text);

    for (re, replacement) in &dict.phonetic_post {
        text = re.replace_all(&text, replacement.as_str()).to_string();
    }

    text = collapse_repeated_characters(&text);
    text = collapse_repeated_phrases(&text);
    text = apply_slang(text);

    if let Some(ref short_re) = dict.short_words_re {
        text = short_re
            .replace_all(&text, |caps: &regex::Captures| caps[0].to_lowercase())
            .to_string();
    }

    for [emoticon, spoken] in &dict.emoticons {
        text = text.replace(emoticon.as_str(), spoken.as_str());
    }

    text = CUSTOM_EMOJI_RE.replace_all(&text, "").to_string();

    process_emojis(&text)
}

fn replace_keyboard_spam(input: &str, cfg: &KeyboardSmashConfig) -> String {
    let mut words = Vec::new();
    for word in input.split_inclusive(char::is_whitespace) {
        let trimmed = word.trim_end();
        let whitespace = &word[trimmed.len()..];

        if !trimmed.is_empty() && trimmed.chars().all(|c| c.is_ascii_alphabetic()) {
            let lower = trimmed.to_lowercase();
            let char_len = lower.chars().count();
            let has_no_vowels = !lower
                .chars()
                .any(|c| matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y'));
            let only_vowel_is_a = lower.contains('a')
                && !lower.chars().any(|c| matches!(c, 'e' | 'i' | 'o' | 'u' | 'y'));

            let is_laugh_spam = only_vowel_is_a
                && char_len >= cfg.laugh_min_len
                && lower
                    .chars()
                    .any(|c| cfg.laugh_required_chars.contains(c))
                && lower
                    .chars()
                    .all(|c| cfg.laugh_allowed_chars.contains(c));

            if char_len >= cfg.no_vowel_min_len && has_no_vowels {
                words.push(format!("{}{whitespace}", cfg.no_vowel_replacement));
                continue;
            } else if is_laugh_spam || (char_len >= cfg.only_a_min_len && only_vowel_is_a) {
                words.push(format!("{}{whitespace}", cfg.laugh_replacement));
                continue;
            }
        }
        words.push(word.to_string());
    }
    words.concat()
}

fn collapse_repeated_characters(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut prev_char: Option<char> = None;
    let mut repeat_count = 0;

    for ch in input.chars() {
        let is_letter = ch.is_alphabetic();
        let is_punct = matches!(ch, '!' | '?' | '.' | ',' | ';' | '-' | '~');

        if let Some(prev) = prev_char {
            if ch.eq_ignore_ascii_case(&prev) && (is_letter || is_punct) {
                repeat_count += 1;
                if repeat_count < 2 {
                    result.push(ch);
                }
                continue;
            }
        }

        prev_char = Some(ch);
        repeat_count = 0;
        result.push(ch);
    }

    result
}

fn collapse_trailing_e(input: &str) -> String {
    let mut parts = Vec::new();
    for word in input.split_inclusive(char::is_whitespace) {
        let trimmed = word.trim_end();
        let whitespace = &word[trimmed.len()..];
        let lower = trimmed.to_lowercase();
        if lower.ends_with("eee") {
            let mut cut = trimmed;
            while cut.to_lowercase().ends_with("ee") {
                cut = &cut[..cut.len() - 1];
            }
            parts.push(format!("{cut}{whitespace}"));
        } else {
            parts.push(word.to_string());
        }
    }
    parts.concat()
}

fn collapse_repeated_phrases(input: &str) -> String {
    let words: Vec<&str> = input.split_whitespace().collect();
    if words.is_empty() {
        return input.to_string();
    }
    let mut result: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let word = words[i];
        let mut count = 1;
        while i + count < words.len() && words[i + count].eq_ignore_ascii_case(word) {
            count += 1;
        }
        if count >= 3 {
            result.push(word);
            result.push(words[i + 1]);
            i += count;
        } else {
            result.push(word);
            i += 1;
        }
    }
    result.join(" ")
}

fn find_longest_emoji(remainder: &str) -> Option<&'static emojis::Emoji> {
    let mut matched_emoji = None;
    for (i, (byte_idx, _)) in remainder.char_indices().enumerate() {
        if i > 8 {
            break;
        }
        let slice_end = byte_idx + remainder[byte_idx..].chars().next().unwrap().len_utf8();
        if let Some(emoji) = emojis::get(&remainder[..slice_end]) {
            matched_emoji = Some(emoji);
        }
    }
    matched_emoji
}

fn process_emojis(input: &str) -> (String, bool) {
    let mut emoji_matches: Vec<String> = Vec::new();
    let mut non_emoji_chars = String::new();

    let mut cursor = 0;
    while cursor < input.len() {
        let remainder = &input[cursor..];
        if let Some(emoji) = find_longest_emoji(remainder) {
            emoji_matches.push(emoji.as_str().to_string());
            cursor += emoji.as_str().len();
        } else {
            let ch = remainder.chars().next().unwrap();
            if !ch.is_whitespace() {
                non_emoji_chars.push(ch);
            }
            cursor += ch.len_utf8();
        }
    }

    if non_emoji_chars.is_empty() {
        if emoji_matches.is_empty() {
            return (String::new(), false);
        }
        let first = &emoji_matches[0];
        if !emoji_matches.iter().all(|e| e == first) {
            return (String::new(), false);
        }
        let label = if emoji_matches.len() > 1 { "emojis" } else { "emoji" };
        return (format!("{first} {label}"), true);
    }

    let mut result = String::with_capacity(input.len() + 16);
    let mut cursor = 0;
    while cursor < input.len() {
        let remainder = &input[cursor..];
        if let Some(emoji) = find_longest_emoji(remainder) {
            result.push_str(&format!(". {} emoji ", emoji.as_str()));
            cursor += emoji.as_str().len();
        } else {
            let ch = remainder.chars().next().unwrap();
            result.push(ch);
            cursor += ch.len_utf8();
        }
    }
    (result.trim().to_string(), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_large_numbers() {
        let (res, _) = clean_tts_text("Account has 123456789012 balance");
        assert!(res.contains("a huge number"));
    }

    #[test]
    fn test_slang_expansion() {
        let (res, _) = clean_tts_text("tbh that was gg wp fr yk");
        assert_eq!(res, "to be honest that was good game well played for real you know");
    }

    #[test]
    fn test_custom_shortened_and_noone() {
        let (res, _) = clean_tts_text("Noone is online so gtg");
        assert_eq!(res, "no one is online so gotta go");
    }

    #[test]
    fn test_short_words_lowercase() {
        let (res, _) = clean_tts_text("WHY ARE YOU HERE");
        assert!(res.contains("why"));
        assert!(res.contains("are"));
        assert!(res.contains("you"));
    }

    #[test]
    fn test_beep_boop_slang() {
        let (res, _) = clean_tts_text("Beep boop");
        assert_eq!(res, "Beep boope");
    }

    #[test]
    fn test_pure_identical_emoji() {
        let (res, pure) = clean_tts_text("🔥");
        assert_eq!(res, "🔥 emoji");
        assert!(pure);

        let (res2, pure2) = clean_tts_text("🔥🔥🔥");
        assert_eq!(res2, "🔥 emojis");
        assert!(pure2);
    }

    #[test]
    fn test_pure_mixed_emoji_suppressed() {
        let (res, pure) = clean_tts_text("🔥💀");
        assert_eq!(res, "");
        assert!(!pure);
    }

    #[test]
    fn test_laughter_and_keyboard_spam() {
        let (res1, _) = clean_tts_text("HASJHSAJHSA");
        assert_eq!(res1, "Hahaha");

        let (res2, _) = clean_tts_text("HAJHAJHAJAH");
        assert_eq!(res2, "Hahaha");

        let (res3, _) = clean_tts_text("hahaha");
        assert_eq!(res3, "Hahaha");
    }

    #[test]
    fn test_ya_pronounced_as_yah() {
        let (res1, _) = clean_tts_text("ya");
        assert_eq!(res1, "yah");

        let (res2, _) = clean_tts_text("YA");
        assert_eq!(res2, "yah");

        let (res3, _) = clean_tts_text("see ya");
        assert_eq!(res3, "see yah");
    }

    #[test]
    fn test_goated_and_ms_and_xd() {
        let (res1, _) = clean_tts_text("he is goated");
        assert_eq!(res1, "he is goat ted");

        let (res2, _) = clean_tts_text("i made it ms");
        assert_eq!(res2, "i made it myself");

        let (res3, _) = clean_tts_text("ping 25ms");
        assert_eq!(res3, "ping 25 M S");

        let (res4, _) = clean_tts_text("ping 25 ms");
        assert_eq!(res4, "ping 25 M S");

        let (res5, _) = clean_tts_text("lol XD");
        assert_eq!(res5, "lol X D");

        let (res6, _) = clean_tts_text("xd that was funny");
        assert_eq!(res6, "X D that was funny");
    }
}
