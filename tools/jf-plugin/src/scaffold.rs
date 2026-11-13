//! `jf-plugin new`: a plugin crate from the template in `src/template/`.

use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};

const CARGO_TOML: &str = include_str!("template/Cargo.toml.in");
const LIB_RS: &str = include_str!("template/lib.rs.in");
const MANIFEST_TOML: &str = include_str!("template/manifest.toml.in");
const SCHEMA_JSON: &str = include_str!("template/settings.schema.json.in");
const README_MD: &str = include_str!("template/README.md.in");
const GITIGNORE: &str = include_str!("template/gitignore.in");

/// What `jf-plugin new` makes.
#[derive(Debug, Clone)]
pub struct Scaffold {
    /// The cargo package name: lowercase letters, digits, `-` and `_`,
    /// starting with a letter.
    pub name: String,
    /// The folder to create (it must not exist, or be empty).
    pub dir: PathBuf,
    /// The plugin id (reverse DNS); `org.example.<name>` by default.
    pub id: Option<String>,
    /// The manifest's author.
    pub author: Option<String>,
    /// The `plugins/sdk` folder of a Jellyfesh checkout.
    pub sdk: PathBuf,
}

/// Writes the crate and returns the files it wrote.
pub fn scaffold(options: &Scaffold) -> Result<Vec<PathBuf>> {
    let name = options.name.as_str();
    ensure!(
        valid_package_name(name),
        "{name:?} is not a good package name: use lowercase letters, digits, '-' and '_', starting with a letter"
    );
    let id = options
        .id
        .clone()
        .unwrap_or_else(|| format!("org.example.{}", name.replace('_', "-")));
    ensure!(
        jf_plugin_host::manifest::valid_id(&id),
        "{id:?} is not a plugin id: use reverse-DNS, lowercase labels of a-z, 0-9 and '-', like org.example.music"
    );
    let author = options.author.clone().unwrap_or_else(|| "Your name".to_string());
    ensure!(
        !author.trim().is_empty() && !author.chars().any(char::is_control),
        "the author must be text on one line"
    );
    let sdk_manifest = options.sdk.join("Cargo.toml");
    let sdk_name = jf_plugin_build::package_name(&sdk_manifest)
        .with_context(|| format!("{} is not the plugin SDK folder", options.sdk.display()))?;
    ensure!(
        sdk_name == "jf-plugin-sdk",
        "{} is the {sdk_name} crate, not jf-plugin-sdk",
        options.sdk.display()
    );

    if options.dir.exists() {
        let empty = std::fs::read_dir(&options.dir)
            .with_context(|| format!("reading {}", options.dir.display()))?
            .next()
            .is_none();
        ensure!(empty, "{} already exists and is not empty", options.dir.display());
    }
    std::fs::create_dir_all(options.dir.join("src"))
        .with_context(|| format!("creating {}", options.dir.display()))?;

    let dir = std::fs::canonicalize(&options.dir)?;
    let sdk = std::fs::canonicalize(&options.sdk)?;
    let sdk_path = relative(&dir, &sdk).unwrap_or(sdk);
    let title = title(name);
    let fill = |template: &str| {
        template
            .replace("{{name}}", name)
            .replace("{{type}}", &type_name(name))
            .replace("{{title}}", &title)
            .replace("{{id}}", &id)
            .replace("{{author}}", &toml_text(&author))
            .replace(
                "{{sdk}}",
                &toml_text(&sdk_path.to_string_lossy().replace('\\', "/")),
            )
    };
    let files = [
        ("Cargo.toml", CARGO_TOML),
        ("src/lib.rs", LIB_RS),
        ("manifest.toml", MANIFEST_TOML),
        ("settings.schema.json", SCHEMA_JSON),
        ("README.md", README_MD),
        (".gitignore", GITIGNORE),
    ];
    let mut written = Vec::new();
    for (file, template) in files {
        let path = options.dir.join(file);
        std::fs::write(&path, fill(template)).with_context(|| format!("writing {}", path.display()))?;
        written.push(path);
    }
    // The template must stay valid: check what was written like the host will.
    let manifest = std::fs::read_to_string(options.dir.join("manifest.toml"))?;
    jf_plugin_host::manifest::parse(&manifest).context("the generated manifest.toml")?;
    let schema = std::fs::read_to_string(options.dir.join("settings.schema.json"))?;
    if let Err(why) = jf_plugin_host::schema::SettingsSchema::parse(&schema) {
        bail!("the generated settings.schema.json: {why}");
    }
    Ok(written)
}

