//! `cargo xtask tokens`: generate the Rust design tokens (`jf-tokens`) from `design/tokens.json`.
//!
//! The token file follows the Design Tokens Format Module. Each top-level group becomes a module;
//! each token becomes a `pub const` chosen by its `$type`:
//!
//! | `$type`       | Rust                                                            |
//! | ------------- | --------------------------------------------------------------- |
//! | `number`      | `f32`; `Themed` when it carries a `dark` value; `[f32; N]` lists  |
//! | `dimension`   | `f32` in px (1 rem = 16 px), plus `<NAME>_FLUID` when fluid      |
//! | `duration`    | `u32` milliseconds, named `<NAME>_MS`                            |
//! | `cubicBezier` | `[f32; 4]`                                                       |
//! | `string`      | `&str`; `fontFamily` is `&[&str]`                                |
//! | `typography`  | `Typography`                                                     |
//! | `color`       | `ColorDef` (light and dark OKLCH), listed in `THEME_COLORS`      |
//! | `shadow`      | `Shadow`; layer colours join `THEME_COLORS`                      |
//!
//! A group with `glass-l`, `glass-c` and `glass-alpha*`/`glass-blur*` pairs also gets one
//! `GlassGrade` per grade and the glass tint colours derived from them.
//!
//! The output goes through `rustfmt`, so `cargo fmt --check` accepts it unchanged and the result
//! is the same on every machine with the pinned toolchain.

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Map, Value};

/// The token source, relative to the workspace root.
pub const SOURCE: &str = "design/tokens.json";
/// The generated Rust module, relative to the workspace root.
pub const OUTPUT: &str = "core/jf-tokens/src/generated.rs";
/// The `$extensions` namespace Jellyfesh uses.
const EXT: &str = "org.jellyfesh";
const PX_PER_REM: f64 = 16.0;

/// Regenerate the Rust tokens, or with `check` only report whether the file is stale.
pub fn run(check: bool) -> Result<()> {
    let root = workspace_root();
    let source = std::fs::read_to_string(root.join(SOURCE)).with_context(|| format!("reading {SOURCE}"))?;
    let json: Value = serde_json::from_str(&source).with_context(|| format!("parsing {SOURCE}"))?;
    let code = rustfmt(&root, &generate(&json)?)?;
    // Compare without carriage returns: a Windows checkout may have converted line endings.
    let current = std::fs::read_to_string(root.join(OUTPUT))
        .ok()
        .map(|s| s.replace("\r\n", "\n"));
    match (check, current) {
        (_, Some(current)) if current == code => {
            println!("{OUTPUT} is up to date");
            Ok(())
        }
        (true, Some(current)) => bail!(
            "{OUTPUT} is stale (first difference on line {}); run `cargo xtask tokens`",
            first_difference(&current, &code)
        ),
        (true, None) => bail!("{OUTPUT} is missing; run `cargo xtask tokens`"),
        (false, _) => {
            std::fs::write(root.join(OUTPUT), &code).with_context(|| format!("writing {OUTPUT}"))?;
            println!("wrote {OUTPUT}");
            Ok(())
        }
    }
}

fn workspace_root() -> PathBuf {
    let xtask = Path::new(env!("CARGO_MANIFEST_DIR"));
    xtask.parent().unwrap_or(xtask).to_path_buf()
}

fn first_difference(a: &str, b: &str) -> usize {
    let mut lines = a.lines().zip(b.lines());
    lines
        .position(|(x, y)| x != y)
        .unwrap_or_else(|| a.lines().count().min(b.lines().count()))
        + 1
}

