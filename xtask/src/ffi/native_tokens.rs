//! `cargo xtask tokens --target swift|kotlin|csharp`: the design tokens as constants for the
//! native shells, from `design/tokens.json` (the same source and rules as the Rust module that
//! `tokens.rs` writes).
//!
//! | Platform | File                                                                   |
//! | -------- | ---------------------------------------------------------------------- |
//! | Swift    | `platforms/macos/Generated/Tokens.swift` (`enum Tokens`)               |
//! | Kotlin   | `…/core-bindings/src/main/kotlin/org/jellyfesh/tokens/Tokens.kt`       |
//! | C#       | `platforms/windows/Generated/Tokens.cs` (`Jellyfesh.Design.Tokens`)    |
//! | CSS      | `platforms/web/generated/tokens.css` (custom properties, Jellyfesh Web) |
//!
//! Each token group becomes a nested type (`type` becomes `TypeScale`, which every language
//! accepts) and each token a constant. Dimensions are the token file's px as points (Apple),
//! dp (Android) or effective pixels (Windows); durations are milliseconds (`…Ms`). Colours are
//! not baked (Misty PLT-03): a colour token is the *key* of that colour in the core's theme
//! snapshot (`ThemeSnapshot.light/dark.colors`), which resolves every colour at run time from one
//! hue. Shadows and glass grades carry those keys too. In CSS a colour is a custom property
//! (`--jf-ink-90`) the page sets from the core's theme, and the static tokens are custom properties
//! of `:root` (dark values under `:root[data-appearance="dark"]`).

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Map, Value};

use super::Lang;

const SOURCE: &str = "design/tokens.json";
const EXT: &str = "org.jellyfesh";
const PX_PER_REM: f64 = 16.0;

/// Writes one platform's token file.
pub fn run(root: &Path, lang: Lang) -> Result<()> {
    let source = std::fs::read_to_string(root.join(SOURCE)).with_context(|| format!("reading {SOURCE}"))?;
    let json: Value = serde_json::from_str(&source).with_context(|| format!("parsing {SOURCE}"))?;
    let tokens = Tokens::parse(&json)?;
    let (relative, code) = match lang {
        Lang::Swift => ("Tokens.swift", swift(&tokens)?),
        Lang::Kotlin => ("org/jellyfesh/tokens/Tokens.kt", kotlin(&tokens)?),
        Lang::CSharp => ("Tokens.cs", csharp(&tokens)?),
        Lang::Css => ("tokens.css", css(&tokens)?),
    };
    let path = root.join(lang.generated_dir()).join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    std::fs::write(&path, code).with_context(|| format!("writing {}", path.display()))?;
    println!("wrote {}/{relative}", lang.generated_dir());
    Ok(())
}

// ---- the language-neutral model -------------------------------------------------------------

/// The token file, parsed.
struct Tokens {
    version: String,
    default_hue: f64,
    groups: Vec<Group>,
}

struct Group {
    key: String,
    doc: Option<String>,
    tokens: Vec<Token>,
}

struct Token {
    /// The name in the token file (`ink-90`); a suffix such as `Ms` is added per kind.
    key: String,
    doc: Option<String>,
    value: Kind,
}

enum Kind {
    Number(f64),
    Numbers(Vec<f64>),
    Themed(f64, f64),
    /// A dimension in px.
    Px(f64),
    /// The fluid range of the token before it: min and max in px, preferred size in vw.
    Fluid {
        min: f64,
        max: f64,
        vw: Option<f64>,
    },
    Ms(u64),
    Bezier([f64; 4]),
    Text(String),
    Fonts(Vec<String>),
    Typography {
        size: f64,
        line_height: f64,
        weight: u64,
    },
    /// The key of a colour in the theme snapshot (`ink.ink-90`).
    Color(String),
    Shadow {
        light: Vec<Layer>,
        dark: Vec<Layer>,
    },
    Glass {
        grade: String,
        light: f64,
        dark: f64,
        blur: f64,
    },
    /// The glass grade constants, clearest first.
    Grades(Vec<String>),
}

struct Layer {
    x: f64,
    y: f64,
    blur: f64,
    spread: f64,
    color: String,
}

impl Tokens {
    fn parse(json: &Value) -> Result<Tokens> {
        let root = json.as_object().context("the token file must be a JSON object")?;
        let version = root
            .get("$extensions")
            .and_then(|e| e.get(EXT))
            .and_then(|e| e.get("version"))
            .and_then(Value::as_str)
            .unwrap_or("unversioned")
            .to_owned();
        let default_hue = root
            .get("parameter")
            .and_then(|g| g.get("hue"))
            .and_then(|t| t.get("$value"))
            .and_then(Value::as_f64)
            .context("parameter.hue is required: it is the default theme hue")?;
        let mut groups = Vec::new();
        for (key, node) in entries(root) {
            let group = node
                .as_object()
                .with_context(|| format!("{key}: expected a group"))?;
            groups.push(Group::parse(key, group)?);
        }
        Ok(Tokens {
            version,
            default_hue,
            groups,
        })
    }
}

impl Group {
    fn parse(key: &str, group: &Map<String, Value>) -> Result<Group> {
        let group_type = group.get("$type").and_then(Value::as_str);
        let mut tokens = Vec::new();
        for (name, node) in entries(group) {
            let path = format!("{key}.{name}");
            let token = node
                .as_object()
                .with_context(|| format!("{path}: expected an object"))?;
            ensure!(
                token.contains_key("$value"),
                "{path}: nested groups are not supported"
            );
            tokens.extend(parse_token(key, name, token, group_type)?);
        }
        tokens.extend(glass_grades(key, group)?);
        Ok(Group {
            key: key.to_owned(),
            doc: group.get("$description").and_then(Value::as_str).map(one_line),
            tokens,
        })
    }
}

