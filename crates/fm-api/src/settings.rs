//! Settings service: the user's preferences (`config.json`) and the app folders.

use serde::{Deserialize, Serialize};

use crate::activation::{ActivationMode, ConflictPolicy, Target};
use crate::library::KeeperPolicy;
use crate::setup::{AnswerSetupStepRequest, SetupStatus};
use crate::{ApiError, ApiResult, ErrorCode};

/// The smallest preview cache, in megabytes.
pub const CACHE_LIMIT_MB_MIN: u32 = 64;
/// The largest preview cache, in megabytes.
pub const CACHE_LIMIT_MB_MAX: u32 = 16_384;
/// The longest time between automatic rescans, in minutes (one day).
pub const RESCAN_INTERVAL_MAX_MINUTES: u32 = 1_440;
/// The theme of a new install and of a `config.json` from before themes.
pub const DEFAULT_THEME_ID: &str = "paper";

/// The colors of the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    /// Follow the OS appearance.
    #[default]
    System,
    /// Always light.
    Light,
    /// Always dark.
    Dark,
}

/// How much space the lists use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Density {
    /// Room to breathe.
    #[default]
    Comfortable,
    /// More rows on screen.
    Compact,
}

/// The language of the UI. Each value but `System` names a message catalog the
/// desktop app ships (`apps/desktop/src/locales/<tag>.json`); a new catalog adds
/// a value here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Language {
    /// Follow the OS language; English when the app has no catalog for it.
    #[default]
    System,
    /// English.
    En,
    /// German.
    De,
}

/// The size of the interface (E58.5): everything in the app but the fonts the
/// user looks at grows by [`Self::factor`]. Stored in `config.json`; a value
/// this version does not have reads as [`Self::Default`]
/// ([`Settings::dropped_interface_size`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum InterfaceSize {
    /// The size the app is drawn at: 100 per cent.
    #[default]
    Default,
    /// 125 per cent.
    Large,
    /// 150 per cent.
    ExtraLarge,
}

impl InterfaceSize {
    /// Every size, from the smallest.
    pub const ALL: [Self; 3] = [Self::Default, Self::Large, Self::ExtraLarge];

    /// How much larger than [`Self::Default`] the interface is drawn: 1, 1.25
    /// or 1.5. The one place these numbers are written.
    pub fn factor(self) -> f64 {
        match self {
            Self::Default => 1.0,
            Self::Large => 1.25,
            Self::ExtraLarge => 1.5,
        }
    }

    /// This size with its factor, as the API answers it.
    pub fn choice(self) -> InterfaceSizeChoice {
        InterfaceSizeChoice {
            size: self,
            factor: self.factor(),
        }
    }

    /// Every size with its factor, from the smallest
    /// ([`Settings::interface_sizes`]).
    pub fn choices() -> Vec<InterfaceSizeChoice> {
        Self::ALL.into_iter().map(Self::choice).collect()
    }
}

/// An interface size with its factor (E58.5), so no adapter writes the
/// numbers.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceSizeChoice {
    /// The size.
    pub size: InterfaceSize,
    /// Its factor ([`InterfaceSize::factor`]): 1, 1.25 or 1.5.
    pub factor: f64,
}

// The factor is one of the three finite constants of `InterfaceSize::factor`,
// never NaN, so equality is total.
impl Eq for InterfaceSizeChoice {}

/// The user's settings. Missing keys take their defaults.
///
/// The TypeScript type marks every field optional because of the defaults;
/// a response always carries all of them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// What the Activate button does.
    pub activation_mode: ActivationMode,
    /// What activation does when a face clashes with another font.
    pub on_conflict: ConflictPolicy,
    /// Activate again, at start, the faces that were session-active when the
    /// app last ran (off by default: a session activation ends with the app).
    pub reactivate_on_launch: bool,
    /// Light, dark or the OS appearance.
    pub theme: Theme,
    /// Comfortable or compact lists.
    pub density: Density,
    /// The largest size of the preview cache, in megabytes (64–16384).
    pub cache_limit_mb: u32,
    /// Minutes between automatic rescans of the sources; 0 turns them off
    /// (at most 1440, one day).
    pub rescan_interval_minutes: u32,
    /// Closing the main window keeps the app running in the tray (the menu
    /// bar on macOS), so session fonts stay active; off, closing the window
    /// quits. On by default.
    pub keep_in_tray: bool,
    /// Whether the app starts, in the tray, when the user logs in. Read from
    /// the OS's login entry on every request, never stored in `config.json`;
    /// `None` when this system does not offer the option (then the switch
    /// is hidden). Off until the user turns it on.
    #[serde(default)]
    pub start_at_login: Option<bool>,
    /// Check for a new version when the app starts. Off unless the user
    /// turns it on (rule 7: no network call without a user action); where
    /// the app does not update itself the switch is hidden.
    #[serde(default)]
    pub check_updates_automatically: bool,
    /// Install a patch or minor update when the user quits the app (E55.9,
    /// ADR 0012): an automatic check downloads it in the background and the
    /// normal quit installs it. Off by default; it has an effect only with
    /// [`Self::check_updates_automatically`] on and where this copy updates
    /// itself, and turning it on is refused elsewhere
    /// (`crate::update::check_settings_update`).
    #[serde(default)]
    pub install_updates_on_quit: bool,
    /// The language of the UI, the native menus and the tray; `System` by
    /// default.
    #[serde(default)]
    pub language: Language,
    /// The folder catalog installs go into (E22.7): the one folder the app
    /// writes into without a choice per download, chosen once by the user.
    /// `None` until the user chooses it.
    #[serde(default)]
    pub download_folder: Option<String>,
    /// Download fonts a document needs from the catalogs the user turned on
    /// (E29.5): `fonts_needed` with `activate` looks a font the library does
    /// not have up in the cached lists and installs its family into
    /// [`Self::download_folder`]. Off by default.
    #[serde(default)]
    pub fetch_missing_fonts: bool,
    /// The named theme (E47.1), one of [`SettingsService::list_themes`]; next
    /// to [`Self::theme`], which still chooses its light or dark token set.
    /// A `config.json` from before themes gets [`DEFAULT_THEME_ID`] from the
    /// struct's defaults; so does a request while the file names a theme this
    /// version does not ship.
    pub theme_id: String,
    /// The languages the user works in (E56.3): ids of
    /// [`crate::SearchService::list_languages`] presets, in the user's order,
    /// none by default. The Duplicates module's rules and guards that name
    /// "my languages" read them and do nothing while the list is empty
    /// ([`MyLanguagesStatus::NotSet`]). Stored in `config.json`.
    #[serde(default)]
    pub my_languages: Vec<String>,
    /// The ids of [`Self::my_languages`] that the file named but this
    /// version has no preset for, dropped on this read. Never stored: the
    /// read that drops them saves the list without them, so each is reported
    /// once. Empty in every other answer.
    #[serde(default)]
    pub dropped_languages: Vec<String>,
    /// The keeper policy of the Duplicates module (E56.4): the ordered rules
    /// that choose the copy of a duplicate set to keep. The default policy
    /// until the user changes it. Stored in `config.json`.
    #[serde(default)]
    pub keeper_policy: KeeperPolicy,
    /// The size of the interface (E58.5): Default, Large or Extra large.
    /// Stored in `config.json`; `Default` for a file from before sizes.
    #[serde(default)]
    pub interface_size: InterfaceSize,
    /// Every interface size with its factor, from the smallest; the factor of
    /// [`Self::interface_size`] is the one to apply. Never stored: every
    /// answer carries all of them.
    #[serde(default = "InterfaceSize::choices")]
    pub interface_sizes: Vec<InterfaceSizeChoice>,
    /// The interface size the file named but this version does not have, read
    /// as `Default` on this read. Never stored: the read that reports it saves
    /// the file with `Default`, so it is reported once. `None` in every other
    /// answer.
    #[serde(default)]
    pub dropped_interface_size: Option<String>,
}