/// The `plugins/sdk` folder of the Jellyfesh checkout `start` is in, or of
/// the checkout this tool was built from.
pub fn find_sdk(start: &Path) -> Option<PathBuf> {
    let start = std::fs::canonicalize(start).unwrap_or_else(|_| start.to_path_buf());
    let built_from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/sdk");
    start
        .ancestors()
        .map(|dir| dir.join("plugins/sdk"))
        .chain(std::iter::once(built_from))
        .find(|dir| dir.join("Cargo.toml").is_file() && dir.join("src/lib.rs").is_file())
        .map(|dir| std::fs::canonicalize(&dir).unwrap_or(dir))
}

fn valid_package_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.as_bytes()[0].is_ascii_lowercase()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// `my-radio_plugin` → `MyRadioPlugin`.
fn type_name(name: &str) -> String {
    name.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// `my-radio_plugin` → `My Radio Plugin`.
fn title(name: &str) -> String {
    name.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Text for inside a TOML basic string.
fn toml_text(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// `to` relative to the folder `from` (both absolute), if they share a root.
fn relative(from: &Path, to: &Path) -> Option<PathBuf> {
    let from: Vec<Component> = from.components().collect();
    let to: Vec<Component> = to.components().collect();
    if from.first() != to.first() {
        return None;
    }
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut out = PathBuf::new();
    for _ in common..from.len() {
        out.push("..");
    }
    for part in &to[common..] {
        out.push(part.as_os_str());
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(valid_package_name("my-radio_2"));
        assert!(!valid_package_name("My"));
        assert!(!valid_package_name("2fast"));
        assert!(!valid_package_name("a b"));
        assert_eq!(type_name("my-radio_plugin"), "MyRadioPlugin");
        assert_eq!(title("my-radio_plugin"), "My Radio Plugin");
        assert_eq!(toml_text(r#"a "b" \c"#), r#"a \"b\" \\c"#);
    }

    #[test]
    fn relative_paths() {
        assert_eq!(
            relative(Path::new("/a/b/c"), Path::new("/a/sdk")),
            Some(PathBuf::from("../../sdk"))
        );
        assert_eq!(
            relative(Path::new("/a"), Path::new("/a/plugins/sdk")),
            Some(PathBuf::from("plugins/sdk"))
        );
    }

    #[test]
    fn writes_a_valid_crate() {
        let dir = tempfile::tempdir().unwrap();
        let sdk = find_sdk(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the SDK of this checkout");
        let target = dir.path().join("radio-paradise");
        let written = scaffold(&Scaffold {
            name: "radio-paradise".into(),
            dir: target.clone(),
            id: None,
            author: Some("Ada \"Countess\" Lovelace".into()),
            sdk,
        })
        .unwrap();
        assert_eq!(written.len(), 6);
        let cargo = std::fs::read_to_string(target.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("name = \"radio-paradise\""));
        assert!(cargo.contains("plugins/sdk\" }"), "{cargo}");
        let manifest = std::fs::read_to_string(target.join("manifest.toml")).unwrap();
        let parsed = jf_plugin_host::manifest::parse(&manifest).unwrap();
        assert_eq!(parsed.id, "org.example.radio-paradise");
        assert_eq!(parsed.author, "Ada \"Countess\" Lovelace");
        let lib = std::fs::read_to_string(target.join("src/lib.rs")).unwrap();
        assert!(lib.contains("export_plugin!(RadioParadise)"));
        // Not over a crate that is already there.
        assert!(
            scaffold(&Scaffold {
                name: "radio-paradise".into(),
                dir: target,
                id: None,
                author: None,
                sdk: find_sdk(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap(),
            })
            .is_err()
        );
    }
}