/// One token, and its fluid range when it has one.
fn parse_token(
    group: &str,
    key: &str,
    token: &Map<String, Value>,
    group_type: Option<&str>,
) -> Result<Vec<Token>> {
    let path = format!("{group}.{key}");
    let ty = token
        .get("$type")
        .and_then(Value::as_str)
        .or(group_type)
        .with_context(|| format!("{path}: no $type"))?;
    let value = &token["$value"];
    let ext = token
        .get("$extensions")
        .and_then(|e| e.get(EXT))
        .and_then(Value::as_object);
    let ext_get = |name: &str| ext.and_then(|e| e.get(name));
    let themed = ext_get("themed").and_then(Value::as_bool).unwrap_or(false);
    let doc = token.get("$description").and_then(Value::as_str).map(one_line);
    let token = |value: Kind| Token {
        key: key.to_owned(),
        doc: doc.clone(),
        value,
    };
    let mut out = Vec::new();
    match ty {
        "color" => out.push(token(Kind::Color(path.clone()))),
        "number" => out.push(token(match (value, ext_get("dark")) {
            (Value::Array(items), _) => {
                Kind::Numbers(items.iter().map(|v| number(v, &path)).collect::<Result<_>>()?)
            }
            (_, Some(dark)) if themed => Kind::Themed(number(value, &path)?, number(dark, &path)?),
            _ => Kind::Number(number(value, &path)?),
        })),
        "dimension" => {
            let (px, scale) = dimension(value, &path)?;
            out.push(token(Kind::Px(px)));
            out.extend(fluid(key, ext_get("fluid"), scale, &path)?);
        }
        "duration" => {
            let (amount, unit) = value_unit(value, &path)?;
            let ms = match unit {
                "ms" => amount,
                "s" => amount * 1000.0,
                other => bail!("{path}: unsupported duration unit {other:?}"),
            };
            ensure!(
                ms >= 0.0 && ms.fract() == 0.0,
                "{path}: a duration must be a whole number of milliseconds"
            );
            out.push(token(Kind::Ms(ms as u64)));
        }
        "cubicBezier" => {
            let points = value
                .as_array()
                .filter(|p| p.len() == 4)
                .with_context(|| format!("{path}: expected four numbers"))?;
            let p = |i: usize| number(&points[i], &path);
            out.push(token(Kind::Bezier([p(0)?, p(1)?, p(2)?, p(3)?])));
        }
        "fontFamily" => {
            let faces = match value {
                Value::String(face) => vec![face.clone()],
                Value::Array(faces) => faces
                    .iter()
                    .map(|f| f.as_str().map(str::to_owned))
                    .collect::<Option<Vec<_>>>()
                    .with_context(|| format!("{path}: expected a list of strings"))?,
                _ => bail!("{path}: expected a string or a list of strings"),
            };
            out.push(token(Kind::Fonts(faces)));
        }
        "string" => {
            let text = value
                .as_str()
                .with_context(|| format!("{path}: expected a string"))?;
            out.push(token(Kind::Text(text.to_owned())));
        }
        "typography" => {
            let size = value
                .get("fontSize")
                .with_context(|| format!("{path}: no fontSize"))?;
            let (size, scale) = dimension(size, &path)?;
            let line_height = number(value.get("lineHeight").unwrap_or(&Value::Null), &path)?;
            let weight = value
                .get("fontWeight")
                .and_then(Value::as_u64)
                .filter(|w| (1..=1000).contains(w))
                .with_context(|| format!("{path}: fontWeight must be 1 to 1000"))?;
            out.push(token(Kind::Typography {
                size,
                line_height,
                weight,
            }));
            out.extend(fluid(key, ext_get("fluid"), scale, &path)?);
        }
        "shadow" => {
            let ext = ext
                .filter(|_| themed)
                .with_context(|| format!("{path}: only themed shadows are supported"))?;
            let layers = |appearance: &str| -> Result<Vec<Layer>> {
                let defs = ext
                    .get(appearance)
                    .and_then(Value::as_array)
                    .with_context(|| format!("{path}: expected a list of {appearance} layers"))?;
                defs.iter()
                    .enumerate()
                    .map(|(i, layer)| shadow_layer(&path, value, i, layer))
                    .collect()
            };
            let (light, dark) = (layers("light")?, layers("dark")?);
            ensure!(
                light.len() == dark.len(),
                "{path}: light and dark need the same number of layers"
            );
            // Layers with their own colour add it to the theme, named `<shadow>.<i>`.
            for (i, layer) in light.iter().enumerate() {
                if layer.color == format!("{path}.{i}") {
                    out.push(Token {
                        key: format!("{key}-layer-{i}"),
                        doc: Some(format!("Colour of layer {i} of {key}.")),
                        value: Kind::Color(layer.color.clone()),
                    });
                }
            }
            out.push(token(Kind::Shadow { light, dark }));
        }
        other => bail!("{path}: unsupported $type {other:?}"),
    }
    Ok(out)
}

fn shadow_layer(path: &str, value: &Value, i: usize, layer: &Value) -> Result<Layer> {
    let field = |key: &str| number(layer.get(key).unwrap_or(&Value::Null), path);
    let color = if layer.get("color").is_some() {
        format!("{path}.{i}")
    } else {
        value
            .get(i)
            .and_then(|l| l.get("color"))
            .and_then(Value::as_str)
            .and_then(|alias| alias.strip_prefix('{')?.strip_suffix('}'))
            .with_context(|| format!("{path}: layer {i} needs a colour or a {{group.token}} alias"))?
            .to_owned()
    };
    Ok(Layer {
        x: field("x")?,
        y: field("y")?,
        blur: field("blur")?,
        spread: field("spread")?,
        color,
    })
}

fn fluid(key: &str, fluid: Option<&Value>, scale: f64, path: &str) -> Result<Option<Token>> {
    let Some(fluid) = fluid else { return Ok(None) };
    let field = |name: &str| -> Result<f64> {
        number(
            fluid
                .get(name)
                .with_context(|| format!("{path}: fluid needs {name}"))?,
            path,
        )
    };
    let vw = fluid.get("vw").map(|v| number(v, path)).transpose()?;
    let mut doc = format!("Fluid range of {key} (px); vw is its preferred size in % of the viewport width.");
    if let Some(basis) = fluid.get("basis").and_then(Value::as_str) {
        write!(doc, " Scales with the {basis}.")?;
    }
    Ok(Some(Token {
        key: format!("{key}-fluid"),
        doc: Some(doc),
        value: Kind::Fluid {
            min: field("min")? * scale,
            max: field("max")? * scale,
            vw,
        },
    }))
}