/// Whether the user has set the languages they work in (E56.3). An answer of
/// a rule or guard that names "my languages" carries it, so the UI can offer
/// to set them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum MyLanguagesStatus {
    /// No language is set: every rule and guard that names "my languages"
    /// did nothing.
    NotSet,
    /// At least one language is set and the rules and guards used them.
    Set,
}

impl MyLanguagesStatus {
    /// The status of this list of preset ids.
    pub fn of(ids: &[String]) -> Self {
        if ids.is_empty() {
            Self::NotSet
        } else {
            Self::Set
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            activation_mode: ActivationMode::Session,
            on_conflict: ConflictPolicy::KeepExisting,
            reactivate_on_launch: false,
            theme: Theme::System,
            density: Density::Comfortable,
            cache_limit_mb: 1_024,
            rescan_interval_minutes: 60,
            keep_in_tray: true,
            start_at_login: None,
            check_updates_automatically: false,
            install_updates_on_quit: false,
            language: Language::System,
            download_folder: None,
            fetch_missing_fonts: false,
            theme_id: DEFAULT_THEME_ID.to_owned(),
            my_languages: Vec::new(),
            dropped_languages: Vec::new(),
            keeper_policy: KeeperPolicy::default(),
            interface_size: InterfaceSize::Default,
            interface_sizes: InterfaceSize::choices(),
            dropped_interface_size: None,
        }
    }
}

/// Change some settings; absent fields keep their value.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsUpdate {
    /// What the Activate button does.
    #[serde(default)]
    pub activation_mode: Option<ActivationMode>,
    /// What activation does when a face clashes with another font.
    #[serde(default)]
    pub on_conflict: Option<ConflictPolicy>,
    /// Activate the last session's faces again at start.
    #[serde(default)]
    pub reactivate_on_launch: Option<bool>,
    /// Light, dark or the OS appearance.
    #[serde(default)]
    pub theme: Option<Theme>,
    /// Comfortable or compact lists.
    #[serde(default)]
    pub density: Option<Density>,
    /// The largest size of the preview cache, in megabytes.
    #[serde(default)]
    pub cache_limit_mb: Option<u32>,
    /// Minutes between automatic rescans; 0 turns them off.
    #[serde(default)]
    pub rescan_interval_minutes: Option<u32>,
    /// Keep running in the tray when the main window is closed.
    #[serde(default)]
    pub keep_in_tray: Option<bool>,
    /// Register (true) or remove (false) the login entry that starts the app
    /// in the tray. Refused where the settings report `start_at_login: None`.
    #[serde(default)]
    pub start_at_login: Option<bool>,
    /// Check for a new version when the app starts.
    #[serde(default)]
    pub check_updates_automatically: Option<bool>,
    /// Install a patch or minor update when the user quits the app. Turning
    /// it on is refused where this copy does not update itself.
    #[serde(default)]
    pub install_updates_on_quit: Option<bool>,
    /// The update control of Settings → About (E55.10): sets
    /// [`Self::check_updates_automatically`] and
    /// [`Self::install_updates_on_quit`] together
    /// ([`crate::update::UpdateLevel::settings`]) and wins over them.
    #[serde(default)]
    pub update_level: Option<crate::update::UpdateLevel>,
    /// The language of the UI.
    #[serde(default)]
    pub language: Option<Language>,
    /// The folder catalog installs go into: an absolute path, or an empty
    /// string to forget it.
    #[serde(default)]
    pub download_folder: Option<String>,
    /// Download fonts a document needs from the catalogs the user turned on.
    #[serde(default)]
    pub fetch_missing_fonts: Option<bool>,
    /// The named theme: the id of one of [`SettingsService::list_themes`];
    /// any other id is refused and nothing changes.
    #[serde(default)]
    pub theme_id: Option<String>,
    /// The languages the user works in: preset ids of
    /// [`crate::SearchService::list_languages`], replacing the list; an empty
    /// list clears it. An id without a preset is refused and nothing changes
    /// ([`unknown_language`]); an id given twice is kept once, where it first
    /// stands.
    #[serde(default)]
    pub my_languages: Option<Vec<String>>,
    /// The keeper policy, replacing the saved one; it is stored normalized
    /// ([`KeeperPolicy::normalized`]). A preferred folder that is not an
    /// absolute path is refused and nothing changes.
    #[serde(default)]
    pub keeper_policy: Option<KeeperPolicy>,
    /// The size of the interface (E58.5). A change is announced as
    /// [`crate::Event::InterfaceSizeChanged`].
    #[serde(default)]
    pub interface_size: Option<InterfaceSize>,
}

