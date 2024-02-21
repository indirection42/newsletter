use std::str::FromStr;
use validator::validate_email;
#[derive(Debug)]
pub struct SubscriberEmail(String);

impl FromStr for SubscriberEmail {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if validate_email(s) {
            Ok(SubscriberEmail(s.to_string()))
        } else {
            Err(format!("{} is not a valid subscriber email.", s))
        }
    }
}

impl AsRef<str> for SubscriberEmail {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::SubscriberEmail;
    use fake::faker::internet::en::SafeEmail;
    use fake::Fake;
    use quickcheck::Arbitrary;

    #[derive(Debug, Clone)]
    struct ValidEmailFixture(pub String);

    impl Arbitrary for ValidEmailFixture {
        fn arbitrary<G: quickcheck::Gen>(g: &mut G) -> Self {
            let email = SafeEmail().fake_with_rng(g);
            Self(email)
        }
    }

    #[quickcheck_macros::quickcheck]
    fn valid_emails_are_parsed_successfully(valid_email: ValidEmailFixture) -> bool {
        valid_email.0.parse::<SubscriberEmail>().is_ok()
    }

    #[test]
    fn empty_string_is_rejected() {
        let email = "";
        assert!(email.parse::<SubscriberEmail>().is_err());
    }

    #[test]
    fn email_missing_at_symbol_is_rejected() {
        let email = "ursula.com";
        assert!(email.parse::<SubscriberEmail>().is_err());
    }

    #[test]
    fn email_missing_subject_is_rejected() {
        let email = "@domain.com";
        assert!(email.parse::<SubscriberEmail>().is_err());
    }
}
