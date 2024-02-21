use std::str::FromStr;
use unicode_segmentation::UnicodeSegmentation;
#[derive(Debug)]
pub struct SubscriberName(String);

impl FromStr for SubscriberName {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let is_empty_or_whitespace = s.trim().is_empty();
        let is_too_long = s.graphemes(true).count() > 256;
        let forbidden_chars = ['/', '(', ')', '"', '<', '>', '\\', '{', '}', ';'];
        let contains_forbidden_chars = s.chars().any(|g| forbidden_chars.contains(&g));
        if is_empty_or_whitespace || is_too_long || contains_forbidden_chars {
            Err(format!("{} is not a valid subscriber name.", s))
        } else {
            Ok(SubscriberName(s.to_string()))
        }
    }
}

impl AsRef<str> for SubscriberName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn a_256_grapheme_long_name_is_valid() {
        let name = "a".repeat(256);
        assert!(name.parse::<SubscriberName>().is_ok());
    }

    #[test]
    fn a_name_longer_than_256_graphemes_is_rejected() {
        let name = "a".repeat(257);
        assert!(name.parse::<SubscriberName>().is_err());
    }

    #[test]
    fn whitespace_only_name_is_rejected() {
        let name = "   ";
        assert!(name.parse::<SubscriberName>().is_err());
    }

    #[test]
    fn empty_string_is_rejected() {
        let name = "";
        assert!(name.parse::<SubscriberName>().is_err());
    }

    #[test]
    fn names_containing_an_invliad_character_are_rejected() {
        for name in ['/', '(', ')', '"', '<', '>', '\\', '{', '}', ';'] {
            let name = name.to_string();
            assert!(name.parse::<SubscriberName>().is_err());
        }
    }

    #[test]
    fn a_valid_name_is_parsed_successfully() {
        let name = "Hello Kitty";
        assert!(name.parse::<SubscriberName>().is_ok());
    }
}