impl Settings {
    /// These settings with the update applied, or `InvalidRequest` when a
    /// value is out of range (then nothing changes). Every implementation of
    /// [`SettingsService::update_settings`] uses this, so the rules are one.
    pub fn apply(&self, update: &SettingsUpdate) -> ApiResult<Self> {
        if let Some(mb) = update.cache_limit_mb
            && !(CACHE_LIMIT_MB_MIN..=CACHE_LIMIT_MB_MAX).contains(&mb)
        {
            return Err(ApiError::invalid(format!(
                "The cache size must be between {CACHE_LIMIT_MB_MIN} and {CACHE_LIMIT_MB_MAX} MB."
            )));
        }
        if let Some(minutes) = update.rescan_interval_minutes
            && minutes > RESCAN_INTERVAL_MAX_MINUTES
        {
            return Err(ApiError::invalid(format!(
                "The rescan interval must be between 0 (off) and {RESCAN_INTERVAL_MAX_MINUTES} minutes."
            )));
        }
        let download_folder = match update.download_folder.as_deref().map(str::trim) {
            None => self.download_folder.clone(),
            Some("") => None,
            Some(folder) if !std::path::Path::new(folder).has_root() => {
                return Err(ApiError::invalid(
                    "Choose the folder for installed fonts with its full path.",
                ));
            }
            Some(folder) => Some(folder.to_owned()),
        };
        if update.start_at_login.is_some() && self.start_at_login.is_none() {
            return Err(ApiError::new(
                ErrorCode::Unsupported,
                "Start at login is not available on this system.",
            ));
        }
        if let Some(policy) = &update.keeper_policy {
            policy.check()?;
        }
        let (check_updates_automatically, install_updates_on_quit) = match update.update_level {
            Some(level) => level.settings(),
            None => (
                update
                    .check_updates_automatically
                    .unwrap_or(self.check_updates_automatically),
                update
                    .install_updates_on_quit
                    .unwrap_or(self.install_updates_on_quit),
            ),
        };
        Ok(Self {
            activation_mode: update.activation_mode.unwrap_or(self.activation_mode),
            on_conflict: update.on_conflict.unwrap_or(self.on_conflict),
            reactivate_on_launch: update
                .reactivate_on_launch
                .unwrap_or(self.reactivate_on_launch),
            theme: update.theme.unwrap_or(self.theme),
            density: update.density.unwrap_or(self.density),
            cache_limit_mb: update.cache_limit_mb.unwrap_or(self.cache_limit_mb),
            rescan_interval_minutes: update
                .rescan_interval_minutes
                .unwrap_or(self.rescan_interval_minutes),
            keep_in_tray: update.keep_in_tray.unwrap_or(self.keep_in_tray),
            start_at_login: update.start_at_login.or(self.start_at_login),
            check_updates_automatically,
            install_updates_on_quit,
            language: update.language.unwrap_or(self.language),
            download_folder,
            fetch_missing_fonts: update
                .fetch_missing_fonts
                .unwrap_or(self.fetch_missing_fonts),
            theme_id: update
                .theme_id
                .clone()
                .unwrap_or_else(|| self.theme_id.clone()),
            my_languages: match &update.my_languages {
                Some(ids) => without_repeats(ids),
                None => self.my_languages.clone(),
            },
            dropped_languages: Vec::new(),
            keeper_policy: match &update.keeper_policy {
                Some(policy) => policy.normalized(),
                None => self.keeper_policy.clone(),
            },
            interface_size: update.interface_size.unwrap_or(self.interface_size),
            interface_sizes: InterfaceSize::choices(),
            dropped_interface_size: None,
        })
    }

    /// The event that announces a change of the interface size from `before`
    /// to these settings, or `None` when the size stayed. Every
    /// implementation of [`SettingsService::update_settings`] uses this.
    pub fn interface_size_change(&self, before: InterfaceSize) -> Option<crate::Event> {
        (self.interface_size != before).then(|| crate::Event::InterfaceSizeChanged {
            interface_size: self.interface_size.choice(),
        })
    }
}

/// `ids` with each id kept once, where it first stands.
fn without_repeats(ids: &[String]) -> Vec<String> {
    let mut kept: Vec<String> = Vec::with_capacity(ids.len());
    for id in ids {
        if !kept.contains(id) {
            kept.push(id.clone());
        }
    }
    kept
}

/// The refusal of a language id that [`crate::SearchService::list_languages`]
/// does not have, in [`SettingsUpdate::my_languages`]. Implementations of
/// [`SettingsService::update_settings`] use it, so the answer is one.
pub fn unknown_language(id: &str) -> ApiError {
    ApiError::invalid("This language is not in the list of languages.")
        .with_detail(format!("myLanguages {id:?}"))
}

/// One token set of a theme: a color for each `--fm-*` token of the same
/// name. `#rrggbb`; [`Self::border`] may also be `#rrggbbaa`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ThemeTokens {
    /// Rings, fills, knobs and badges.
    pub accent: String,
    /// Words and marks on the accent; at least 4.5:1 on it.
    pub on_accent: String,
    /// Level 0: the list and the specimens.
    pub bg: String,
    /// Level 1: side panes and bars.
    pub surface: String,
    /// Level 2: popovers, dialogs and cards.
    pub surface_2: String,
    /// Hairlines between rows and panels.
    pub border: String,
}

/// A named theme the app ships, from `data/themes` (E47.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ThemeDefinition {
    /// The value of [`Settings::theme_id`], e.g. `paper`.
    pub id: String,
    /// The name to show, e.g. `Paper`.
    pub name: String,
    /// The tokens for the light appearance.
    pub light: ThemeTokens,
    /// The tokens for the dark appearance.
    pub dark: ThemeTokens,
}

