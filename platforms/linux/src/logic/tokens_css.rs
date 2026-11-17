//! Expands `{{group.token}}` placeholders in the stylesheet templates
//! (`data/css/*.css`) with values from `design/tokens.json`.
//!
//! `build.rs` includes this file to generate the app's CSS at build time; the
//! crate compiles it too, so the expansion is unit-tested. It depends only on
//! `serde_json`, which is both a build and a normal dependency.
//!
//! | Token `$type` | Expands to |
//! | --- | --- |
//! | `dimension` | `12px` (rem tokens are converted at 16 px) |
//! | `duration` | `380ms` |
//! | `cubicBezier` | `cubic-bezier(0.22, 1, 0.36, 1)` |
//! | `number` | the number (`{{group.token * 0.9}}`: times a factor) |
//! | `color` | `@jf_<name>`: a named colour the run-time theme defines |
//! | `typography` | `font-size: 14px; line-height: 1.3; font-weight: 500` |
//! | `shadow` | a `box-shadow` list whose colours are run-time theme colours |
//!
//! Theme colours are never baked into the CSS (Misty PLT-03): the app defines
//! every `@jf_*` colour at run time from the core's theme slice.

// build.rs uses all of this; the app itself only needs `color_name` and
// `num` at run time.
#![allow(dead_code)]

use serde_json::Value;

/// Pixels per rem, as in Misty's web tokens.
pub const REM_PX: f64 = 16.0;

/// Replaces every `{{group.token}}` in `template`.
pub fn expand(template: &str, tokens: &Value) -> Result<String, String> {
    let mut out = String::with_capacity(template.len() + template.len() / 4);
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after.find("}}").ok_or_else(|| {
            let near: String = after.chars().take(40).collect();
            format!("unclosed placeholder near `{near}`")
        })?;
        out.push_str(&token_css(after[..end].trim(), tokens)?);
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

/// The CSS for one token, by its `group.token` path.
pub fn token_css(path: &str, tokens: &Value) -> Result<String, String> {
    // A number token may be scaled: `field.aurora-strength * 0.9`.
    if let Some((name, factor)) = path.split_once('*') {
        let name = name.trim();
        let factor: f64 = factor
            .trim()
            .parse()
            .map_err(|_| format!("`{path}`: the factor must be a number"))?;
        let token = lookup(tokens, name).ok_or_else(|| format!("unknown token `{name}`"))?;
        let value = token
            .get("$value")
            .and_then(Value::as_f64)
            .filter(|_| token.get("$type").and_then(Value::as_str) == Some("number"))
            .ok_or_else(|| format!("`{path}`: only number tokens can be scaled"))?;
        return Ok(num(value * factor));
    }
    let token = lookup(tokens, path).ok_or_else(|| format!("unknown token `{path}`"))?;
    let kind = token.get("$type").and_then(Value::as_str).unwrap_or_default();
    let value = token
        .get("$value")
        .ok_or_else(|| format!("token `{path}` has no $value"))?;
    let bad = || format!("token `{path}` ({kind}) has an unexpected value: {value}");
    match kind {
        "dimension" => dimension(value).ok_or_else(bad),
        "duration" => duration(value).ok_or_else(bad),
        "cubicBezier" => bezier(value).ok_or_else(bad),
        "number" => value.as_f64().map(num).ok_or_else(bad),
        "color" => Ok(format!("@{}", color_name(path))),
        "typography" => typography(value).ok_or_else(bad),
        "shadow" => shadow(path, value).ok_or_else(bad),
        other => Err(format!(
            "token `{path}` has type `{other}`, which has no CSS form"
        )),
    }
}

/// The GTK named colour for a theme colour: `ink.ink-90` becomes `jf_ink_90`,
/// `accent.on-accent` becomes `jf_on_accent`, `elevation.shadow-1.0`
/// becomes `jf_shadow_1_0`.
pub fn color_name(path: &str) -> String {
    let name = path.split_once('.').map_or(path, |(_, name)| name);
    let mut out = String::with_capacity(name.len() + 3);
    out.push_str("jf_");
    out.extend(
        name.chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }),
    );
    out
}

