//! How the sign-in dialog draws the fields a source asks for
//! (`AuthStep::AskFields`, docs/core-api.md "Signing in"): each field as the
//! plugin describes it (label, kind, secret, required, placeholder, help),
//! as Jellyfesh Web's sign-in sheet does. The core and the plugin check the
//! answers; this only decides the input and when Sign In can be pressed.

use jf_core::{AuthField, AuthFieldKind};

/// The input a field gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    /// One line of text.
    Line(Purpose),
    /// One line, masked (passwords, keys), whatever its kind.
    Secret,
    /// Several lines: a `Cookie:` header pasted from a browser.
    Lines,
}

/// What on-screen keyboards and input methods should offer for a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    Text,
    Email,
    Url,
    /// Digits only, such as a one-time code.
    Digits,
}

/// The input for a field: masked when it is secret, several lines for a
/// cookie header, otherwise a line for its kind.
pub fn input_for(field: &AuthField) -> Input {
    if field.secret {
        return Input::Secret;
    }
    match field.kind {
        AuthFieldKind::CookieHeader => Input::Lines,
        AuthFieldKind::Email => Input::Line(Purpose::Email),
        AuthFieldKind::Url => Input::Line(Purpose::Url),
        AuthFieldKind::Number => Input::Line(Purpose::Digits),
        AuthFieldKind::Text => Input::Line(Purpose::Text),
    }
}

/// Whether Sign In can be pressed: every required field has something in
/// it (spaces alone do not count). Optional fields may stay empty.
pub fn ready<'a>(fields: impl IntoIterator<Item = (&'a AuthField, &'a str)>) -> bool {
    fields
        .into_iter()
        .all(|(field, value)| !field.required || !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(kind: AuthFieldKind, secret: bool, required: bool) -> AuthField {
        AuthField {
            name: "field".into(),
            label: "Field".into(),
            kind,
            secret,
            required,
            placeholder: None,
            help: None,
        }
    }

    #[test]
    fn the_kind_picks_the_input() {
        assert_eq!(
            input_for(&field(AuthFieldKind::Text, false, true)),
            Input::Line(Purpose::Text)
        );
        assert_eq!(
            input_for(&field(AuthFieldKind::Email, false, true)),
            Input::Line(Purpose::Email)
        );
        assert_eq!(
            input_for(&field(AuthFieldKind::Url, false, true)),
            Input::Line(Purpose::Url)
        );
        assert_eq!(
            input_for(&field(AuthFieldKind::Number, false, true)),
            Input::Line(Purpose::Digits)
        );
        assert_eq!(
            input_for(&field(AuthFieldKind::CookieHeader, false, true)),
            Input::Lines
        );
    }

    #[test]
    fn secrets_are_masked_whatever_their_kind() {
        for kind in [
            AuthFieldKind::Text,
            AuthFieldKind::Number,
            AuthFieldKind::CookieHeader,
        ] {
            assert_eq!(input_for(&field(kind, true, true)), Input::Secret);
        }
    }

    #[test]
    fn sign_in_waits_for_the_required_fields_only() {
        let user = field(AuthFieldKind::Text, false, true);
        let password = field(AuthFieldKind::Text, true, true);
        let code = field(AuthFieldKind::Number, false, false);
        assert!(!ready([(&user, ""), (&password, "secret"), (&code, "")]));
        assert!(!ready([(&user, "   "), (&password, "secret"), (&code, "")]));
        assert!(ready([(&user, "listener"), (&password, "secret"), (&code, "")]));
        assert!(ready([(&user, "listener"), (&password, " x "), (&code, "123")]));
        // Nothing to fill in: nothing to wait for.
        assert!(ready([(&code, "")]));
        assert!(ready(std::iter::empty()));
    }
}
