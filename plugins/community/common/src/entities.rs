//! HTML character references in text the services send (QQ Music escapes
//! some lyrics and titles this way).

/// Replaces `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`, `&nbsp;` and numeric
/// references (`&#39;`, `&#x27;`) with the characters they stand for. Anything
/// else that starts with `&` is left as it is.
pub fn decode(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        match tail.find(';').filter(|end| *end <= 10) {
            Some(end) => match character(&tail[1..end]) {
                Some(c) => {
                    out.push(c);
                    rest = &tail[end + 1..];
                }
                None => {
                    out.push('&');
                    rest = &tail[1..];
                }
            },
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn character(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        _ => {
            let number = name.strip_prefix('#')?;
            let code = match number.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => number.parse::<u32>().ok()?,
            };
            char::from_u32(code).filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_references() {
        assert_eq!(decode("Rock &amp; Roll"), "Rock & Roll");
        assert_eq!(decode("It&apos;s &#39;fine&#x27;"), "It's 'fine'");
        assert_eq!(decode("[00:01.00]&#58;)"), "[00:01.00]:)");
        assert_eq!(decode("&lt;tag&gt; &quot;q&quot;"), "<tag> \"q\"");
        assert_eq!(decode("no references"), "no references");
    }

    #[test]
    fn leaves_other_ampersands() {
        assert_eq!(decode("A & B"), "A & B");
        assert_eq!(decode("&unknown; &#xZZ; &"), "&unknown; &#xZZ; &");
        assert_eq!(decode("&#0;"), "&#0;");
        assert_eq!(decode("tail &amp"), "tail &amp");
    }
}