fn lookup<'a>(tokens: &'a Value, path: &str) -> Option<&'a Value> {
    let (group, name) = path.split_once('.')?;
    tokens.get(group)?.get(name)
}

/// A number without a trailing `.0`, rounded to 4 decimals.
pub fn num(v: f64) -> String {
    let rounded = (v * 10_000.0).round() / 10_000.0;
    if rounded == rounded.trunc() && rounded.abs() < 1e15 {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded}")
    }
}

fn px(v: &Value) -> Option<f64> {
    let value = v.get("value")?.as_f64()?;
    match v.get("unit")?.as_str()? {
        "px" => Some(value),
        "rem" => Some(value * REM_PX),
        _ => None,
    }
}

fn dimension(v: &Value) -> Option<String> {
    px(v).map(|p| format!("{}px", num(p)))
}

fn duration(v: &Value) -> Option<String> {
    let value = v.get("value")?.as_f64()?;
    match v.get("unit")?.as_str()? {
        "ms" => Some(format!("{}ms", num(value))),
        "s" => Some(format!("{}ms", num(value * 1_000.0))),
        _ => None,
    }
}

fn bezier(v: &Value) -> Option<String> {
    let points: Vec<f64> = v.as_array()?.iter().map(Value::as_f64).collect::<Option<_>>()?;
    let [a, b, c, d] = points.as_slice() else {
        return None;
    };
    Some(format!(
        "cubic-bezier({}, {}, {}, {})",
        num(*a),
        num(*b),
        num(*c),
        num(*d)
    ))
}

fn typography(v: &Value) -> Option<String> {
    let size = px(v.get("fontSize")?)?;
    let line = v.get("lineHeight")?.as_f64()?;
    let weight = v.get("fontWeight")?.as_f64()?;
    Some(format!(
        "font-size: {}px; line-height: {}; font-weight: {}",
        num(size),
        num(line),
        num(weight)
    ))
}

