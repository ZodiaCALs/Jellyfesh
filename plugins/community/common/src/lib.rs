//! Helpers shared by the unofficial NetEase Cloud Music and QQ Music plugins.
//!
//! Everything here is plain Rust without host calls, so it is tested on the
//! build machine with `cargo test`:
//!
//! * [`base64`]: the standard alphabet, which both services use;
//! * [`lrc`]: reading LRC lyrics, merging a translation in as bilingual
//!   lines (the same time stamp twice, which Jellyfesh shows as a
//!   translation), writing them back;
//! * [`entities`]: HTML character references, which some lyrics carry;
//! * [`paging`]: offsets as cursors, slicing whole lists into pages;
//! * [`quality`]: the "Streaming Quality" setting both plugins offer;
//! * [`matching`]: finding a service's own track for an item that came from
//!   another source (lyrics and artwork are asked for those too).

pub mod base64;
pub mod entities;
pub mod lrc;
pub mod matching;
pub mod paging;
pub mod quality;

/// Joins artist names the way Jellyfesh reads them: the first name is the
/// primary artist, and `,` separates the others. Empty names are skipped;
/// none at all gives `None`.
pub fn join_artists<'a>(names: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let names: Vec<&str> = names
        .into_iter()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .collect();
    (!names.is_empty()).then(|| names.join(", "))
}

/// `text` without surrounding white space, or `None` when nothing is left.
pub fn non_empty(text: Option<&str>) -> Option<String> {
    text.map(str::trim).filter(|t| !t.is_empty()).map(str::to_string)
}

/// `http://…` becomes `https://…`; other URLs are returned unchanged. Both
/// services still hand out some `http` links to hosts that also answer over
/// https, and the host only fetches fixed hosts over https.
pub fn upgrade_to_https(url: &str) -> String {
    match url.strip_prefix("http://") {
        Some(rest) => format!("https://{rest}"),
        None => url.to_string(),
    }
}

/// The host name of an `http` or `https` URL, lowercase, without port.
pub fn url_host(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.contains('@') {
        return None;
    }
    let host = authority.split(':').next()?.trim_end_matches('.');
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// The calendar year of an instant given in ms since the epoch, in a time
/// zone `utc_offset_hours` east of UTC (both services date releases in China
/// time, UTC+8). `None` for 0 and earlier, which the services use for
/// "unknown".
pub fn year_from_epoch_ms(ms: i64, utc_offset_hours: i64) -> Option<u32> {
    if ms <= 0 {
        return None;
    }
    let days = (ms + utc_offset_hours * 3_600_000).div_euclid(86_400_000);
    // Days since 1970-01-01 to a civil date (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    u32::try_from(year).ok().filter(|y| *y > 0)
}

/// The year at the start of a date written as text (`2020-01-01`, `2020`).
pub fn year_from_text(text: &str) -> Option<u32> {
    let digits: String = text.trim().chars().take_while(char::is_ascii_digit).collect();
    if digits.len() != 4 {
        return None;
    }
    digits.parse().ok().filter(|y| *y > 0)
}

/// Whether `host` is `domain` itself or one of its subdomains.
pub fn host_in(host: &str, domain: &str) -> bool {
    host == domain
        || (host.len() > domain.len()
            && host.ends_with(domain)
            && host.as_bytes()[host.len() - domain.len() - 1] == b'.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_artists() {
        assert_eq!(join_artists(["A", " B ", ""]), Some("A, B".into()));
        assert_eq!(join_artists(Vec::<&str>::new()), None);
        assert_eq!(join_artists([" "]), None);
    }

    #[test]
    fn reads_hosts_and_upgrades() {
        assert_eq!(
            upgrade_to_https("http://p1.music.126.net/a.jpg"),
            "https://p1.music.126.net/a.jpg"
        );
        assert_eq!(upgrade_to_https("https://x.example/a"), "https://x.example/a");
        assert_eq!(
            url_host("https://M701.music.126.net:443/a?b").as_deref(),
            Some("m701.music.126.net")
        );
        assert_eq!(url_host("http://y.gtimg.cn").as_deref(), Some("y.gtimg.cn"));
        assert_eq!(url_host("https://user@evil.example/"), None);
        assert_eq!(url_host("ftp://x.example/"), None);
        assert!(host_in("m701.music.126.net", "music.126.net"));
        assert!(host_in("music.126.net", "music.126.net"));
        assert!(!host_in("evilmusic.126.net", "music.126.net"));
        assert!(!host_in("music.126.net.evil.example", "music.126.net"));
    }

    #[test]
    fn reads_years() {
        // 2020-01-01 00:00 in China is still 2019 in UTC.
        let new_year_china = 1_577_808_000_000; // 2019-12-31T16:00:00Z
        assert_eq!(year_from_epoch_ms(new_year_china, 8), Some(2020));
        assert_eq!(year_from_epoch_ms(new_year_china, 0), Some(2019));
        assert_eq!(year_from_epoch_ms(951_782_400_000, 0), Some(2000), "2000-02-29");
        assert_eq!(year_from_epoch_ms(0, 8), None);
        assert_eq!(year_from_epoch_ms(-5, 8), None);
        assert_eq!(year_from_text("2020-01-01"), Some(2020));
        assert_eq!(year_from_text(" 1999"), Some(1999));
        assert_eq!(year_from_text("20-01-01"), None);
        assert_eq!(year_from_text(""), None);
    }

    #[test]
    fn trims_text() {
        assert_eq!(non_empty(Some("  a ")), Some("a".into()));
        assert_eq!(non_empty(Some("  ")), None);
        assert_eq!(non_empty(None), None);
    }
}