/// Glass grades (Clear, Mist, Veil) and their tint colours, as `tokens.rs` derives them.
fn glass_grades(group: &str, tokens: &Map<String, Value>) -> Result<Vec<Token>> {
    if !(tokens.contains_key("glass-l") && tokens.contains_key("glass-c")) {
        return Ok(Vec::new());
    }
    let mut grades = Vec::new();
    for (key, token) in entries(tokens) {
        let Some(suffix) = key.strip_prefix("glass-alpha") else {
            continue;
        };
        let grade = match suffix {
            "" => "mist",
            _ => match suffix.strip_prefix('-') {
                Some(grade) => grade,
                None => continue,
            },
        };
        let path = format!("{group}.{key}");
        let light = number(token.get("$value").unwrap_or(&Value::Null), &path)?;
        let dark = token
            .get("$extensions")
            .and_then(|e| e.get(EXT))
            .and_then(|e| e.get("dark"))
            .map(|d| number(d, &path))
            .transpose()?
            .unwrap_or(light);
        let blur_key = format!("glass-blur{suffix}");
        let blur = tokens
            .get(&blur_key)
            .and_then(|t| t.get("$value"))
            .with_context(|| format!("{path}: no {blur_key}"))?;
        let (blur, _) = dimension(blur, &blur_key)?;
        grades.push((light, dark, blur, grade.to_owned()));
    }
    grades.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.3.cmp(&b.3)));
    let mut out = Vec::new();
    for (light, dark, blur, grade) in &grades {
        out.push(Token {
            key: grade.clone(),
            doc: Some(format!(
                "The {grade} glass grade: tint alpha and blur radius (px)."
            )),
            value: Kind::Glass {
                grade: grade.clone(),
                light: *light,
                dark: *dark,
                blur: *blur,
            },
        });
    }
    if !grades.is_empty() {
        out.push(Token {
            key: "grades".into(),
            doc: Some("Every glass grade, clearest first.".into()),
            value: Kind::Grades(grades.iter().map(|g| g.3.clone()).collect()),
        });
    }
    for (_, _, _, grade) in &grades {
        out.push(Token {
            key: format!("tint-{grade}"),
            doc: Some(format!("Tint of {grade} glass, resolved at the theme hue.")),
            value: Kind::Color(format!("{group}.tint-{grade}")),
        });
    }
    Ok(out)
}

// ---- emitters -------------------------------------------------------------------------------

fn header(comment: &str, target: &str) -> String {
    format!(
        "{comment} @generated by `cargo xtask tokens --target {target}` from `{SOURCE}`. Do not edit by hand.\n\
         {comment}\n\
         {comment} Dimensions are the token file's px as density-independent units (pt, dp, epx);\n\
         {comment} durations are milliseconds; curves are cubic-bezier control points. Colours are\n\
         {comment} keys into the core's theme snapshot (ThemeSnapshot.light/dark.colors), which\n\
         {comment} resolves them at run time from one hue (Misty PLT-03).\n"
    )
}

fn swift(tokens: &Tokens) -> Result<String> {
    let mut out = header("//", "swift");
    out += "\nimport CoreGraphics\n\n";
    out += "/// Design tokens from `design/tokens.json`.\npublic enum Tokens {\n";
    writeln!(out, "    public static let version = {:?}", tokens.version)?;
    writeln!(
        out,
        "    /// Hue the theme uses before an artwork hue is known.\n    public static let defaultHue: Double = {}",
        float(tokens.default_hue)
    )?;
    for group in &tokens.groups {
        writeln!(out)?;
        doc_lines(&mut out, "    ///", group.doc.as_deref());
        writeln!(out, "    public enum {} {{", type_name(&group.key))?;
        let mut names = BTreeSet::new();
        for token in &group.tokens {
            let name = unique(&mut names, &group.key, camel(&member(token)))?;
            doc_lines(&mut out, "        ///", token.doc.as_deref());
            let (ty, value) = match &token.value {
                Kind::Number(n) => ("Double".to_owned(), float(*n)),
                Kind::Numbers(v) => ("[Double]".to_owned(), list(v.iter().map(|n| float(*n)))),
                Kind::Themed(l, d) => (
                    "Themed".into(),
                    format!("Themed(light: {}, dark: {})", float(*l), float(*d)),
                ),
                Kind::Px(px) => ("CGFloat".into(), float(*px)),
                Kind::Fluid { min, max, vw } => (
                    "Fluid".into(),
                    format!(
                        "Fluid(min: {}, max: {}, vw: {})",
                        float(*min),
                        float(*max),
                        vw.map_or("nil".into(), float)
                    ),
                ),
                Kind::Ms(ms) => ("Int".into(), ms.to_string()),
                Kind::Bezier([a, b, c, d]) => (
                    "CubicBezier".into(),
                    format!(
                        "CubicBezier({}, {}, {}, {})",
                        float(*a),
                        float(*b),
                        float(*c),
                        float(*d)
                    ),
                ),
                Kind::Text(text) => ("String".into(), format!("{text:?}")),
                Kind::Fonts(faces) => ("[String]".into(), list(faces.iter().map(|f| format!("{f:?}")))),
                Kind::Typography {
                    size,
                    line_height,
                    weight,
                } => (
                    "Typography".into(),
                    format!(
                        "Typography(size: {}, lineHeight: {}, weight: {weight})",
                        float(*size),
                        float(*line_height)
                    ),
                ),
                Kind::Color(key) => ("ThemeColorKey".into(), format!("ThemeColorKey({key:?})")),
                Kind::Shadow { light, dark } => {
                    let layers = |layers: &[Layer]| {
                        list(layers.iter().map(|l| {
                            format!(
                                "ShadowLayer(x: {}, y: {}, blur: {}, spread: {}, color: ThemeColorKey({:?}))",
                                float(l.x),
                                float(l.y),
                                float(l.blur),
                                float(l.spread),
                                l.color
                            )
                        }))
                    };
                    (
                        "Shadow".into(),
                        format!("Shadow(light: {}, dark: {})", layers(light), layers(dark)),
                    )
                }
                Kind::Glass {
                    grade,
                    light,
                    dark,
                    blur,
                } => (
                    "GlassGrade".into(),
                    format!(
                        "GlassGrade(name: {grade:?}, alpha: Themed(light: {}, dark: {}), blur: {})",
                        float(*light),
                        float(*dark),
                        float(*blur)
                    ),
                ),
                Kind::Grades(grades) => ("[GlassGrade]".into(), list(grades.iter().map(|g| camel(g)))),
            };
            writeln!(out, "        public static let {name}: {ty} = {value}")?;
        }
        writeln!(out, "    }}")?;
    }
    out += "}\n\n";
    out += "/// The key of a colour in `Theme.colors` (for example `ink.ink-90`).\n\
            public struct ThemeColorKey: Hashable, Sendable {\n    public let name: String\n    \
            public init(_ name: String) { self.name = name }\n}\n\n\
            /// A value with a light and a dark appearance.\n\
            public struct Themed: Hashable, Sendable {\n    public let light: Double\n    public let dark: Double\n}\n\n\
            /// Cubic-bezier control points (x1, y1, x2, y2), for `Animation.timingCurve`.\n\
            public struct CubicBezier: Hashable, Sendable {\n    public let x1: Double, y1: Double, x2: Double, y2: Double\n    \
            public init(_ x1: Double, _ y1: Double, _ x2: Double, _ y2: Double) {\n        \
            (self.x1, self.y1, self.x2, self.y2) = (x1, y1, x2, y2)\n    }\n}\n\n\
            /// A type role: size (pt), line height (a multiple of the size) and weight (1 to 1000).\n\
            public struct Typography: Hashable, Sendable {\n    public let size: CGFloat\n    public let lineHeight: Double\n    public let weight: Int\n}\n\n\
            /// A fluid size range (pt); `vw` is the preferred size in % of the viewport width.\n\
            public struct Fluid: Hashable, Sendable {\n    public let min: CGFloat\n    public let max: CGFloat\n    public let vw: Double?\n}\n\n\
            /// One shadow layer; its colour comes from the theme.\n\
            public struct ShadowLayer: Hashable, Sendable {\n    public let x: CGFloat, y: CGFloat, blur: CGFloat, spread: CGFloat\n    public let color: ThemeColorKey\n}\n\n\
            /// A shadow's layers in each appearance.\n\
            public struct Shadow: Hashable, Sendable {\n    public let light: [ShadowLayer]\n    public let dark: [ShadowLayer]\n}\n\n\
            /// A glass grade: tint alpha per appearance and blur radius (pt).\n\
            public struct GlassGrade: Hashable, Sendable {\n    public let name: String\n    public let alpha: Themed\n    public let blur: CGFloat\n}\n";
    Ok(out)
}