fn shadow(path: &str, v: &Value) -> Option<String> {
    let layers = v.as_array()?;
    let mut parts = Vec::with_capacity(layers.len());
    for (i, layer) in layers.iter().enumerate() {
        let color = match layer.get("color")? {
            // A reference such as "{field.sky}".
            Value::String(r) => {
                let inner = r.strip_prefix('{')?.strip_suffix('}')?;
                format!("@{}", color_name(inner))
            }
            // A themed colour of its own: elevation.shadow-1.0, -1.1, …
            _ => format!("@{}", color_name(&format!("{path}.{i}"))),
        };
        parts.push(format!(
            "{} {} {} {} {color}",
            dimension(layer.get("offsetX")?)?,
            dimension(layer.get("offsetY")?)?,
            dimension(layer.get("blur")?)?,
            dimension(layer.get("spread")?)?,
        ));
    }
    (!parts.is_empty()).then(|| parts.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tokens() -> Value {
        json!({
            "radius": { "r-lg": { "$type": "dimension", "$value": { "value": 0.75, "unit": "rem" } } },
            "player": { "mini-height": { "$type": "dimension", "$value": { "value": 64, "unit": "px" } } },
            "motion": {
                "dur-3": { "$type": "duration", "$value": { "value": 380.0, "unit": "ms" } },
                "ease-fluid": { "$type": "cubicBezier", "$value": [0.22, 1.0, 0.36, 1.0] }
            },
            "ink": { "ink-90": { "$type": "color", "$value": { "hex": "#101820" } } },
            "accent": { "on-accent": { "$type": "color", "$value": { "hex": "#f9fcff" } } },
            "glass": { "glass-sat": { "$type": "number", "$value": 1.75 } },
            "type-roles": { "list-title": { "$type": "typography",
                "$value": { "fontSize": { "value": 14, "unit": "px" }, "lineHeight": 1.3, "fontWeight": 500 } } },
            "elevation": {
                "shadow-1": { "$type": "shadow", "$value": [
                    { "color": { "hex": "#1e2f41" }, "offsetX": { "value": 0.0, "unit": "px" },
                      "offsetY": { "value": 1.0, "unit": "px" }, "blur": { "value": 2.0, "unit": "px" },
                      "spread": { "value": 0.0, "unit": "px" } },
                    { "color": { "hex": "#142f4b" }, "offsetX": { "value": 0.0, "unit": "px" },
                      "offsetY": { "value": 10.0, "unit": "px" }, "blur": { "value": 28.0, "unit": "px" },
                      "spread": { "value": -18.0, "unit": "px" } } ] },
                "focus": { "$type": "shadow", "$value": [
                    { "color": "{field.sky}", "offsetX": { "value": 0.0, "unit": "px" },
                      "offsetY": { "value": 0.0, "unit": "px" }, "blur": { "value": 0.0, "unit": "px" },
                      "spread": { "value": 2.0, "unit": "px" } } ] }
            },
            "brand": { "mood": { "$type": "string", "$value": "calm" } }
        })
    }

    #[test]
    fn scales_number_tokens() {
        let t = tokens();
        assert_eq!(expand("{{glass.glass-sat * 2}}", &t).unwrap(), "3.5");
        assert_eq!(expand("{{ glass.glass-sat*0.5 }}", &t).unwrap(), "0.875");
        assert!(
            expand("{{radius.r-lg * 2}}", &t).is_err(),
            "dimensions are not scaled"
        );
        assert!(expand("{{glass.glass-sat * x}}", &t).is_err());
        assert!(expand("{{glass.nope * 2}}", &t).is_err());
    }

    #[test]
    fn expands_every_kind() {
        let t = tokens();
        let css = expand(
            ".a { border-radius: {{radius.r-lg}}; min-height: {{ player.mini-height }}; \
             transition: opacity {{motion.dur-3}} {{motion.ease-fluid}}; color: {{ink.ink-90}}; \
             {{type-roles.list-title}}; box-shadow: {{elevation.shadow-1}}; }",
            &t,
        )
        .unwrap();
        assert_eq!(
            css,
            ".a { border-radius: 12px; min-height: 64px; \
             transition: opacity 380ms cubic-bezier(0.22, 1, 0.36, 1); color: @jf_ink_90; \
             font-size: 14px; line-height: 1.3; font-weight: 500; \
             box-shadow: 0px 1px 2px 0px @jf_shadow_1_0, 0px 10px 28px -18px @jf_shadow_1_1; }"
        );
        assert_eq!(token_css("glass.glass-sat", &t).unwrap(), "1.75");
        assert_eq!(
            token_css("elevation.focus", &t).unwrap(),
            "0px 0px 0px 2px @jf_sky"
        );
    }

    #[test]
    fn reports_mistakes() {
        let t = tokens();
        assert!(expand("{{radius.r-nope}}", &t).unwrap_err().contains("r-nope"));
        assert!(expand("{{radius.r-lg", &t).unwrap_err().contains("unclosed"));
        assert!(expand("{{brand.mood}}", &t).unwrap_err().contains("no CSS form"));
        assert_eq!(expand("no placeholders", &t).unwrap(), "no placeholders");
    }

    #[test]
    fn names_colours_for_gtk() {
        assert_eq!(color_name("ink.ink-90"), "jf_ink_90");
        assert_eq!(color_name("accent.on-accent"), "jf_on_accent");
        assert_eq!(color_name("elevation.shadow-1.0"), "jf_shadow_1_0");
        assert_eq!(color_name("glass.tint-veil"), "jf_tint_veil");
    }

    #[test]
    fn formats_numbers_compactly() {
        assert_eq!(num(12.0), "12");
        assert_eq!(num(0.22), "0.22");
        assert_eq!(num(-18.0), "-18");
        assert_eq!(num(1.0 / 3.0), "0.3333");
    }

    #[test]
    fn the_real_token_file_expands() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/tokens.json");
        let tokens: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        for key in [
            "radius.r-full",
            "space.s4",
            "motion.dur-4",
            "motion.ease-spring",
            "elevation.shadow-2",
            "lyrics.line",
            "player.mini-height",
            "glass.glass-blur-veil",
        ] {
            assert!(token_css(key, &tokens).is_ok(), "{key}");
        }
    }
}
