use crate::get_dictionary;

pub fn apply_slang(mut text: String) -> String {
    let dict = get_dictionary();
    for (re, replacement) in &dict.slang_rules {
        if re.is_match(&text) {
            text = re.replace_all(&text, replacement.as_str()).to_string();
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shortened_texts_conversion() {
        assert_eq!(apply_slang("gtg brb".to_string()), "gotta go be right back");
        assert_eq!(apply_slang("GTG rn".to_string()), "gotta go right now");
        assert_eq!(apply_slang("yk that's cool".to_string()), "you know that's cool");
    }

    #[test]
    fn test_noone_conversion() {
        assert_eq!(apply_slang("Noone is here".to_string()), "no one is here");
        assert_eq!(apply_slang("noone showed up".to_string()), "no one showed up");
    }

    #[test]
    fn test_forgor_conversion() {
        assert_eq!(apply_slang("I forgor".to_string()), "I four gore");
        assert_eq!(apply_slang("i forgor".to_string()), "i four gore");
        assert_eq!(apply_slang("FORGOR".to_string()), "four gore");
    }

    #[test]
    fn test_sry_conversion() {
        assert_eq!(apply_slang("sry for the delay".to_string()), "sorry for the delay");
        assert_eq!(apply_slang("SRY".to_string()), "sorry");
    }

    #[test]
    fn test_boop_conversion() {
        assert_eq!(apply_slang("Beep boop".to_string()), "Beep boope");
        assert_eq!(apply_slang("boop".to_string()), "boope");
        assert_eq!(apply_slang("BOOP".to_string()), "boope");
    }

    #[test]
    fn test_ya_conversion() {
        assert_eq!(apply_slang("ya".to_string()), "yah");
        assert_eq!(apply_slang("YA".to_string()), "yah");
        assert_eq!(apply_slang("see ya".to_string()), "see yah");
    }

    #[test]
    fn test_goated_conversion() {
        assert_eq!(apply_slang("that was goated".to_string()), "that was goat ted");
        assert_eq!(apply_slang("GOATED".to_string()), "goat ted");
    }

    #[test]
    fn test_ms_and_number_ms_conversion() {
        assert_eq!(apply_slang("i can do it ms".to_string()), "i can do it myself");
        assert_eq!(apply_slang("ping is 50ms".to_string()), "ping is 50 M S");
        assert_eq!(apply_slang("ping is 50 ms".to_string()), "ping is 50 M S");
        assert_eq!(apply_slang("delay: 1.5ms".to_string()), "delay: 1.5 M S");
        assert_eq!(apply_slang("delay: 1.5 ms".to_string()), "delay: 1.5 M S");
        assert_eq!(apply_slang("0ms latency".to_string()), "0 M S latency");
    }

    #[test]
    fn test_xd_conversion() {
        assert_eq!(apply_slang("that's hilarious XD".to_string()), "that's hilarious X D");
        assert_eq!(apply_slang("xd".to_string()), "X D");
        assert_eq!(apply_slang("xdd".to_string()), "X D");
    }
}