fn kotlin(tokens: &Tokens) -> Result<String> {
    let mut out = header("//", "kotlin");
    out += "\npackage org.jellyfesh.tokens\n\n";
    out += "/** Design tokens from `design/tokens.json`. */\nobject Tokens {\n";
    writeln!(out, "    const val version: String = {:?}", tokens.version)?;
    writeln!(
        out,
        "    /** Hue the theme uses before an artwork hue is known. */\n    const val defaultHue: Float = {}",
        kfloat(tokens.default_hue)
    )?;
    for group in &tokens.groups {
        writeln!(out)?;
        doc_block(&mut out, "    ", group.doc.as_deref());
        writeln!(out, "    object {} {{", type_name(&group.key))?;
        let mut names = BTreeSet::new();
        for token in &group.tokens {
            let name = unique(&mut names, &group.key, camel(&member(token)))?;
            doc_block(&mut out, "        ", token.doc.as_deref());
            let (keyword, ty, value) = match &token.value {
                Kind::Number(n) => ("const val", "Float".to_owned(), kfloat(*n)),
                Kind::Numbers(v) => (
                    "val",
                    "List<Float>".into(),
                    format!(
                        "listOf({})",
                        v.iter().map(|n| kfloat(*n)).collect::<Vec<_>>().join(", ")
                    ),
                ),
                Kind::Themed(l, d) => (
                    "val",
                    "Themed".into(),
                    format!("Themed({}, {})", kfloat(*l), kfloat(*d)),
                ),
                Kind::Px(px) => ("const val", "Float".into(), kfloat(*px)),
                Kind::Fluid { min, max, vw } => (
                    "val",
                    "Fluid".into(),
                    format!(
                        "Fluid({}, {}, {})",
                        kfloat(*min),
                        kfloat(*max),
                        vw.map_or("null".into(), kfloat)
                    ),
                ),
                Kind::Ms(ms) => ("const val", "Int".into(), ms.to_string()),
                Kind::Bezier([a, b, c, d]) => (
                    "val",
                    "CubicBezier".into(),
                    format!(
                        "CubicBezier({}, {}, {}, {})",
                        kfloat(*a),
                        kfloat(*b),
                        kfloat(*c),
                        kfloat(*d)
                    ),
                ),
                Kind::Text(text) => ("const val", "String".into(), kstring(text)),
                Kind::Fonts(faces) => (
                    "val",
                    "List<String>".into(),
                    format!(
                        "listOf({})",
                        faces.iter().map(|f| kstring(f)).collect::<Vec<_>>().join(", ")
                    ),
                ),
                Kind::Typography {
                    size,
                    line_height,
                    weight,
                } => (
                    "val",
                    "Typography".into(),
                    format!(
                        "Typography({}, {}, {weight})",
                        kfloat(*size),
                        kfloat(*line_height)
                    ),
                ),
                Kind::Color(key) => (
                    "val",
                    "ThemeColorKey".into(),
                    format!("ThemeColorKey({})", kstring(key)),
                ),
                Kind::Shadow { light, dark } => {
                    let layers = |layers: &[Layer]| {
                        let items: Vec<String> = layers
                            .iter()
                            .map(|l| {
                                format!(
                                    "ShadowLayer({}, {}, {}, {}, ThemeColorKey({}))",
                                    kfloat(l.x),
                                    kfloat(l.y),
                                    kfloat(l.blur),
                                    kfloat(l.spread),
                                    kstring(&l.color)
                                )
                            })
                            .collect();
                        format!("listOf({})", items.join(", "))
                    };
                    (
                        "val",
                        "Shadow".into(),
                        format!("Shadow({}, {})", layers(light), layers(dark)),
                    )
                }
                Kind::Glass {
                    grade,
                    light,
                    dark,
                    blur,
                } => (
                    "val",
                    "GlassGrade".into(),
                    format!(
                        "GlassGrade({}, Themed({}, {}), {})",
                        kstring(grade),
                        kfloat(*light),
                        kfloat(*dark),
                        kfloat(*blur)
                    ),
                ),
                Kind::Grades(grades) => (
                    "val",
                    "List<GlassGrade>".into(),
                    format!(
                        "listOf({})",
                        grades.iter().map(|g| camel(g)).collect::<Vec<_>>().join(", ")
                    ),
                ),
            };
            writeln!(out, "        {keyword} {name}: {ty} = {value}")?;
        }
        writeln!(out, "    }}")?;
    }
    out += "}\n\n";
    out += "/** The key of a colour in `Theme.colors` (for example `ink.ink-90`). */\n\
            @JvmInline\nvalue class ThemeColorKey(val name: String)\n\n\
            /** A value with a light and a dark appearance. */\n\
            data class Themed(val light: Float, val dark: Float)\n\n\
            /** Cubic-bezier control points, as for `CubicBezierEasing(x1, y1, x2, y2)`. */\n\
            data class CubicBezier(val x1: Float, val y1: Float, val x2: Float, val y2: Float)\n\n\
            /** A type role: size (dp), line height (a multiple of the size) and weight (1 to 1000). */\n\
            data class Typography(val size: Float, val lineHeight: Float, val weight: Int)\n\n\
            /** A fluid size range (dp); `vw` is the preferred size in % of the viewport width. */\n\
            data class Fluid(val min: Float, val max: Float, val vw: Float?)\n\n\
            /** One shadow layer; its colour comes from the theme. */\n\
            data class ShadowLayer(val x: Float, val y: Float, val blur: Float, val spread: Float, val color: ThemeColorKey)\n\n\
            /** A shadow's layers in each appearance. */\n\
            data class Shadow(val light: List<ShadowLayer>, val dark: List<ShadowLayer>)\n\n\
            /** A glass grade: tint alpha per appearance and blur radius (dp). */\n\
            data class GlassGrade(val name: String, val alpha: Themed, val blur: Float)\n";
    Ok(out)
}