/// Format generated code with the workspace's `rustfmt.toml`.
fn rustfmt(root: &Path, code: &str) -> Result<String> {
    let program = std::env::var_os("RUSTFMT").unwrap_or_else(|| "rustfmt".into());
    let mut child = Command::new(program)
        .current_dir(root)
        .args(["--edition", "2024", "--config-path"])
        .arg(root.join("rustfmt.toml"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("running rustfmt (install it with `rustup component add rustfmt`)")?;
    let mut stdin = child.stdin.take().context("rustfmt stdin")?;
    let input = code.to_owned();
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let output = child.wait_with_output().context("waiting for rustfmt")?;
    writer
        .join()
        .map_err(|_| anyhow::anyhow!("rustfmt input thread panicked"))??;
    ensure!(
        output.status.success(),
        "rustfmt rejected the generated code:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).context("rustfmt output is not UTF-8")
}

/// Generate the Rust module (before formatting) for a parsed token file.
pub fn generate(tokens: &Value) -> Result<String> {
    let root = tokens
        .as_object()
        .context("the token file must be a JSON object")?;
    let mut generator = Generator::default();
    let mut modules = String::new();
    for (key, node) in entries(root) {
        let group = node
            .as_object()
            .with_context(|| format!("{key}: expected a group"))?;
        modules += &generator.module(key, group)?;
    }
    for (token, target) in &generator.references {
        ensure!(
            generator.color_names.contains(target),
            "{token}: refers to {target}, which is not a theme colour"
        );
    }

    let version = root
        .get("$extensions")
        .and_then(|e| e.get(EXT))
        .and_then(|e| e.get("version"))
        .and_then(Value::as_str)
        .unwrap_or("unversioned");
    let hue = root
        .get("parameter")
        .and_then(|g| g.get("hue"))
        .and_then(|t| t.get("$value"))
        .context("parameter.hue is required: it is the default theme hue")?;
    let hue = number(hue, "parameter.hue")?;

    let mut out = String::new();
    writeln!(
        out,
        "// @generated by `cargo xtask tokens` from `{SOURCE}`. Do not edit by hand:"
    )?;
    writeln!(
        out,
        "// change the token file and run `cargo xtask tokens` (CI runs `cargo xtask tokens --check`)."
    )?;
    writeln!(out, "//")?;
    writeln!(
        out,
        "// Units: dimensions in px (1 rem = {PX_PER_REM} px), durations in milliseconds (`_MS`),"
    )?;
    writeln!(
        out,
        "// cubic-bezier curves as `[x1, y1, x2, y2]`. Colours are OKLCH definitions that"
    )?;
    writeln!(out, "// `jf-palette` resolves to sRGB at run time from one hue.")?;
    writeln!(out)?;
    writeln!(
        out,
        "#![allow(clippy::approx_constant, clippy::excessive_precision)]"
    )?;
    writeln!(out)?;
    if !generator.colors.is_empty() {
        writeln!(out, "use crate::ColorDef;")?;
        writeln!(out)?;
    }
    writeln!(
        out,
        "/// Version of the token file (`$extensions.{EXT}.version`)."
    )?;
    writeln!(out, "pub const VERSION: &str = {version:?};")?;
    writeln!(
        out,
        "/// Hue that theme colours are resolved at when there is no artwork hue (`parameter.hue`)."
    )?;
    writeln!(out, "pub const DEFAULT_HUE: f32 = {};", float(hue))?;
    if !generator.colors.is_empty() {
        writeln!(
            out,
            "/// Every themed colour, by group: what `jf-palette` resolves into a theme."
        )?;
        writeln!(out, "pub const THEME_COLORS: &[ColorDef] = &[")?;
        for path in &generator.colors {
            writeln!(out, "    {path},")?;
        }
        writeln!(out, "];")?;
    }
    out += &modules;
    Ok(out)
}

#[derive(Default)]
struct Generator {
    /// `module::CONST` paths of the theme colours, in output order.
    colors: Vec<String>,
    /// Theme colour names such as `ink.ink-90`.
    color_names: BTreeSet<String>,
    /// (token, colour name) pairs for shadow layers that reuse another colour.
    references: Vec<(String, String)>,
}

/// One generated module.
struct Module {
    group: String,
    ident: String,
    body: String,
    types: BTreeSet<&'static str>,
    names: BTreeSet<String>,
}

impl Module {
    fn new(group: &str) -> Self {
        Module {
            group: group.to_owned(),
            ident: module_ident(group),
            body: String::new(),
            types: BTreeSet::new(),
            names: BTreeSet::new(),
        }
    }

    fn uses(&mut self, ty: &'static str) {
        self.types.insert(ty);
    }

    fn constant(&mut self, docs: &[String], name: &str, ty: &str, value: &str) -> Result<()> {
        ensure!(
            self.names.insert(name.to_owned()),
            "{}: two tokens generate the constant {name}",
            self.group
        );
        for line in docs {
            writeln!(self.body, "    /// {line}")?;
        }
        writeln!(self.body, "    pub const {name}: {ty} = {value};")?;
        Ok(())
    }
}

impl Generator {
    fn module(&mut self, group: &str, tokens: &Map<String, Value>) -> Result<String> {
        let mut module = Module::new(group);
        let group_type = tokens.get("$type").and_then(Value::as_str);
        for (key, node) in entries(tokens) {
            let token = node
                .as_object()
                .with_context(|| format!("{group}.{key}: expected an object"))?;
            ensure!(
                token.contains_key("$value"),
                "{group}.{key}: nested groups are not supported by the generator yet"
            );
            self.token(&mut module, key, token, group_type)?;
        }
        self.glass_grades(&mut module, tokens)?;

        let mut out = String::new();
        writeln!(out)?;
        match tokens.get("$description").and_then(Value::as_str) {
            Some(description) => writeln!(out, "/// {}", doc_text(description))?,
            None => writeln!(out, "/// The `{group}` tokens.")?,
        }
        writeln!(out, "pub mod {} {{", module.ident)?;
        if !module.types.is_empty() {
            let types: Vec<&str> = module.types.iter().copied().collect();
            writeln!(out, "    use crate::{{{}}};", types.join(", "))?;
            writeln!(out)?;
        }
        out += &module.body;
        writeln!(out, "}}")?;
        Ok(out)
    }

    fn token(
        &mut self,
        module: &mut Module,
        key: &str,
        token: &Map<String, Value>,
        group_type: Option<&str>,
    ) -> Result<()> {
        let path = format!("{}.{key}", module.group);
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
        let name = const_ident(key);
        let mut docs = Vec::new();
        if let Some(description) = token.get("$description").and_then(Value::as_str) {
            docs.push(doc_text(description));
        }

        match ty {
            "color" => {
                let (light, dark) = if themed {
                    (lcha(ext_get("light"), &path)?, lcha(ext_get("dark"), &path)?)
                } else {
                    let fixed = lcha_from_value(value, &path)?;
                    (fixed.clone(), fixed)
                };
                self.color(module, &docs, &name, &path, &light, &dark)
            }
            "number" => {
                if let Some(unit) = ext_get("unit").and_then(Value::as_str) {
                    docs.push(format!("Unit: {unit}."));
                }
                match (value, ext_get("dark")) {
                    (Value::Array(items), _) => {
                        let items = items
                            .iter()
                            .map(|v| number(v, &path).map(float))
                            .collect::<Result<Vec<_>>>()?;
                        let ty = format!("[f32; {}]", items.len());
                        module.constant(&docs, &name, &ty, &format!("[{}]", items.join(", ")))
                    }
                    (_, Some(dark)) if themed => {
                        module.uses("Themed");
                        let value = format!(
                            "Themed::new({}, {})",
                            float(number(value, &path)?),
                            float(number(dark, &path)?)
                        );
                        module.constant(&docs, &name, "Themed", &value)
                    }
                    _ => module.constant(&docs, &name, "f32", &float(number(value, &path)?)),
                }
            }
            "dimension" => {
                let (px, scale, source) = dimension(value, &path)?;
                if let Some(source) = source {
                    docs.push(format!("Source: `{source}`."));
                }
                module.constant(&docs, &name, "f32", &float(px))?;
                self.fluid(module, &name, ext_get("fluid"), scale, &path)
            }
            "duration" => {
                let (amount, unit) = value_unit(value, &path)?;
                let ms = match unit {
                    "ms" => amount,
                    "s" => amount * 1000.0,
                    other => bail!("{path}: unsupported duration unit {other:?}"),
                };
                ensure!(
                    ms >= 0.0 && ms.fract() == 0.0 && ms <= f64::from(u32::MAX),
                    "{path}: a duration must be a whole number of milliseconds"
                );
                module.constant(&docs, &format!("{name}_MS"), "u32", &format!("{ms}"))
            }
            "cubicBezier" => {
                let points = value
                    .as_array()
                    .with_context(|| format!("{path}: expected four numbers"))?;
                ensure!(points.len() == 4, "{path}: expected four numbers");
                let points = points
                    .iter()
                    .map(|v| number(v, &path).map(float))
                    .collect::<Result<Vec<_>>>()?;
                module.constant(&docs, &name, "[f32; 4]", &format!("[{}]", points.join(", ")))
            }
            "fontFamily" => {
                let faces = match value {
                    Value::String(face) => vec![format!("{face:?}")],
                    Value::Array(faces) => faces
                        .iter()
                        .map(|f| f.as_str().map(|f| format!("{f:?}")))
                        .collect::<Option<Vec<_>>>()
                        .with_context(|| format!("{path}: expected a list of strings"))?,
                    _ => bail!("{path}: expected a string or a list of strings"),
                };
                module.constant(&docs, &name, "&[&str]", &format!("&[{}]", faces.join(", ")))
            }
            "string" => {
                let text = value
                    .as_str()
                    .with_context(|| format!("{path}: expected a string"))?;
                module.constant(&docs, &name, "&str", &format!("{text:?}"))
            }
            "typography" => {
                let size = value
                    .get("fontSize")
                    .with_context(|| format!("{path}: no fontSize"))?;
                let (size, scale, _) = dimension(size, &path)?;
                let line_height = number(value.get("lineHeight").unwrap_or(&Value::Null), &path)?;
                let weight = value
                    .get("fontWeight")
                    .and_then(Value::as_u64)
                    .filter(|w| (1..=1000).contains(w))
                    .with_context(|| format!("{path}: fontWeight must be 1 to 1000"))?;
                module.uses("Typography");
                let value = format!(
                    "Typography::new({}, {}, {weight})",
                    float(size),
                    float(line_height)
                );
                module.constant(&docs, &name, "Typography", &value)?;
                self.fluid(module, &name, ext_get("fluid"), scale, &path)
            }
            "shadow" => self.shadow(
                module,
                &docs,
                &name,
                &path,
                value,
                themed.then_some(ext).flatten(),
            ),
            other => bail!("{path}: unsupported $type {other:?}"),
        }
    }

    /// A theme colour: a `ColorDef` constant that is also listed in `THEME_COLORS`.
    fn color(
        &mut self,
        module: &mut Module,
        docs: &[String],
        name: &str,
        color_name: &str,
        light: &str,
        dark: &str,
    ) -> Result<()> {
        ensure!(
            self.color_names.insert(color_name.to_owned()),
            "two colours are named {color_name}"
        );
        module.uses("ColorDef");
        module.uses("Lcha");
        let value = format!("ColorDef {{ name: {color_name:?}, light: {light}, dark: {dark} }}");
        module.constant(docs, name, "ColorDef", &value)?;
        self.colors.push(format!("{}::{name}", module.ident));
        Ok(())
    }

    fn fluid(
        &mut self,
        module: &mut Module,
        name: &str,
        fluid: Option<&Value>,
        scale: f64,
        path: &str,
    ) -> Result<()> {
        let Some(fluid) = fluid else { return Ok(()) };
        let field = |key: &str| -> Result<f64> {
            number(
                fluid
                    .get(key)
                    .with_context(|| format!("{path}: fluid needs {key}"))?,
                path,
            )
        };
        let (min, max) = (field("min")? * scale, field("max")? * scale);
        let vw = match fluid.get("vw") {
            Some(vw) => format!("Some({})", float(number(vw, path)?)),
            None => "None".to_owned(),
        };
        let mut docs = vec![format!(
            "Fluid range of `{name}` in px; `vw` is its preferred size in % of the viewport width."
        )];
        if let Some(basis) = fluid.get("basis").and_then(Value::as_str) {
            docs.push(format!("Scales with the {basis}."));
        }
        module.uses("Fluid");
        let value = format!("Fluid::new({}, {}, {vw})", float(min), float(max));
        module.constant(&docs, &format!("{name}_FLUID"), "Fluid", &value)
    }

    fn shadow(
        &mut self,
        module: &mut Module,
        docs: &[String],
        name: &str,
        path: &str,
        value: &Value,
        ext: Option<&Map<String, Value>>,
    ) -> Result<()> {
        let ext = ext.with_context(|| format!("{path}: only themed shadows are supported"))?;
        let layers = |appearance: &str| -> Result<&Vec<Value>> {
            ext.get(appearance)
                .and_then(Value::as_array)
                .with_context(|| format!("{path}: expected a list of {appearance} layers"))
        };
        let (light, dark) = (layers("light")?, layers("dark")?);
        ensure!(
            light.len() == dark.len(),
            "{path}: light and dark need the same number of layers"
        );

        let mut colors = Vec::new();
        for (i, (l, d)) in light.iter().zip(dark).enumerate() {
            let color = match (l.get("color"), d.get("color")) {
                (Some(l), Some(d)) => {
                    let color_name = format!("{path}.{i}");
                    let docs = [format!("Colour of layer {i} of `{name}`.")];
                    self.color(
                        module,
                        &docs,
                        &format!("{name}_{i}"),
                        &color_name,
                        &lcha(Some(l), path)?,
                        &lcha(Some(d), path)?,
                    )?;
                    color_name
                }
                (None, None) => {
                    let alias = value
                        .get(i)
                        .and_then(|layer| layer.get("color"))
                        .and_then(Value::as_str)
                        .and_then(|alias| alias.strip_prefix('{')?.strip_suffix('}'))
                        .with_context(|| {
                            format!("{path}: layer {i} needs a colour or a {{group.token}} alias")
                        })?;
                    self.references.push((path.to_owned(), alias.to_owned()));
                    alias.to_owned()
                }
                _ => bail!("{path}: layer {i} has a colour in one appearance only"),
            };
            colors.push(color);
        }

        let layer_list = |defs: &[Value]| -> Result<String> {
            let mut out = Vec::new();
            for (layer, color) in defs.iter().zip(&colors) {
                let field = |key: &str| number(layer.get(key).unwrap_or(&Value::Null), path).map(float);
                let (x, y, blur, spread) = (field("x")?, field("y")?, field("blur")?, field("spread")?);
                out.push(format!("ShadowLayer::new({x}, {y}, {blur}, {spread}, {color:?})"));
            }
            Ok(out.join(", "))
        };
        module.uses("Shadow");
        module.uses("ShadowLayer");
        let value = format!(
            "Shadow {{ light: &[{}], dark: &[{}] }}",
            layer_list(light)?,
            layer_list(dark)?
        );
        module.constant(docs, name, "Shadow", &value)
    }

    /// Glass grades (Clear, Mist, Veil) from `glass-alpha*`/`glass-blur*` pairs, and their tints:
    /// `oklch(glass-l glass-c hue / alpha)`, the fill each grade draws over the blurred backdrop.
    fn glass_grades(&mut self, module: &mut Module, tokens: &Map<String, Value>) -> Result<()> {
        let (Some(l), Some(c)) = (tokens.get("glass-l"), tokens.get("glass-c")) else {
            return Ok(());
        };
        let group = module.group.clone();
        let themed = |token: &Value, key: &str| -> Result<(f64, f64)> {
            let path = format!("{group}.{key}");
            let light = number(token.get("$value").unwrap_or(&Value::Null), &path)?;
            let dark = token
                .get("$extensions")
                .and_then(|e| e.get(EXT))
                .and_then(|e| e.get("dark"));
            Ok((
                light,
                dark.map(|d| number(d, &path)).transpose()?.unwrap_or(light),
            ))
        };
        let (l, c) = (themed(l, "glass-l")?, themed(c, "glass-c")?);

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
            let blur_key = format!("glass-blur{suffix}");
            let blur = tokens
                .get(&blur_key)
                .with_context(|| format!("{group}.{key}: no {blur_key}"))?;
            let (blur, _, _) = dimension(blur.get("$value").unwrap_or(&Value::Null), &blur_key)?;
            grades.push((themed(token, key)?, blur, grade));
        }
        grades.sort_by(|a, b| a.0.0.total_cmp(&b.0.0).then(a.2.cmp(b.2)));
        if grades.is_empty() {
            return Ok(());
        }

        module.uses("GlassGrade");
        module.uses("Themed");
        let mut names = Vec::new();
        for ((light, dark), blur, grade) in &grades {
            let name = const_ident(grade);
            let docs = [format!(
                "The {grade} glass grade: tint alpha and blur radius (px)."
            )];
            let value = format!(
                "GlassGrade::new({grade:?}, Themed::new({}, {}), {})",
                float(*light),
                float(*dark),
                float(*blur)
            );
            module.constant(&docs, &name, "GlassGrade", &value)?;
            names.push(name);
        }
        let docs = ["Every glass grade, clearest first.".to_owned()];
        module.constant(
            &docs,
            "GRADES",
            "&[GlassGrade]",
            &format!("&[{}]", names.join(", ")),
        )?;

        for ((light, dark), _, grade) in &grades {
            let docs = [format!(
                "Tint of {grade} glass: `glass-l`, `glass-c` at the theme hue, with the grade's alpha."
            )];
            let light = format!(
                "Lcha::relative({}, {}, 0.0, {})",
                float(l.0),
                float(c.0),
                float(*light)
            );
            let dark = format!(
                "Lcha::relative({}, {}, 0.0, {})",
                float(l.1),
                float(c.1),
                float(*dark)
            );
            let color_name = format!("{group}.tint-{grade}");
            self.color(
                module,
                &docs,
                &format!("TINT_{}", const_ident(grade)),
                &color_name,
                &light,
                &dark,
            )?;
        }
        Ok(())
    }
}

/// Object entries in a stable, readable order (natural sort: `s2` before `s10`), without `$` keys.
fn entries(map: &Map<String, Value>) -> Vec<(&str, &Value)> {
    let mut entries: Vec<(&str, &Value)> = map
        .iter()
        .filter(|(k, _)| !k.starts_with('$'))
        .map(|(k, v)| (k.as_str(), v))
        .collect();
    entries.sort_by(|a, b| natural_cmp(a.0, b.0));
    entries
}

/// Compare strings with runs of digits compared as numbers.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.as_bytes(), b.as_bytes());
    loop {
        match (a.first(), b.first()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let da = a.iter().take_while(|c| c.is_ascii_digit()).count();
                let db = b.iter().take_while(|c| c.is_ascii_digit()).count();
                let (na, ra) = a.split_at(da);
                let (nb, rb) = b.split_at(db);
                let trim = |n: &'_ [u8]| -> usize { n.iter().take_while(|&&c| c == b'0').count() };
                let (ta, tb) = (&na[trim(na)..], &nb[trim(nb)..]);
                let order = ta
                    .len()
                    .cmp(&tb.len())
                    .then_with(|| ta.cmp(tb))
                    .then_with(|| da.cmp(&db));
                if order != Ordering::Equal {
                    return order;
                }
                (a, b) = (ra, rb);
            }
            (Some(x), Some(y)) => {
                if x != y {
                    return x.cmp(y);
                }
                (a, b) = (&a[1..], &b[1..]);
            }
        }
    }
}