/// The refusal of a theme id that [`SettingsService::list_themes`] does not
/// have. Implementations of [`SettingsService::update_settings`] use it, so
/// the answer is one.
pub fn unknown_theme(id: &str) -> ApiError {
    ApiError::invalid("This theme does not exist.").with_detail(format!("themeId {id:?}"))
}

/// The per-user folders the app keeps its own files in (ADR 0007).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppFolders {
    /// Holds `config.json`.
    pub config: String,
    /// Holds the index database.
    pub data: String,
    /// Holds the preview cache.
    pub cache: String,
    /// Holds the daily log files.
    pub logs: String,
}

/// The local API endpoint (ADR 0015): where it listens and the token every
/// request must carry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EndpointInfo {
    /// `http://127.0.0.1:<port>`; `None` while the endpoint is not running
    /// (the app has not started it yet, or the caller is the CLI's direct link).
    #[serde(default)]
    pub url: Option<String>,
    /// The bearer token: 64 lowercase hex digits (32 random bytes).
    pub token: String,
}

/// The path of the MCP server on the local API endpoint (E27.1).
pub const MCP_PATH: &str = "/mcp";

/// The name the MCP server reports in `initialize`, and the name the config
/// snippets register it under.
pub const MCP_SERVER_NAME: &str = "font-manager";

/// The MCP clients the app writes a config snippet for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum McpClient {
    /// Claude Code: a `claude mcp add --transport http …` command.
    ClaudeCode,
    /// Any client that reads a JSON `mcpServers` block with an HTTP server.
    Http,
    /// Claude Desktop: a JSON `mcpServers` block that starts the stdio shim
    /// (`fm-mcp-stdio`, E27.7) by an absolute path that lasts while the app
    /// is closed. The shim reads the address and token itself, so the block
    /// holds neither.
    ClaudeDesktop,
}

/// A config snippet that connects one MCP client to the app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct McpSnippet {
    /// The client it is for.
    pub client: McpClient,
    /// What to paste: a shell command or a JSON block, with the real URL and
    /// token in it.
    pub text: String,
}

/// The MCP server the app hosts on the local API endpoint (E27.1). Off by
/// default; while off, `/mcp` is not served.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct McpStatus {
    /// Whether the user turned the server on.
    pub enabled: bool,
    /// `http://127.0.0.1:<port>/mcp`; `None` while the local API endpoint is
    /// not running. The port changes from run to run (ADR 0015).
    #[serde(default)]
    pub url: Option<String>,
    /// One snippet per client, with the URL and the endpoint token; empty
    /// while there is no URL.
    #[serde(default)]
    pub snippets: Vec<McpSnippet>,
    /// One grant per tool that changes something (E27.6), in
    /// [`MCP_MUTATING_TOOLS`] order; all off until the user turns one on.
    /// The read-only tools need no grant.
    #[serde(default)]
    pub grants: Vec<McpGrant>,
}

/// The MCP tools that change something; each needs a grant (E27.6).
pub const MCP_MUTATING_TOOLS: [&str; 4] = [
    "activate_fonts",
    "deactivate_fonts",
    "tag_fonts",
    "export_web",
];

/// The mutating MCP tools that also ask the user each time (E27.6).
pub const MCP_CONFIRM_TOOLS: [&str; 1] = ["deactivate_fonts"];

/// How long a call waits for the user's answer to a confirmation, in seconds.
pub const MCP_CONFIRM_TIMEOUT_SECS: u32 = 60;

/// Whether an MCP client may call one tool that changes something.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct McpGrant {
    /// The tool's MCP name, e.g. `deactivate_fonts`.
    pub tool: String,
    /// Whether the user allowed the tool in Settings → AI assistants.
    pub granted: bool,
    /// Whether every call also asks the user (Allow / Deny) first.
    #[serde(default)]
    pub confirm: bool,
}

/// Turn the grant of one mutating MCP tool on or off.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct McpGrantUpdate {
    /// The tool's MCP name; one of [`MCP_MUTATING_TOOLS`].
    pub tool: String,
    /// On (true) or off (false).
    pub granted: bool,
}

/// An MCP tool call that asks whether it may run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct McpToolCall {
    /// The tool's MCP name.
    pub tool: String,
    /// The faces the call acts on, when it names them; the confirmation
    /// says how many.
    #[serde(default)]
    pub target: Option<Target>,
}

/// The user's answer to an [`crate::Event::McpConfirmationRequested`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct McpConfirmationAnswer {
    /// The confirmation.
    pub id: u32,
    /// Allow (true) or Deny (false).
    pub allow: bool,
}

/// Turn the MCP server on or off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct McpUpdate {
    /// On (true) or off (false).
    pub enabled: bool,
}