fn csharp(tokens: &Tokens) -> Result<String> {
    let mut out = header("//", "csharp");
    out += "\nusing System.Collections.Generic;\n\nnamespace Jellyfesh.Design;\n\n";
    out += "/// <summary>Design tokens from design/tokens.json.</summary>\npublic static class Tokens\n{\n";
    writeln!(
        out,
        "    public const string Version = {};",
        cstring(&tokens.version)
    )?;
    writeln!(
        out,
        "    /// <summary>Hue the theme uses before an artwork hue is known.</summary>\n    public const double DefaultHue = {};",
        cfloat(tokens.default_hue)
    )?;
    for group in &tokens.groups {
        writeln!(out)?;
        if let Some(doc) = &group.doc {
            writeln!(out, "    /// <summary>{}</summary>", xml(doc))?;
        }
        writeln!(out, "    public static class {}", type_name(&group.key))?;
        writeln!(out, "    {{")?;
        let mut names = BTreeSet::new();
        let class = type_name(&group.key);
        for token in &group.tokens {
            let mut name = pascal(&member(token));
            // C# forbids a member named like its class (Tokens.Accent.Accent): add the kind.
            if name == class {
                name += if matches!(token.value, Kind::Color(_)) {
                    "Color"
                } else {
                    "Value"
                };
            }
            let name = unique(&mut names, &group.key, name)?;
            if let Some(doc) = &token.doc {
                writeln!(out, "        /// <summary>{}</summary>", xml(doc))?;
            }
            let (keyword, ty, value) = match &token.value {
                Kind::Number(n) => ("const", "double".to_owned(), cfloat(*n)),
                Kind::Numbers(v) => (
                    "static readonly",
                    "IReadOnlyList<double>".into(),
                    format!(
                        "new double[] {{ {} }}",
                        v.iter().map(|n| cfloat(*n)).collect::<Vec<_>>().join(", ")
                    ),
                ),
                Kind::Themed(l, d) => (
                    "static readonly",
                    "Themed".into(),
                    format!("new({}, {})", cfloat(*l), cfloat(*d)),
                ),
                Kind::Px(px) => ("const", "double".into(), cfloat(*px)),
                Kind::Fluid { min, max, vw } => (
                    "static readonly",
                    "Fluid".into(),
                    format!(
                        "new({}, {}, {})",
                        cfloat(*min),
                        cfloat(*max),
                        vw.map_or("null".into(), cfloat)
                    ),
                ),
                Kind::Ms(ms) => ("const", "int".into(), ms.to_string()),
                Kind::Bezier([a, b, c, d]) => (
                    "static readonly",
                    "CubicBezier".into(),
                    format!(
                        "new({}, {}, {}, {})",
                        cfloat(*a),
                        cfloat(*b),
                        cfloat(*c),
                        cfloat(*d)
                    ),
                ),
                Kind::Text(text) => ("const", "string".into(), cstring(text)),
                Kind::Fonts(faces) => (
                    "static readonly",
                    "IReadOnlyList<string>".into(),
                    format!(
                        "new[] {{ {} }}",
                        faces.iter().map(|f| cstring(f)).collect::<Vec<_>>().join(", ")
                    ),
                ),
                Kind::Typography {
                    size,
                    line_height,
                    weight,
                } => (
                    "static readonly",
                    "Typography".into(),
                    format!("new({}, {}, {weight})", cfloat(*size), cfloat(*line_height)),
                ),
                Kind::Color(key) => (
                    "static readonly",
                    "ThemeColorKey".into(),
                    format!("new({})", cstring(key)),
                ),
                Kind::Shadow { light, dark } => {
                    let layers = |layers: &[Layer]| {
                        let items: Vec<String> = layers
                            .iter()
                            .map(|l| {
                                format!(
                                    "new ShadowLayer({}, {}, {}, {}, new ThemeColorKey({}))",
                                    cfloat(l.x),
                                    cfloat(l.y),
                                    cfloat(l.blur),
                                    cfloat(l.spread),
                                    cstring(&l.color)
                                )
                            })
                            .collect();
                        format!("new[] {{ {} }}", items.join(", "))
                    };
                    (
                        "static readonly",
                        "Shadow".into(),
                        format!("new({}, {})", layers(light), layers(dark)),
                    )
                }
                Kind::Glass {
                    grade,
                    light,
                    dark,
                    blur,
                } => (
                    "static readonly",
                    "GlassGrade".into(),
                    format!(
                        "new({}, new Themed({}, {}), {})",
                        cstring(grade),
                        cfloat(*light),
                        cfloat(*dark),
                        cfloat(*blur)
                    ),
                ),
                Kind::Grades(grades) => (
                    "static readonly",
                    "IReadOnlyList<GlassGrade>".into(),
                    format!(
                        "new[] {{ {} }}",
                        grades.iter().map(|g| pascal(g)).collect::<Vec<_>>().join(", ")
                    ),
                ),
            };
            writeln!(out, "        public {keyword} {ty} {name} = {value};")?;
        }
        writeln!(out, "    }}")?;
    }
    out += "}\n\n";
    out += "/// <summary>The key of a colour in Theme.Colors (for example ink.ink-90).</summary>\n\
            public readonly record struct ThemeColorKey(string Name);\n\n\
            /// <summary>A value with a light and a dark appearance.</summary>\n\
            public readonly record struct Themed(double Light, double Dark);\n\n\
            /// <summary>Cubic-bezier control points (a KeySpline cannot overshoot: Spring needs the composition API).</summary>\n\
            public readonly record struct CubicBezier(double X1, double Y1, double X2, double Y2);\n\n\
            /// <summary>A type role: size (epx), line height (a multiple of the size) and weight (1 to 1000).</summary>\n\
            public readonly record struct Typography(double Size, double LineHeight, int Weight);\n\n\
            /// <summary>A fluid size range (epx); Vw is the preferred size in % of the viewport width.</summary>\n\
            public readonly record struct Fluid(double Min, double Max, double? Vw);\n\n\
            /// <summary>One shadow layer; its colour comes from the theme.</summary>\n\
            public readonly record struct ShadowLayer(double X, double Y, double Blur, double Spread, ThemeColorKey Color);\n\n\
            /// <summary>A shadow's layers in each appearance.</summary>\n\
            public sealed record Shadow(IReadOnlyList<ShadowLayer> Light, IReadOnlyList<ShadowLayer> Dark);\n\n\
            /// <summary>A glass grade: tint alpha per appearance and blur radius (epx).</summary>\n\
            public readonly record struct GlassGrade(string Name, Themed Alpha, double Blur);\n";
    Ok(out)
}

