//! Errors of the host API.

use serde::{Deserialize, Serialize};

use crate::types::PluginError;

/// Errors from installing, configuring and managing plugins.
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    /// Reading or writing the plugins directory failed.
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
    /// The `.jfpkg` is not a valid package.
    #[error("invalid package: {0}")]
    Package(String),
    /// `manifest.toml` does not follow the schema.
    #[error("invalid manifest: {0}")]
    Manifest(String),
    /// The `SIGNATURE` is malformed or does not match the package.
    #[error("signature: {0}")]
    Signature(String),
    /// A package for an installed plugin is signed by another key (or not at
    /// all) than the one remembered when it was first installed.
    #[error(
        "{0} was installed signed by key {1}; uninstall it before installing a package signed differently"
    )]
    KeyChanged(String, String),
    /// `api_version` names a WIT version this host cannot run.
    #[error("the plugin targets API {found}; this host runs {supported}")]
    ApiVersion {
        /// The manifest's `api_version`.
        found: String,
        /// The host's version, `major.minor`.
        supported: String,
    },
    /// `plugin.wasm` is not a valid WebAssembly component.
    #[error("invalid component: {0}")]
    InvalidComponent(String),
    /// The component imports something the host does not provide (WASI, a
    /// different WIT version) or lacks an export.
    #[error("the component does not fit the jellyfesh:plugin@1 world: {0}")]
    Incompatible(String),
    /// Instantiating the plugin or its first call failed.
    #[error("the plugin failed to start: {0}")]
    Start(String),
    /// The package is not signed by a trusted key and community plugins are
    /// turned off.
    #[error("community plugins are turned off")]
    CommunityDisabled,
    /// No plugin with this id is installed.
    #[error("no plugin with id {0}")]
    NotInstalled(String),
    /// Settings do not match the plugin's settings schema.
    #[error("invalid settings: {0}")]
    Settings(String),
    /// The secret store failed.
    #[error("secret store: {0}")]
    Secrets(String),
    /// Anything else.
    #[error("{0}")]
    Other(String),
}

/// Why a call into a plugin produced no result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum CallError {
    /// No plugin with this id is installed.
    #[error("no plugin with id {0}")]
    NotInstalled(String),
    /// The plugin is turned off; the text says why.
    #[error("the plugin is turned off: {0}")]
    Disabled(String),
    /// The plugin is not signed by a trusted key and community plugins are
    /// turned off.
    #[error("community plugins are turned off")]
    CommunityDisabled,
    /// The manifest does not declare this capability.
    #[error("the plugin does not declare `{0}`")]
    NotDeclared(String),
    /// The plugin answered with an error.
    #[error("{}", plugin_error_text(.0))]
    Plugin(PluginError),
    /// The call used up its CPU budget. The instance was restarted.
    #[error("the plugin used up its CPU budget")]
    CpuLimit,
    /// The plugin tried to grow its memory past the limit. The instance was
    /// restarted.
    #[error("the plugin went over its memory limit")]
    MemoryLimit,
    /// The call ran past its wall-clock deadline. The instance was restarted.
    #[error("the plugin did not answer in time")]
    Timeout,
    /// The plugin trapped (a panic, `unreachable`, a bad memory access). The
    /// instance was restarted.
    #[error("the plugin crashed: {0}")]
    Crashed(String),
    /// The plugin answered with something the host refuses (for example a
    /// stream URL outside its allowlist).
    #[error("the plugin answered with invalid data: {0}")]
    InvalidResponse(String),
    /// The plugin could not be compiled or instantiated.
    #[error("the plugin could not be loaded: {0}")]
    Load(String),
    /// The host was dropped before the call ran.
    #[error("the plugin host has shut down")]
    Shutdown,
    /// The capability is part of Jellyfesh Pro, which is not active
    /// (ADR-0021). The app shows the Pro sheet instead of an error.
    #[error("this needs Jellyfesh Pro")]
    ProRequired,
    /// The caller cancelled the call (a lyrics job).
    #[error("cancelled")]
    Cancelled,
}

impl CallError {
    /// Whether this error counts as a limit violation ("strike"): three in a
    /// row turn the plugin off.
    pub fn is_violation(&self) -> bool {
        matches!(
            self,
            CallError::CpuLimit | CallError::MemoryLimit | CallError::Timeout | CallError::Crashed(_)
        )
    }
}

/// Text for a plugin error, for people.
pub fn plugin_error_text(error: &PluginError) -> String {
    match error {
        PluginError::Unsupported => "not supported by this plugin".into(),
        PluginError::AuthRequired => "you need to log in".into(),
        PluginError::NotFound => "not found".into(),
        PluginError::InvalidInput(text) => format!("invalid input: {text}"),
        PluginError::RateLimited => "the service asked to slow down".into(),
        PluginError::Network(text) => format!("network error: {text}"),
        PluginError::Service(text) => text.clone(),
        PluginError::Internal(text) => format!("plugin error: {text}"),
    }
}