/// The user's settings, the app folders and the local API endpoint.
pub trait SettingsService {
    /// The current settings.
    fn get_settings(&self) -> ApiResult<Settings>;
    /// Changes some settings and saves them; returns all of them.
    fn update_settings(&self, req: SettingsUpdate) -> ApiResult<Settings>;
    /// Where the app keeps its config, data, cache and logs.
    fn app_folders(&self) -> ApiResult<AppFolders>;
    /// The themes the app ships, the default ([`DEFAULT_THEME_ID`]) first.
    fn list_themes(&self) -> ApiResult<Vec<ThemeDefinition>>;
    /// The local API endpoint's address and token. `Unsupported` where there
    /// is no data folder to keep the token in.
    fn endpoint_info(&self) -> ApiResult<EndpointInfo> {
        Err(endpoint_unsupported())
    }
    /// Makes a new token, keeps it and returns it; the old one fails at once.
    fn regenerate_endpoint_token(&self) -> ApiResult<EndpointInfo> {
        Err(endpoint_unsupported())
    }
    /// Whether the MCP server is on, its URL and the client config snippets.
    /// `Unsupported` where there is no local API.
    fn mcp_status(&self) -> ApiResult<McpStatus> {
        Err(mcp_unsupported())
    }
    /// Turns the MCP server on or off and saves the choice; it applies to
    /// the next request, without a restart.
    fn set_mcp_enabled(&self, req: McpUpdate) -> ApiResult<McpStatus> {
        let _ = req;
        Err(mcp_unsupported())
    }
    /// Turns the grant of one mutating MCP tool on or off and saves it; the
    /// next call sees it, without a restart. `InvalidRequest` for a tool
    /// that needs no grant.
    fn set_mcp_grant(&self, req: McpGrantUpdate) -> ApiResult<McpStatus> {
        let _ = req;
        Err(mcp_unsupported())
    }
    /// Whether an MCP tool call may run now. Read-only tools may; a mutating
    /// tool needs its grant (`PermissionDenied` names the Settings switch),
    /// and a tool in [`MCP_CONFIRM_TOOLS`] also waits up to
    /// [`MCP_CONFIRM_TIMEOUT_SECS`] for the user's Allow. Deny and the
    /// timeout are `PermissionDenied`.
    fn authorize_mcp_tool(&self, req: McpToolCall) -> ApiResult<()> {
        let _ = req;
        Err(mcp_unsupported())
    }
    /// Answers a waiting confirmation. `NotFound` once it was answered or
    /// timed out.
    fn answer_mcp_confirmation(&self, req: McpConfirmationAnswer) -> ApiResult<()> {
        let _ = req;
        Err(mcp_unsupported())
    }
    /// The first-run setup's open steps (E55.2, ADR 0026), from the library's
    /// folders, the catalogs, the settings and the answers in `config.json`;
    /// no network request and no scan. `Unsupported` where there is no setup.
    fn setup_status(&self) -> ApiResult<SetupStatus> {
        Err(setup_unsupported())
    }
    /// Records that the user answered or skipped a setup step, in
    /// `config.json`, and returns the steps still open. `InvalidRequest` for
    /// [`crate::setup::SetupStep::DownloadFolder`], which closes when the folder is chosen.
    fn answer_setup_step(&self, req: AnswerSetupStepRequest) -> ApiResult<SetupStatus> {
        let _ = req;
        Err(setup_unsupported())
    }
}

fn setup_unsupported() -> ApiError {
    ApiError::new(
        ErrorCode::Unsupported,
        "The first-run setup is not available here.",
    )
}

/// The refusal of an answer to [`crate::setup::SetupStep::DownloadFolder`]; every
/// implementation of [`SettingsService::answer_setup_step`] uses it.
pub fn download_folder_is_chosen() -> ApiError {
    ApiError::invalid(
        "The folder for installed fonts is answered by choosing it, not by skipping it.",
    )
}

fn mcp_unsupported() -> ApiError {
    ApiError::new(
        ErrorCode::Unsupported,
        "The MCP server is not available here.",
    )
}