/// `--jf-<name>` of a theme colour (`ink.ink-90` → `--jf-ink-90`, `elevation.shadow-1.0` →
/// `--jf-shadow-1-0`), as the Linux app names its GTK colours.
pub fn css_color(key: &str) -> String {
    let name = key.split_once('.').map_or(key, |(_, name)| name);
    let name: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    format!("--jf-{name}")
}

/// `--jf-<group>-<key>` of a static token, without the group repeated (`glass.glass-blur` →
/// `--jf-glass-blur`, `space.s4` → `--jf-space-s4`).
fn css_name(group: &str, key: &str) -> String {
    let key = key.strip_prefix(&format!("{group}-")).unwrap_or(key);
    if key == group {
        format!("--jf-{group}")
    } else {
        format!("--jf-{group}-{key}")
    }
}

/// A CSS number: the shortest decimal for the `f32`, without a trailing `.0`.
fn css_number(value: f64) -> String {
    let text = float(value);
    text.strip_suffix(".0").map(str::to_owned).unwrap_or(text)
}

fn css_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

fn css_shadow(layers: &[Layer]) -> String {
    layers
        .iter()
        .map(|l| {
            format!(
                "{}px {}px {}px {}px var({})",
                css_number(l.x),
                css_number(l.y),
                css_number(l.blur),
                css_number(l.spread),
                css_color(&l.color)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Jellyfesh Web's stylesheet: every static token as a custom property of `:root`, the dark values
/// under `:root[data-appearance="dark"]`. Colours are not baked (Misty PLT-03): `--jf-<colour>`
/// properties are set at run time from the core's theme, and shadows refer to them.
fn css(tokens: &Tokens) -> Result<String> {
    let mut light: Vec<(String, String)> = vec![
        ("--jf-version".into(), css_string(&tokens.version)),
        ("--jf-default-hue".into(), css_number(tokens.default_hue)),
    ];
    let mut dark: Vec<(String, String)> = Vec::new();
    for group in &tokens.groups {
        for token in &group.tokens {
            let name = css_name(&group.key, &token.key);
            match &token.value {
                Kind::Color(_) | Kind::Grades(_) => {}
                Kind::Number(n) => light.push((name, css_number(*n))),
                Kind::Numbers(v) => light.push((
                    name,
                    v.iter().map(|n| css_number(*n)).collect::<Vec<_>>().join(" "),
                )),
                Kind::Themed(l, d) => {
                    light.push((name.clone(), css_number(*l)));
                    dark.push((name, css_number(*d)));
                }
                Kind::Px(px) => light.push((name, format!("{}px", css_number(*px)))),
                Kind::Fluid { min, max, vw } => match vw {
                    Some(vw) => light.push((
                        name,
                        format!(
                            "clamp({}px, {}vw, {}px)",
                            css_number(*min),
                            css_number(*vw),
                            css_number(*max)
                        ),
                    )),
                    None => {
                        light.push((format!("{name}-min"), format!("{}px", css_number(*min))));
                        light.push((format!("{name}-max"), format!("{}px", css_number(*max))));
                    }
                },
                Kind::Ms(ms) => light.push((name, format!("{ms}ms"))),
                Kind::Bezier([a, b, c, d]) => light.push((
                    name,
                    format!(
                        "cubic-bezier({}, {}, {}, {})",
                        css_number(*a),
                        css_number(*b),
                        css_number(*c),
                        css_number(*d)
                    ),
                )),
                Kind::Text(text) => light.push((name, css_string(text))),
                Kind::Fonts(faces) => light.push((
                    name,
                    faces
                        .iter()
                        .map(|f| if f.contains(' ') { css_string(f) } else { f.clone() })
                        .collect::<Vec<_>>()
                        .join(", "),
                )),
                Kind::Typography {
                    size,
                    line_height,
                    weight,
                } => {
                    light.push((format!("{name}-size"), format!("{}px", css_number(*size))));
                    light.push((format!("{name}-line-height"), css_number(*line_height)));
                    light.push((format!("{name}-weight"), weight.to_string()));
                }
                Kind::Shadow { light: l, dark: d } => {
                    light.push((name.clone(), css_shadow(l)));
                    dark.push((name, css_shadow(d)));
                }
                Kind::Glass {
                    grade,
                    light: l,
                    dark: d,
                    blur,
                } => {
                    let base = format!("--jf-glass-{grade}");
                    light.push((format!("{base}-alpha"), css_number(*l)));
                    dark.push((format!("{base}-alpha"), css_number(*d)));
                    light.push((format!("{base}-blur"), format!("{}px", css_number(*blur))));
                }
            }
        }
    }
    let mut seen = BTreeSet::new();
    for (name, _) in &light {
        ensure!(
            seen.insert(name.clone()),
            "two tokens generate the custom property {name}"
        );
    }
    let block = |props: &[(String, String)]| {
        props
            .iter()
            .map(|(name, value)| format!("  {name}: {value};\n"))
            .collect::<String>()
    };
    let mut out =
        format!("/* @generated by `cargo xtask tokens --target css` from `{SOURCE}`. Do not edit by hand.\n");
    out += " *\n";
    out += " * Jellyfesh Web's design tokens as custom properties. Dimensions in px, durations in ms,\n";
    out += " * curves as cubic-bezier(); dark values under :root[data-appearance=\"dark\"]. Colours are\n";
    out += " * not baked (Misty PLT-03): the page sets --jf-<name> (for example --jf-ink-90) from the\n";
    out += " * core's theme at run time, which resolves every colour from one hue; shadows refer to them.\n";
    out += " */\n\n";
    write!(
        out,
        ":root {{\n{}}}\n\n:root[data-appearance=\"dark\"] {{\n{}}}\n",
        block(&light),
        block(&dark)
    )?;
    Ok(out)
}

// ---- names and literals ---------------------------------------------------------------------

/// The member name before casing: durations end in `ms`.
fn member(token: &Token) -> String {
    match token.value {
        Kind::Ms(_) => format!("{}-ms", token.key),
        _ => token.key.clone(),
    }
}

/// `type-roles` → `TypeRoles`; `type` → `TypeScale` (Swift reserves `Type` for metatypes).
fn type_name(group: &str) -> String {
    match group {
        "type" => "TypeScale".into(),
        other => pascal(other),
    }
}

fn words(key: &str) -> Vec<String> {
    key.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    chars
        .next()
        .map(|c| c.to_ascii_uppercase().to_string() + chars.as_str())
        .unwrap_or_default()
}

/// `ink-90` → `ink90`, `text-display` → `textDisplay`.
fn camel(key: &str) -> String {
    let words = words(key);
    let mut out = words.first().cloned().unwrap_or_default();
    for word in &words[1.min(words.len())..] {
        out += &capitalised(word);
    }
    guard_digit(out)
}

/// `ink-90` → `Ink90`.
fn pascal(key: &str) -> String {
    guard_digit(words(key).iter().map(|w| capitalised(w)).collect())
}

fn guard_digit(name: String) -> String {
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{name}")
    } else {
        name
    }
}

fn unique(names: &mut BTreeSet<String>, group: &str, name: String) -> Result<String> {
    ensure!(
        names.insert(name.clone()),
        "{group}: two tokens generate the name {name}"
    );
    Ok(name)
}

/// The shortest decimal that reads back as the same `f32`, as a Swift or C# double literal.
fn float(value: f64) -> String {
    format!("{:?}", value as f32)
}

/// A Kotlin `Float` literal.
fn kfloat(value: f64) -> String {
    format!("{}f", float(value))
}

/// A C# `double` literal (the same digits as the Rust module's `f32`).
fn cfloat(value: f64) -> String {
    float(value)
}

fn list(items: impl Iterator<Item = String>) -> String {
    format!("[{}]", items.collect::<Vec<_>>().join(", "))
}

/// A Kotlin string literal (`$` would start a template).
fn kstring(text: &str) -> String {
    format!("{text:?}").replace('$', "\\$")
}

/// A C# string literal.
fn cstring(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out += "\\\"",
            '\\' => out += "\\\\",
            '\n' => out += "\\n",
            c if (c as u32) < 0x20 => write!(out, "\\u{:04x}", c as u32).unwrap_or_default(),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn doc_lines(out: &mut String, prefix: &str, doc: Option<&str>) {
    if let Some(doc) = doc {
        out.push_str(prefix);
        out.push(' ');
        out.push_str(doc);
        out.push('\n');
    }
}

fn doc_block(out: &mut String, indent: &str, doc: Option<&str>) {
    if let Some(doc) = doc {
        // `*/` would end the comment early.
        let doc = doc.replace("*/", "*\u{2215}");
        out.push_str(&format!("{indent}/** {doc} */\n"));
    }
}

/// Object entries in a stable, readable order (natural sort: `s2` before `s10`), without `$` keys.
fn entries(map: &Map<String, Value>) -> Vec<(&str, &Value)> {
    let mut entries: Vec<(&str, &Value)> = map
        .iter()
        .filter(|(k, _)| !k.starts_with('$'))
        .map(|(k, v)| (k.as_str(), v))
        .collect();
    entries.sort_by_key(|entry| natural_key(entry.0));
    entries
}

/// Sort key comparing runs of digits as numbers.
fn natural_key(key: &str) -> Vec<(String, u64)> {
    let mut parts = Vec::new();
    let mut text = String::new();
    let mut chars = key.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            let mut digits = c.to_string();
            while let Some(d) = chars.peek().copied().filter(char::is_ascii_digit) {
                digits.push(d);
                chars.next();
            }
            parts.push((std::mem::take(&mut text), digits.parse().unwrap_or(u64::MAX)));
        } else {
            text.push(c);
        }
    }
    parts.push((text, 0));
    parts
}

fn number(value: &Value, path: &str) -> Result<f64> {
    value
        .as_f64()
        .with_context(|| format!("{path}: expected a number, found {value}"))
}

fn value_unit<'a>(value: &'a Value, path: &str) -> Result<(f64, &'a str)> {
    let amount = number(value.get("value").unwrap_or(&Value::Null), path)?;
    let unit = value
        .get("unit")
        .and_then(Value::as_str)
        .with_context(|| format!("{path}: no unit"))?;
    Ok((amount, unit))
}

/// A dimension in px, and the factor its unit converts by.
fn dimension(value: &Value, path: &str) -> Result<(f64, f64)> {
    let (amount, unit) = value_unit(value, path)?;
    match unit {
        "px" => Ok((amount, 1.0)),
        "rem" => Ok((amount * PX_PER_REM, PX_PER_REM)),
        other => bail!("{path}: unsupported dimension unit {other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Tokens {
        Tokens::parse(&json!({
            "$extensions": { "org.jellyfesh": { "version": "test" } },
            "parameter": { "hue": { "$type": "number", "$value": 250.0 } },
            "space": { "s10": { "$type": "dimension", "$value": { "value": 2.5, "unit": "rem" } },
                       "s2": { "$type": "dimension", "$value": { "value": 0.5, "unit": "rem" } } },
            "type": { "text-h1": { "$type": "dimension", "$value": { "value": 2, "unit": "rem" },
                "$extensions": { "org.jellyfesh": { "fluid": { "min": 2, "max": 3, "vw": 4.0 } } } } },
            "motion": { "dur-0": { "$type": "duration", "$value": { "value": 80.0, "unit": "ms" } },
                        "ease": { "$type": "cubicBezier", "$value": [0.22, 1.0, 0.36, 1.0] } },
            "ink": { "ink-90": { "$type": "color", "$value": {} } },
            "brand": { "mood": { "$type": "string", "$value": "calm \"tidal\" $x", "$description": "Mood." } }
        }))
        .unwrap()
    }

    #[test]
    fn names_follow_each_language() {
        assert_eq!(camel("ink-90"), "ink90");
        assert_eq!(camel("text-display"), "textDisplay");
        assert_eq!(pascal("glass-alpha-clear"), "GlassAlphaClear");
        assert_eq!(pascal("2xl"), "_2xl");
        assert_eq!(type_name("type"), "TypeScale");
        assert_eq!(type_name("type-roles"), "TypeRoles");
    }

    #[test]
    fn writes_every_language() {
        let tokens = sample();
        let space = tokens.groups.iter().find(|g| g.key == "space").unwrap();
        assert_eq!(space.tokens[0].key, "s2", "natural order");
        let swift = swift(&tokens).unwrap();
        assert!(swift.contains("public enum TypeScale {"), "{swift}");
        assert!(swift.contains("public static let s2: CGFloat = 8.0"), "{swift}");
        assert!(swift.contains("public static let dur0Ms: Int = 80"), "{swift}");
        assert!(
            swift.contains("public static let textH1Fluid: Fluid = Fluid(min: 32.0, max: 48.0, vw: 4.0)"),
            "{swift}"
        );
        assert!(
            swift.contains("public static let ink90: ThemeColorKey = ThemeColorKey(\"ink.ink-90\")"),
            "{swift}"
        );
        let kotlin = kotlin(&tokens).unwrap();
        assert!(kotlin.contains("const val s10: Float = 40.0f"), "{kotlin}");
        assert!(
            kotlin.contains("val ease: CubicBezier = CubicBezier(0.22f, 1.0f, 0.36f, 1.0f)"),
            "{kotlin}"
        );
        assert!(
            kotlin.contains("const val mood: String = \"calm \\\"tidal\\\" \\$x\""),
            "{kotlin}"
        );
        let csharp = csharp(&tokens).unwrap();
        assert!(csharp.contains("public static class TypeScale"), "{csharp}");
        assert!(csharp.contains("public const int Dur0Ms = 80;"), "{csharp}");
        assert!(
            csharp.contains("public static readonly ThemeColorKey Ink90 = new(\"ink.ink-90\");"),
            "{csharp}"
        );
    }

    #[test]
    fn the_real_token_file_converts() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let json: Value = serde_json::from_str(&std::fs::read_to_string(root.join(SOURCE)).unwrap()).unwrap();
        let tokens = Tokens::parse(&json).unwrap();
        for code in [
            swift(&tokens).unwrap(),
            kotlin(&tokens).unwrap(),
            csharp(&tokens).unwrap(),
        ] {
            assert!(code.contains("Glass"), "glass grades");
            assert!(code.contains("elevation.shadow-1.0"), "shadow layer colours");
        }

        // CSS: every static token once, colours only as theme references.
        let stylesheet = css(&tokens).unwrap();
        assert!(stylesheet.contains("  --jf-space-s4: 16px;\n"));
        assert!(stylesheet.contains("  --jf-motion-ease-fluid: cubic-bezier(0.22, 1, 0.36, 1);\n"));
        assert!(stylesheet.contains("  --jf-lyrics-line-size: 40px;\n"));
        assert!(stylesheet.contains("var(--jf-shadow-1-0)"));
        assert!(stylesheet.contains(":root[data-appearance=\"dark\"] {"));
        assert!(!stylesheet.contains('#'), "no baked colour");
        assert_eq!(css_color("elevation.shadow-1.0"), "--jf-shadow-1-0");
        assert_eq!(css_name("glass", "glass-blur"), "--jf-glass-blur");
        assert_eq!(css_name("line", "line"), "--jf-line");

        // The colour keys are exactly the colours the core's theme resolves: the names of the
        // ColorDefs in the committed Rust module (THEME_COLORS).
        let rust = std::fs::read_to_string(root.join(crate::tokens::OUTPUT)).unwrap();
        let resolved: BTreeSet<&str> = rust
            .split("name: \"")
            .skip(1)
            .map(|rest| &rest[..rest.find('"').unwrap()])
            .collect();
        let keys: BTreeSet<&str> = tokens
            .groups
            .iter()
            .flat_map(|g| &g.tokens)
            .filter_map(|t| match &t.value {
                Kind::Color(key) => Some(key.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(keys, resolved);
    }
}