const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false",
    "fn", "for", "gen", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "unsafe", "use", "where",
    "while", "yield",
];

/// `type-roles` → `type_roles`; a keyword gets a trailing underscore (`type` → `type_`).
fn module_ident(group: &str) -> String {
    let ident = identifier(group).to_ascii_lowercase();
    if KEYWORDS.contains(&ident.as_str()) {
        format!("{ident}_")
    } else {
        ident
    }
}

/// `ink-90` → `INK_90`, `r-2xl` → `R_2XL`.
fn const_ident(key: &str) -> String {
    identifier(key).to_ascii_uppercase()
}

fn identifier(key: &str) -> String {
    let ident: String = key
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if ident.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{ident}")
    } else {
        ident
    }
}

/// A description as one doc-comment line (square brackets escaped so rustdoc sees no links).
fn doc_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

/// An `f32` literal: the shortest text that reads back as the same `f32`.
fn float(value: f64) -> String {
    format!("{:?}", value as f32)
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

/// A dimension in px, the factor its unit converts by, and its source text when not px.
fn dimension(value: &Value, path: &str) -> Result<(f64, f64, Option<String>)> {
    let (amount, unit) = value_unit(value, path)?;
    match unit {
        "px" => Ok((amount, 1.0, None)),
        "rem" => Ok((amount * PX_PER_REM, PX_PER_REM, Some(format!("{amount}rem")))),
        other => bail!("{path}: unsupported dimension unit {other:?}"),
    }
}

/// `Lcha::relative(l, c, h, alpha)` or `Lcha::fixed(…)` from a `{l, c, h, alpha, hue}` definition.
fn lcha(def: Option<&Value>, path: &str) -> Result<String> {
    let def = def.with_context(|| format!("{path}: a themed colour needs light and dark definitions"))?;
    let field = |key: &str| number(def.get(key).unwrap_or(&Value::Null), path).map(float);
    let constructor = match def.get("hue").and_then(Value::as_str) {
        Some("relative") => "relative",
        Some("fixed") => "fixed",
        other => bail!("{path}: hue must be \"relative\" or \"fixed\", found {other:?}"),
    };
    Ok(format!(
        "Lcha::{constructor}({}, {}, {}, {})",
        field("l")?,
        field("c")?,
        field("h")?,
        field("alpha")?
    ))
}

/// A fixed colour from an untinted `$value` (`colorSpace: oklch`).
fn lcha_from_value(value: &Value, path: &str) -> Result<String> {
    ensure!(
        value.get("colorSpace").and_then(Value::as_str) == Some("oklch"),
        "{path}: only OKLCH colours are supported"
    );
    let components = value
        .get("components")
        .and_then(Value::as_array)
        .filter(|c| c.len() == 3);
    let components = components.with_context(|| format!("{path}: expected three components"))?;
    let alpha = value
        .get("alpha")
        .map(|a| number(a, path))
        .transpose()?
        .unwrap_or(1.0);
    let [l, c, h] = [0, 1, 2].map(|i| number(&components[i], path).map(float));
    Ok(format!("Lcha::fixed({}, {}, {}, {})", l?, c?, h?, float(alpha)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn natural_order_compares_numbers() {
        let mut keys = vec!["s10", "s2", "s1", "ink-90", "ink-06", "ink-10", "r-2xl", "r-xs"];
        keys.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            keys,
            ["ink-06", "ink-10", "ink-90", "r-2xl", "r-xs", "s1", "s2", "s10"]
        );
    }

    #[test]
    fn identifiers() {
        assert_eq!(module_ident("type-roles"), "type_roles");
        assert_eq!(module_ident("type"), "type_");
        assert_eq!(const_ident("ink-90"), "INK_90");
        assert_eq!(const_ident("2xl"), "_2XL");
    }

    #[test]
    fn floats_are_shortest_f32_literals() {
        assert_eq!(float(0.085), "0.085");
        assert_eq!(float(250.0), "250.0");
        assert_eq!(float(2.4 * PX_PER_REM), "38.4");
        assert_eq!(float(-70.0), "-70.0");
    }

    #[test]
    fn generates_each_token_type() {
        let tokens = json!({
            "$extensions": { "org.jellyfesh": { "version": "test" } },
            "parameter": { "hue": { "$type": "number", "$value": 250.0 } },
            "space": { "s1": { "$type": "dimension", "$value": { "value": 0.25, "unit": "rem" } } },
            "motion": {
                "dur-0": { "$type": "duration", "$value": { "value": 80.0, "unit": "ms" } },
                "ease": { "$type": "cubicBezier", "$value": [0.22, 1.0, 0.36, 1.0] }
            },
            "ink": { "ink-90": { "$type": "color", "$value": {}, "$extensions": { "org.jellyfesh": {
                "themed": true,
                "light": { "l": 0.205, "c": 0.02, "h": 0.0, "alpha": 0.92, "hue": "relative" },
                "dark": { "l": 0.98, "c": 0.005, "h": 0.0, "alpha": 0.92, "hue": "relative" }
            } } } },
            "brand": { "icon": { "$type": "string", "$value": "ice" } }
        });
        let code = generate(&tokens).unwrap();
        assert!(code.contains("pub const DEFAULT_HUE: f32 = 250.0;"), "{code}");
        assert!(code.contains("pub const VERSION: &str = \"test\";"), "{code}");
        assert!(code.contains("pub const S1: f32 = 4.0;"), "{code}");
        assert!(code.contains("pub const DUR_0_MS: u32 = 80;"), "{code}");
        assert!(
            code.contains("pub const EASE: [f32; 4] = [0.22, 1.0, 0.36, 1.0];"),
            "{code}"
        );
        assert!(code.contains("pub const ICON: &str = \"ice\";"), "{code}");
        assert!(
            code.contains(
                "ColorDef { name: \"ink.ink-90\", light: Lcha::relative(0.205, 0.02, 0.0, 0.92), \
                 dark: Lcha::relative(0.98, 0.005, 0.0, 0.92) }"
            ),
            "{code}"
        );
        assert!(code.contains("    ink::INK_90,\n"), "{code}");
    }

    #[test]
    fn rejects_unknown_types_and_dangling_references() {
        let unknown = json!({ "parameter": { "hue": { "$type": "number", "$value": 1 } },
            "g": { "t": { "$type": "gradient", "$value": [] } } });
        assert!(
            generate(&unknown)
                .unwrap_err()
                .to_string()
                .contains("unsupported $type")
        );

        let dangling = json!({ "parameter": { "hue": { "$type": "number", "$value": 1 } },
            "elevation": { "focus": { "$type": "shadow", "$value": [{ "color": "{field.sky}" }],
                "$extensions": { "org.jellyfesh": { "themed": true,
                    "light": [{ "x": 0, "y": 0, "blur": 0, "spread": 2, "ref": "--m-sky" }],
                    "dark": [{ "x": 0, "y": 0, "blur": 0, "spread": 2, "ref": "--m-sky" }] } } } } });
        assert!(
            generate(&dangling)
                .unwrap_err()
                .to_string()
                .contains("not a theme colour")
        );
    }
}