fn endpoint_unsupported() -> ApiError {
    ApiError::new(
        ErrorCode::Unsupported,
        "The local API is not available here.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_session_keep_existing_system_comfortable() {
        let s = Settings::default();
        assert_eq!(s.activation_mode, ActivationMode::Session);
        assert_eq!(s.on_conflict, ConflictPolicy::KeepExisting);
        assert!(!s.reactivate_on_launch);
        assert_eq!(s.theme, Theme::System);
        assert_eq!(s.density, Density::Comfortable);
        assert_eq!(s.cache_limit_mb, 1_024);
        assert_eq!(s.rescan_interval_minutes, 60);
        assert!(s.keep_in_tray);
        assert_eq!(s.start_at_login, None);
        assert!(!s.check_updates_automatically);
    }

    #[test]
    fn the_keeper_policy_is_the_default_until_an_update_replaces_it_normalized() {
        use crate::library::{KeeperPolicy, KeeperRule};
        let old: Settings = serde_json::from_str("{}").expect("old file");
        assert_eq!(old.keeper_policy, KeeperPolicy::default());
        let next = old
            .apply(&SettingsUpdate {
                keeper_policy: Some(KeeperPolicy {
                    rules: vec![KeeperRule::Path, KeeperRule::MostGlyphs],
                    ..KeeperPolicy::default()
                }),
                ..Default::default()
            })
            .expect("apply");
        assert_eq!(
            next.keeper_policy.rules,
            [KeeperRule::MostGlyphs, KeeperRule::Path]
        );
        let kept = next.apply(&SettingsUpdate::default()).expect("apply");
        assert_eq!(kept.keeper_policy, next.keeper_policy);
        let refused = next.apply(&SettingsUpdate {
            keeper_policy: Some(KeeperPolicy {
                preferred_folders: vec!["relative".into()],
                ..KeeperPolicy::default()
            }),
            ..Default::default()
        });
        assert_eq!(
            refused.expect_err("relative folder").code,
            ErrorCode::InvalidRequest
        );
    }

    #[test]
    fn the_interface_size_is_default_for_old_files_and_each_size_has_its_factor() {
        let old: Settings = serde_json::from_str(r#"{"theme":"dark"}"#).expect("old file");
        assert_eq!(old.interface_size, InterfaceSize::Default);
        let factors: Vec<(InterfaceSize, f64)> = old
            .interface_sizes
            .iter()
            .map(|c| (c.size, c.factor))
            .collect();
        assert_eq!(
            factors,
            [
                (InterfaceSize::Default, 1.0),
                (InterfaceSize::Large, 1.25),
                (InterfaceSize::ExtraLarge, 1.5)
            ]
        );
        let json = serde_json::to_value(Settings::default()).expect("json");
        assert_eq!(json["interfaceSize"], "default");
        assert_eq!(
            json["interfaceSizes"],
            serde_json::json!([
                {"size": "default", "factor": 1.0},
                {"size": "large", "factor": 1.25},
                {"size": "extraLarge", "factor": 1.5}
            ])
        );
        let update: SettingsUpdate =
            serde_json::from_str(r#"{"interfaceSize":"extraLarge"}"#).expect("update");
        let next = old.apply(&update).expect("applied");
        assert_eq!(next.interface_size, InterfaceSize::ExtraLarge);
        assert_eq!(next.theme, Theme::Dark, "the other settings stay");
        assert_eq!(
            next.interface_size_change(InterfaceSize::Default),
            Some(crate::Event::InterfaceSizeChanged {
                interface_size: InterfaceSize::ExtraLarge.choice()
            })
        );
        assert_eq!(next.interface_size_change(InterfaceSize::ExtraLarge), None);
        assert!(
            serde_json::from_str::<SettingsUpdate>(r#"{"interfaceSize":"huge"}"#).is_err(),
            "a request with another size is refused"
        );
    }

    #[test]
    fn my_languages_are_empty_for_old_config_files_and_an_update_replaces_them() {
        // A config.json written before E56.3 has no language set.
        let old: Settings = serde_json::from_str(r#"{"theme":"dark"}"#).expect("old config");
        assert!(old.my_languages.is_empty());
        assert_eq!(
            MyLanguagesStatus::of(&old.my_languages),
            MyLanguagesStatus::NotSet
        );
        let json = serde_json::to_value(Settings::default()).expect("json");
        assert_eq!(json["myLanguages"], serde_json::json!([]));
        let set: SettingsUpdate =
            serde_json::from_str(r#"{"myLanguages":["sr-Cyrl","de","sr-Cyrl"]}"#).expect("update");
        let next = old.apply(&set).expect("valid");
        // Kept once, where it first stands.
        assert_eq!(next.my_languages, ["sr-Cyrl", "de"]);
        assert_eq!(
            MyLanguagesStatus::of(&next.my_languages),
            MyLanguagesStatus::Set
        );
        let kept = next.apply(&SettingsUpdate::default()).expect("valid");
        assert_eq!(kept.my_languages, ["sr-Cyrl", "de"]);
        let cleared = kept
            .apply(&SettingsUpdate {
                my_languages: Some(Vec::new()),
                ..SettingsUpdate::default()
            })
            .expect("valid");
        assert!(cleared.my_languages.is_empty());
        // The dropped ids are an answer of one read, never carried on.
        let stale = Settings {
            dropped_languages: vec!["xx".into()],
            ..Settings::default()
        };
        assert!(
            stale
                .apply(&SettingsUpdate::default())
                .expect("valid")
                .dropped_languages
                .is_empty()
        );
    }

    #[test]
    fn automatic_update_checks_are_off_for_old_config_files_and_can_be_turned_on() {
        // A config.json written before the field existed never checks by itself.
        let old: Settings = serde_json::from_str(r#"{"theme":"dark"}"#).expect("old config");
        assert!(!old.check_updates_automatically);
        let json = serde_json::to_value(Settings::default()).expect("json");
        assert_eq!(json["checkUpdatesAutomatically"], false);
        let on: SettingsUpdate =
            serde_json::from_str(r#"{"checkUpdatesAutomatically":true}"#).expect("update");
        let next = Settings::default().apply(&on).expect("valid");
        assert!(next.check_updates_automatically);
        let unchanged = next.apply(&SettingsUpdate::default()).expect("valid");
        assert!(unchanged.check_updates_automatically);
    }

    #[test]
    fn install_on_quit_is_off_for_old_config_files_and_can_be_turned_on() {
        // A config.json written before E55.9 never installs by itself.
        let old: Settings =
            serde_json::from_str(r#"{"checkUpdatesAutomatically":true}"#).expect("old config");
        assert!(!old.install_updates_on_quit);
        assert!(!Settings::default().install_updates_on_quit);
        let json = serde_json::to_value(Settings::default()).expect("json");
        assert_eq!(json["installUpdatesOnQuit"], false);
        let on: SettingsUpdate =
            serde_json::from_str(r#"{"installUpdatesOnQuit":true}"#).expect("update");
        let next = old.apply(&on).expect("valid");
        assert!(next.install_updates_on_quit);
        assert!(
            next.apply(&SettingsUpdate::default())
                .expect("valid")
                .install_updates_on_quit
        );
    }

    #[test]
    fn start_at_login_is_never_written_and_refused_where_not_offered() {
        // Not offered: null on the wire.
        let json = serde_json::to_value(Settings::default()).expect("json");
        assert_eq!(json["startAtLogin"], serde_json::Value::Null);
        let on: SettingsUpdate = serde_json::from_str(r#"{"startAtLogin":true}"#).expect("update");
        let e = Settings::default().apply(&on).expect_err("not offered");
        assert_eq!(e.code, ErrorCode::Unsupported);

        // Offered (the service filled it from the OS): the update applies.
        let offered = Settings {
            start_at_login: Some(false),
            ..Settings::default()
        };
        assert_eq!(
            serde_json::to_value(&offered).expect("json")["startAtLogin"],
            false
        );
        assert_eq!(
            offered.apply(&on).expect("valid").start_at_login,
            Some(true)
        );
        let unchanged = offered.apply(&SettingsUpdate::default()).expect("valid");
        assert_eq!(unchanged.start_at_login, Some(false));
    }

    #[test]
    fn language_is_system_for_old_config_files_and_can_be_chosen() {
        // A config.json written before the field existed follows the OS.
        let old: Settings = serde_json::from_str(r#"{"theme":"dark"}"#).expect("old config");
        assert_eq!(old.language, Language::System);
        assert_eq!(Settings::default().language, Language::System);
        let json = serde_json::to_value(Settings::default()).expect("json");
        assert_eq!(json["language"], "system");
        let de: SettingsUpdate = serde_json::from_str(r#"{"language":"de"}"#).expect("update");
        let next = Settings::default().apply(&de).expect("valid");
        assert_eq!(next.language, Language::De);
        assert_eq!(serde_json::to_value(&next).expect("json")["language"], "de");
        let unchanged = next.apply(&SettingsUpdate::default()).expect("valid");
        assert_eq!(unchanged.language, Language::De);
        let bad: Result<SettingsUpdate, _> = serde_json::from_str(r#"{"language":"xx"}"#);
        assert!(bad.is_err(), "an unknown language is refused");
    }

    #[test]
    fn keep_in_tray_defaults_on_for_old_config_files_and_can_be_turned_off() {
        // A config.json written before the field existed keeps the app in the tray.
        let old: Settings = serde_json::from_str(r#"{"theme":"dark"}"#).expect("old config");
        assert!(old.keep_in_tray);
        let json = serde_json::to_value(Settings::default()).expect("json");
        assert_eq!(json["keepInTray"], true);
        let off: SettingsUpdate = serde_json::from_str(r#"{"keepInTray":false}"#).expect("update");
        let next = Settings::default().apply(&off).expect("valid");
        assert!(!next.keep_in_tray);
        let unchanged = next.apply(&SettingsUpdate::default()).expect("valid");
        assert!(!unchanged.keep_in_tray);
    }

    #[test]
    fn the_theme_id_is_paper_for_old_config_files_and_is_kept_by_other_updates() {
        assert_eq!(Settings::default().theme_id, "paper");
        // A config.json written before the field existed gets the default.
        let old: Settings = serde_json::from_str(r#"{"theme":"dark"}"#).expect("old config");
        assert_eq!(old.theme_id, "paper");
        assert_eq!(old.theme, Theme::Dark, "the old setting is untouched");
        let json = serde_json::to_value(Settings::default()).expect("json");
        assert_eq!(json["themeId"], "paper");
        assert_eq!(json["theme"], "system");

        let moss: SettingsUpdate = serde_json::from_str(r#"{"themeId":"moss"}"#).expect("update");
        assert_eq!(moss.theme, None);
        let next = Settings::default().apply(&moss).expect("valid");
        assert_eq!(next.theme_id, "moss");
        assert_eq!(next.theme, Theme::System, "the two settings are separate");
        let other = next
            .apply(&SettingsUpdate {
                theme: Some(Theme::Dark),
                ..SettingsUpdate::default()
            })
            .expect("valid");
        assert_eq!(other.theme_id, "moss", "kept by another update");
    }

    #[test]
    fn theme_tokens_are_camel_case_on_the_wire() {
        let themes = crate::mock::MockApi::new().list_themes().expect("themes");
        let json = serde_json::to_value(&themes[0]).expect("json");
        for key in ["accent", "onAccent", "bg", "surface", "surface2", "border"] {
            assert!(json["light"][key].is_string(), "light {key}");
            assert!(json["dark"][key].is_string(), "dark {key}");
        }
    }

    #[test]
    fn endpoint_info_is_camel_case_and_the_url_may_be_absent() {
        let info = EndpointInfo {
            url: Some("http://127.0.0.1:4242".into()),
            token: "ab".repeat(32),
        };
        let json = serde_json::to_value(&info).expect("json");
        assert_eq!(json["url"], "http://127.0.0.1:4242");
        assert_eq!(json["token"], "ab".repeat(32));
        let stopped: EndpointInfo = serde_json::from_str(r#"{"token":"00"}"#).expect("parse");
        assert_eq!(stopped.url, None);
    }

    #[test]
    fn the_mock_regenerates_a_new_64_digit_token() {
        let api = crate::mock::MockApi::new();
        let first = api.endpoint_info().expect("info");
        assert_eq!(first.token.len(), 64);
        assert!(first.token.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(
            api.endpoint_info().expect("info"),
            first,
            "stable until regenerated"
        );
        let next = api.regenerate_endpoint_token().expect("regenerate");
        assert_ne!(next.token, first.token);
        assert_eq!(next.token.len(), 64);
        assert_eq!(api.endpoint_info().expect("info"), next);
    }

    #[test]
    fn mcp_status_is_camel_case_and_off_in_the_mock_until_turned_on() {
        let api = crate::mock::MockApi::new();
        let off = api.mcp_status().expect("status");
        assert!(!off.enabled);
        let json = serde_json::to_value(&off).expect("json");
        assert_eq!(json["enabled"], false);
        assert_eq!(json["url"], "http://127.0.0.1:47613/mcp");
        assert_eq!(json["snippets"][0]["client"], "claudeCode");
        assert_eq!(json["snippets"][1]["client"], "http");
        assert_eq!(json["snippets"][2]["client"], "claudeDesktop");
        let on = api
            .set_mcp_enabled(McpUpdate { enabled: true })
            .expect("on");
        assert!(on.enabled);
        assert_eq!(api.mcp_status().expect("status"), on);
        let old: McpStatus = serde_json::from_str(r#"{"enabled":true}"#).expect("parse");
        assert_eq!(old.url, None);
        assert!(old.snippets.is_empty());
        assert!(old.grants.is_empty(), "grants default to none");
    }

    #[test]
    fn every_mutating_tool_has_a_grant_off_until_turned_on_in_the_mock() {
        let api = crate::mock::MockApi::new();
        let status = api.mcp_status().expect("status");
        let json = serde_json::to_value(&status).expect("json");
        assert_eq!(json["grants"][1]["tool"], "deactivate_fonts");
        assert_eq!(json["grants"][1]["granted"], false);
        assert_eq!(json["grants"][1]["confirm"], true);
        let tools: Vec<&str> = status.grants.iter().map(|g| g.tool.as_str()).collect();
        assert_eq!(tools, MCP_MUTATING_TOOLS);
        assert!(status.grants.iter().all(|g| !g.granted));

        let call = |tool: &str| McpToolCall {
            tool: tool.into(),
            target: None,
        };
        api.authorize_mcp_tool(call("get_font"))
            .expect("read tools always run");
        let e = api.authorize_mcp_tool(call("tag_fonts")).expect_err("off");
        assert_eq!(e.code, ErrorCode::PermissionDenied);
        let on = api
            .set_mcp_grant(McpGrantUpdate {
                tool: "tag_fonts".into(),
                granted: true,
            })
            .expect("on");
        assert!(on.grants.iter().any(|g| g.tool == "tag_fonts" && g.granted));
        api.authorize_mcp_tool(call("tag_fonts")).expect("granted");
        let e = api
            .set_mcp_grant(McpGrantUpdate {
                tool: "get_font".into(),
                granted: true,
            })
            .expect_err("needs no grant");
        assert_eq!(e.code, ErrorCode::InvalidRequest);
        let e = api
            .answer_mcp_confirmation(McpConfirmationAnswer { id: 1, allow: true })
            .expect_err("nothing waits");
        assert_eq!(e.code, ErrorCode::NotFound);
    }

    #[test]
    fn apply_changes_only_the_given_fields() {
        let next = Settings::default()
            .apply(&SettingsUpdate {
                theme: Some(Theme::Dark),
                cache_limit_mb: Some(CACHE_LIMIT_MB_MIN),
                rescan_interval_minutes: Some(0),
                ..SettingsUpdate::default()
            })
            .expect("valid");
        assert_eq!(next.theme, Theme::Dark);
        assert_eq!(next.cache_limit_mb, CACHE_LIMIT_MB_MIN);
        assert_eq!(next.rescan_interval_minutes, 0);
        assert_eq!(next.density, Density::Comfortable);
        assert_eq!(next.activation_mode, ActivationMode::Session);
    }

    #[test]
    fn apply_rejects_out_of_range_values() {
        let s = Settings::default();
        for update in [
            SettingsUpdate {
                cache_limit_mb: Some(CACHE_LIMIT_MB_MIN - 1),
                ..SettingsUpdate::default()
            },
            SettingsUpdate {
                cache_limit_mb: Some(CACHE_LIMIT_MB_MAX + 1),
                theme: Some(Theme::Dark),
                ..SettingsUpdate::default()
            },
            SettingsUpdate {
                rescan_interval_minutes: Some(RESCAN_INTERVAL_MAX_MINUTES + 1),
                ..SettingsUpdate::default()
            },
        ] {
            let err = s.apply(&update).expect_err("out of range");
            assert_eq!(err.code, ErrorCode::InvalidRequest);
        }
    }

    #[test]
    fn fetching_missing_fonts_is_off_by_default_turned_on_and_kept_by_other_updates() {
        assert!(!Settings::default().fetch_missing_fonts);
        let old: Settings = serde_json::from_str(r#"{"theme":"dark"}"#).expect("old file");
        assert!(!old.fetch_missing_fonts, "a config.json from before E29.5");
        let on: SettingsUpdate =
            serde_json::from_value(serde_json::json!({ "fetchMissingFonts": true }))
                .expect("update");
        let next = Settings::default().apply(&on).expect("valid");
        assert!(next.fetch_missing_fonts);
        assert_eq!(
            serde_json::to_value(&next).expect("json")["fetchMissingFonts"],
            true
        );
        let other = next
            .apply(&SettingsUpdate {
                theme: Some(Theme::Light),
                ..SettingsUpdate::default()
            })
            .expect("valid");
        assert!(other.fetch_missing_fonts, "kept by another update");
        let off = other
            .apply(&SettingsUpdate {
                fetch_missing_fonts: Some(false),
                ..SettingsUpdate::default()
            })
            .expect("valid");
        assert!(!off.fetch_missing_fonts);
    }

    #[test]
    fn the_download_folder_is_empty_by_default_set_once_and_kept_by_other_updates() {
        assert_eq!(Settings::default().download_folder, None);
        let old: Settings = serde_json::from_str(r#"{"theme":"dark"}"#).expect("old file");
        assert_eq!(old.download_folder, None, "a config.json from before E22.7");
        let folder = std::env::temp_dir().join("Font Fu Fonts");
        let folder = folder.to_string_lossy().into_owned();
        let set: SettingsUpdate =
            serde_json::from_value(serde_json::json!({ "downloadFolder": folder }))
                .expect("update");
        let next = Settings::default().apply(&set).expect("valid");
        assert_eq!(next.download_folder.as_deref(), Some(folder.as_str()));
        let json = serde_json::to_value(&next).expect("json");
        assert_eq!(json["downloadFolder"], folder.as_str());

        // Another update keeps it; an empty string forgets it.
        let other = next
            .apply(&SettingsUpdate {
                theme: Some(Theme::Light),
                ..SettingsUpdate::default()
            })
            .expect("valid");
        assert_eq!(other.download_folder.as_deref(), Some(folder.as_str()));
        let cleared = other
            .apply(&SettingsUpdate {
                download_folder: Some("  ".into()),
                ..SettingsUpdate::default()
            })
            .expect("valid");
        assert_eq!(cleared.download_folder, None);
    }

    #[test]
    fn a_relative_download_folder_is_refused_and_nothing_changes() {
        let e = Settings::default()
            .apply(&SettingsUpdate {
                download_folder: Some("Fonts".into()),
                theme: Some(Theme::Dark),
                ..SettingsUpdate::default()
            })
            .expect_err("relative");
        assert_eq!(e.code, ErrorCode::InvalidRequest);
    }

    #[test]
    fn wire_format_is_camel_case_with_defaults_for_missing_keys() {
        let json = serde_json::to_value(Settings::default()).expect("json");
        assert_eq!(json["activationMode"], "session");
        assert_eq!(json["onConflict"], "keepExisting");
        assert_eq!(json["reactivateOnLaunch"], false);
        assert_eq!(json["cacheLimitMb"], 1_024);
        assert_eq!(json["rescanIntervalMinutes"], 60);
        let partial: Settings = serde_json::from_str(r#"{"density":"compact"}"#).expect("partial");
        assert_eq!(partial.density, Density::Compact);
        assert_eq!(partial.theme, Theme::System);
        let update: SettingsUpdate = serde_json::from_str(r#"{"theme":"light"}"#).expect("update");
        assert_eq!(update.theme, Some(Theme::Light));
        assert_eq!(update.density, None);
        let on: SettingsUpdate =
            serde_json::from_str(r#"{"reactivateOnLaunch":true}"#).expect("update");
        let next = Settings::default().apply(&on).expect("valid");
        assert!(next.reactivate_on_launch);
        assert_eq!(next.theme, Theme::System);
    }
}
