//! Persistent Bun runtime for trusted local Termy TypeScript plugins.

use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    path::{Component, Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex, RwLock, TryLockError,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

const MAX_PROTOCOL_BYTES: usize = 1024 * 1024;
const MAX_PROTOCOL_HANDSHAKE_BYTES: usize = 1024;
const LOAD_TIMEOUT: Duration = Duration::from_secs(90);
const HOST_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const DEFAULT_INVOKE_TIMEOUT_MS: u64 = 10_000;
const MAX_INVOKE_TIMEOUT_MS: u64 = 30_000;
const MAX_INVOKE_QUEUE_WAIT_MS: u64 = 30_000;
const MAX_PLUGIN_COMMANDS: usize = 512;
const MAX_INPUTS_PER_COMMAND: usize = 16;
const MAX_SELECT_OPTIONS: usize = 128;
const MAX_ACTIONS: usize = 32;
pub const MAX_INSTALLED_PLUGINS: usize = 32;
pub const MAX_PLUGIN_SOURCE_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_PLUGIN_SOURCE_FILES: usize = 4_096;
const BUNDLE_CACHE_FORMAT: &[u8] = b"termy-plugin-bundle-v1\0";
const DISABLED_MARKER: &str = ".termy-disabled";
const SOURCE_METADATA_FILE: &str = ".termy-source.json";

const HOST_SOURCE: &str = include_str!("host.ts");
const WORKER_SOURCE: &str = include_str!("worker.ts");
const TYPE_DECLARATIONS: &str = include_str!("termy.d.ts");

#[derive(Clone)]
pub struct PluginRuntime {
    inner: Arc<PluginRuntimeInner>,
}

struct PluginRuntimeInner {
    plugins_dir: Option<PathBuf>,
    refresh: Mutex<()>,
    catalog: RwLock<PluginCatalog>,
    host: Mutex<PluginHostState>,
}

#[derive(Default)]
struct PluginCatalog {
    fingerprint: Option<[u8; 32]>,
    commands: Vec<PluginCommand>,
    revisions: BTreeMap<String, String>,
}

#[derive(Default)]
struct PluginHostState {
    connection: Option<Arc<HostConnection>>,
    next_request_id: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PluginRefresh {
    pub changed: bool,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstalledPlugin {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub enabled: bool,
    pub path: PathBuf,
    pub source: Option<PluginSourceMetadata>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSourceMetadata {
    pub repository_url: String,
    #[serde(default)]
    pub requested_ref: Option<String>,
    pub revision: String,
    #[serde(default)]
    pub subdirectory: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PluginInventory {
    pub plugins: Vec<InstalledPlugin>,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCommand {
    pub plugin_id: String,
    pub plugin_name: String,
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub disabled_reason: Option<String>,
    #[serde(default)]
    pub icon: PluginIcon,
    #[serde(default)]
    pub inputs: Vec<PluginInput>,
    #[serde(default = "default_invoke_timeout_ms")]
    pub timeout_ms: u64,
}

impl PluginCommand {
    pub fn qualified_id(&self) -> String {
        format!("{}.{}", self.plugin_id, self.id)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginIcon {
    #[default]
    Command,
    Play,
    Terminal,
    Folder,
    Link,
    Clipboard,
    Settings,
    Info,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PluginInput {
    Text {
        id: String,
        label: String,
        #[serde(default)]
        placeholder: Option<String>,
        #[serde(default, rename = "defaultValue")]
        default_value: Option<String>,
        #[serde(default)]
        required: bool,
        #[serde(default = "default_text_max_length", rename = "maxLength")]
        max_length: usize,
    },
    Select {
        id: String,
        label: String,
        #[serde(default)]
        placeholder: Option<String>,
        #[serde(default, rename = "defaultValue")]
        default_value: Option<String>,
        #[serde(default)]
        required: bool,
        options: Vec<PluginSelectOption>,
    },
    Confirm {
        id: String,
        label: String,
        #[serde(default, rename = "defaultValue")]
        default_value: bool,
    },
}

impl PluginInput {
    pub fn id(&self) -> &str {
        match self {
            Self::Text { id, .. } | Self::Select { id, .. } | Self::Confirm { id, .. } => id,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Text { label, .. } | Self::Select { label, .. } | Self::Confirm { label, .. } => {
                label
            }
        }
    }

    pub fn placeholder(&self) -> Option<&str> {
        match self {
            Self::Text { placeholder, .. } | Self::Select { placeholder, .. } => {
                placeholder.as_deref()
            }
            Self::Confirm { .. } => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSelectOption {
    pub value: String,
    pub label: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginContext {
    pub working_directory: Option<String>,
    pub active_command: Option<String>,
    pub platform: String,
    pub app_version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "type")]
pub enum PluginAction {
    #[serde(rename = "terminal.run")]
    TerminalRun {
        command: String,
        #[serde(default, rename = "workingDirectory")]
        working_directory: Option<String>,
    },
    #[serde(rename = "termy.command")]
    TermyCommand { command: String },
    #[serde(rename = "clipboard.write")]
    ClipboardWrite { text: String },
    #[serde(rename = "url.open")]
    UrlOpen { url: String },
    #[serde(rename = "toast")]
    Toast {
        level: PluginToastLevel,
        message: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PluginSource {
    id: String,
    root: String,
    cache_key: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginManifestFile {
    api_version: u32,
    id: String,
    name: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    main: Option<String>,
}

type PluginFiles = Vec<(String, Vec<u8>)>;

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum HostRequest<'a> {
    Load {
        id: u64,
        plugins: &'a [PluginSource],
    },
    Invoke {
        id: u64,
        #[serde(rename = "pluginId")]
        plugin_id: &'a str,
        #[serde(rename = "commandId")]
        command_id: &'a str,
        revision: &'a str,
        inputs: &'a BTreeMap<String, Value>,
        context: &'a PluginContext,
    },
}

impl HostRequest<'_> {
    fn id(&self) -> u64 {
        match self {
            Self::Load { id, .. } | Self::Invoke { id, .. } => *id,
        }
    }
}

#[derive(Deserialize)]
struct HostResponse {
    id: u64,
    ok: bool,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HostLoadResult {
    plugins: Vec<HostLoadedPlugin>,
    #[serde(default)]
    errors: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HostLoadedPlugin {
    plugin_id: String,
    commands: Value,
}

#[derive(Deserialize)]
struct HostInvokeResult {
    #[serde(default)]
    actions: Vec<PluginAction>,
}

impl PluginRuntime {
    pub fn new(config_path: Option<&Path>) -> Self {
        let plugins_dir = config_path
            .and_then(Path::parent)
            .map(|parent| parent.join("plugins"));
        Self {
            inner: Arc::new(PluginRuntimeInner {
                plugins_dir,
                refresh: Mutex::new(()),
                catalog: RwLock::new(PluginCatalog::default()),
                host: Mutex::new(PluginHostState::default()),
            }),
        }
    }

    pub fn plugins_directory(&self) -> Option<PathBuf> {
        self.inner.plugins_dir.clone()
    }

    pub fn bun_path(&self) -> Result<Option<PathBuf>, String> {
        resolve_bun_binary()
    }

    pub fn installed_plugins(&self) -> Result<PluginInventory, String> {
        let plugins_dir = self
            .inner
            .plugins_dir
            .as_deref()
            .ok_or_else(|| "Termy config path is unavailable".to_string())?;
        inventory_plugins(plugins_dir)
    }

    pub fn install_from_directory(&self, source: &Path) -> Result<InstalledPlugin, String> {
        self.install_from_directory_inner(source, None)
    }

    pub fn install_from_directory_with_source(
        &self,
        source: &Path,
        source_metadata: PluginSourceMetadata,
    ) -> Result<InstalledPlugin, String> {
        validate_source_metadata(&source_metadata)?;
        self.install_from_directory_inner(source, Some(source_metadata))
    }

    fn install_from_directory_inner(
        &self,
        source: &Path,
        source_metadata: Option<PluginSourceMetadata>,
    ) -> Result<InstalledPlugin, String> {
        let plugins_dir = self
            .inner
            .plugins_dir
            .as_deref()
            .ok_or_else(|| "Termy config path is unavailable".to_string())?;
        let source_directory_metadata = fs::symlink_metadata(source).map_err(|error| {
            format!(
                "Failed to inspect plugin source {}: {error}",
                source.display()
            )
        })?;
        if source_directory_metadata.file_type().is_symlink() || !source_directory_metadata.is_dir()
        {
            return Err("Plugin source must be a real directory, not a symlink".to_string());
        }
        let manifest_path = source.join("plugin.json");
        let manifest_metadata = fs::symlink_metadata(&manifest_path)
            .map_err(|error| format!("Plugin source is missing plugin.json: {error}"))?;
        if manifest_metadata.file_type().is_symlink() || !manifest_metadata.is_file() {
            return Err("plugin.json must be a regular file".to_string());
        }
        let manifest: PluginManifestFile = serde_json::from_slice(
            &fs::read(&manifest_path)
                .map_err(|error| format!("Failed to read plugin.json: {error}"))?,
        )
        .map_err(|error| format!("Invalid plugin.json: {error}"))?;
        let (manifest, files) = inspect_plugin_root(source, &manifest.id)?;

        fs::create_dir_all(plugins_dir).map_err(|error| {
            format!(
                "Failed to create plugin directory {}: {error}",
                plugins_dir.display()
            )
        })?;
        let installed_count = fs::read_dir(plugins_dir)
            .map_err(|error| format!("Failed to read {}: {error}", plugins_dir.display()))?
            .filter_map(Result::ok)
            .filter(|entry| {
                !entry.file_name().to_string_lossy().starts_with('.')
                    && entry.file_type().is_ok_and(|file_type| file_type.is_dir())
            })
            .count();
        if installed_count >= MAX_INSTALLED_PLUGINS {
            return Err(format!(
                "Termy supports at most {MAX_INSTALLED_PLUGINS} installed plugins"
            ));
        }
        let destination = plugins_dir.join(&manifest.id);
        match fs::symlink_metadata(&destination) {
            Ok(_) => return Err(format!("Plugin `{}` is already installed", manifest.id)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Failed to inspect plugin destination {}: {error}",
                    destination.display()
                ));
            }
        }

        let mut random = [0_u8; 8];
        OsRng.fill_bytes(&mut random);
        let temporary =
            plugins_dir.join(format!(".install-{}-{}", manifest.id, hex_digest(&random)));
        let install_result = (|| {
            write_plugin_installation(&temporary, &files, source_metadata.as_ref(), true)?;
            fs::rename(&temporary, &destination)
                .map_err(|error| format!("Failed to finish plugin installation: {error}"))
        })();
        if let Err(error) = install_result {
            let _ = fs::remove_dir_all(&temporary);
            return Err(error);
        }
        self.invalidate_after_management();
        Ok(installed_plugin_from_manifest(
            manifest,
            destination,
            true,
            source_metadata,
            None,
        ))
    }

    pub fn update_plugin_from_directory(
        &self,
        id: &str,
        source: &Path,
        source_metadata: PluginSourceMetadata,
    ) -> Result<InstalledPlugin, String> {
        validate_source_metadata(&source_metadata)?;
        let destination = self.plugin_root_for_management(id)?;
        let enabled = plugin_is_enabled(&destination, id)?;
        let (manifest, files) = inspect_plugin_root(source, id)?;
        let plugins_dir = destination
            .parent()
            .ok_or_else(|| "Managed plugin directory has no parent".to_string())?;
        let mut random = [0_u8; 8];
        OsRng.fill_bytes(&mut random);
        let suffix = hex_digest(&random);
        let temporary = plugins_dir.join(format!(".update-{id}-{suffix}"));
        let backup = plugins_dir.join(format!(".backup-{id}-{suffix}"));

        if let Err(error) =
            write_plugin_installation(&temporary, &files, Some(&source_metadata), enabled)
        {
            let _ = fs::remove_dir_all(&temporary);
            return Err(error);
        }
        if let Err(error) = fs::rename(&destination, &backup) {
            let _ = fs::remove_dir_all(&temporary);
            return Err(format!("Failed to prepare plugin `{id}` update: {error}"));
        }
        if let Err(error) = fs::rename(&temporary, &destination) {
            let restore_error = fs::rename(&backup, &destination).err();
            let _ = fs::remove_dir_all(&temporary);
            return Err(match restore_error {
                Some(restore_error) => format!(
                    "Failed to install plugin `{id}` update: {error}; restoring the previous plugin also failed: {restore_error}"
                ),
                None => format!("Failed to install plugin `{id}` update: {error}"),
            });
        }
        let _ = fs::remove_dir_all(&backup);
        self.clear_plugin_bundle_cache(id);
        self.invalidate_after_management();
        Ok(installed_plugin_from_manifest(
            manifest,
            destination,
            enabled,
            Some(source_metadata),
            None,
        ))
    }

    pub fn set_plugin_enabled(&self, id: &str, enabled: bool) -> Result<(), String> {
        let root = self.plugin_root_for_management(id)?;
        let marker = root.join(DISABLED_MARKER);
        if enabled {
            match fs::remove_file(&marker) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(format!("Failed to enable plugin `{id}`: {error}")),
            }
        } else {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&marker)
            {
                Ok(mut file) => file
                    .write_all(b"Managed by Termy settings.\n")
                    .map_err(|error| format!("Failed to disable plugin `{id}`: {error}"))?,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    let metadata = fs::symlink_metadata(&marker).map_err(|error| {
                        format!("Failed to inspect disabled state for plugin `{id}`: {error}")
                    })?;
                    if metadata.file_type().is_symlink() || !metadata.is_file() {
                        return Err(format!(
                            "Plugin `{id}` has an invalid {DISABLED_MARKER} marker"
                        ));
                    }
                }
                Err(error) => return Err(format!("Failed to disable plugin `{id}`: {error}")),
            }
        }
        self.invalidate_after_management();
        Ok(())
    }

    pub fn uninstall_plugin(&self, id: &str) -> Result<(), String> {
        let root = self.plugin_root_for_management(id)?;
        fs::remove_dir_all(&root)
            .map_err(|error| format!("Failed to uninstall plugin `{id}`: {error}"))?;
        self.clear_plugin_bundle_cache(id);
        self.invalidate_after_management();
        Ok(())
    }

    fn clear_plugin_bundle_cache(&self, id: &str) {
        let Some(plugins_dir) = self.inner.plugins_dir.as_deref() else {
            return;
        };
        let cache = plugins_dir.join(".termy-cache/bundles").join(id);
        let _ = fs::remove_dir_all(cache);
    }

    fn plugin_root_for_management(&self, id: &str) -> Result<PathBuf, String> {
        if !valid_id(id) {
            return Err(format!("Invalid plugin ID `{id}`"));
        }
        let plugins_dir = self
            .inner
            .plugins_dir
            .as_deref()
            .ok_or_else(|| "Termy config path is unavailable".to_string())?;
        let root = plugins_dir.join(id);
        let metadata = fs::symlink_metadata(&root)
            .map_err(|error| format!("Plugin `{id}` is not installed: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(format!("Plugin `{id}` is not a managed plugin directory"));
        }
        Ok(root)
    }

    fn invalidate_after_management(&self) {
        self.inner
            .host
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .connection
            .take();
        self.inner
            .catalog
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .fingerprint = None;
    }

    pub fn commands(&self) -> Vec<PluginCommand> {
        self.inner
            .catalog
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .commands
            .clone()
    }

    pub fn command_with_revision(
        &self,
        plugin_id: &str,
        command_id: &str,
    ) -> Option<(PluginCommand, String)> {
        let catalog = self
            .inner
            .catalog
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let command = catalog
            .commands
            .iter()
            .find(|command| command.plugin_id == plugin_id && command.id == command_id)?
            .clone();
        let revision = catalog.revisions.get(plugin_id)?.clone();
        Some((command, revision))
    }

    pub fn refresh_if_changed(&self) -> PluginRefresh {
        match self.refresh_if_changed_inner() {
            Ok(refresh) => refresh,
            Err(error) => PluginRefresh {
                changed: false,
                errors: vec![error],
            },
        }
    }

    fn refresh_if_changed_inner(&self) -> Result<PluginRefresh, String> {
        let Some(plugins_dir) = self.inner.plugins_dir.as_deref() else {
            return Ok(PluginRefresh::default());
        };
        let _refresh = self
            .inner
            .refresh
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let discovered = discover_plugins(plugins_dir)?;
        let (previous_fingerprint, had_commands) = {
            let catalog = self
                .inner
                .catalog
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            (catalog.fingerprint, !catalog.commands.is_empty())
        };
        let host_needs_restart = if discovered.sources.is_empty() {
            false
        } else {
            let mut host = self
                .inner
                .host
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let needs_restart = host
                .connection
                .as_ref()
                .is_none_or(|connection| connection.is_failed());
            if needs_restart {
                host.connection.take();
            }
            needs_restart
        };
        if previous_fingerprint == Some(discovered.fingerprint) && !host_needs_restart {
            return Ok(PluginRefresh::default());
        }

        ensure_managed_files(plugins_dir)?;
        if discovered.sources.is_empty() {
            let mut host = self
                .inner
                .host
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            host.connection.take();
            let mut catalog = self
                .inner
                .catalog
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            catalog.fingerprint = Some(discovered.fingerprint);
            catalog.commands.clear();
            catalog.revisions.clear();
            let bundles = plugins_dir.join(".termy-cache/bundles");
            if bundles.exists() {
                fs::remove_dir_all(&bundles).map_err(|error| {
                    format!(
                        "Failed to clear plugin bundle cache {}: {error}",
                        bundles.display()
                    )
                })?;
            }
            return Ok(PluginRefresh {
                changed: previous_fingerprint.is_some() || had_commands,
                errors: Vec::new(),
            });
        }

        let (request_id, connection) = {
            let mut host = self
                .inner
                .host
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if host.connection.is_none() {
                host.connection = Some(Arc::new(HostConnection::spawn(plugins_dir)?));
            }
            let request_id = host.next_id();
            let connection = Arc::clone(
                host.connection
                    .as_ref()
                    .expect("plugin host connection must exist"),
            );
            (request_id, connection)
        };
        let request = HostRequest::Load {
            id: request_id,
            plugins: &discovered.sources,
        };
        let load_result = match connection.request::<HostLoadResult>(&request, LOAD_TIMEOUT) {
            Ok(result) => result,
            Err(error) => {
                if matches!(error, HostRequestError::Transport(_)) {
                    let mut host = self
                        .inner
                        .host
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    if host
                        .connection
                        .as_ref()
                        .is_some_and(|current| Arc::ptr_eq(current, &connection))
                    {
                        host.connection.take();
                    }
                }
                return Err(error.into_message());
            }
        };
        let mut errors = load_result.errors;
        let source_revisions = discovered
            .sources
            .iter()
            .map(|source| (source.id.as_str(), source.cache_key.as_str()))
            .collect::<HashMap<_, _>>();
        let mut seen_plugins = HashSet::new();
        let mut commands = Vec::new();
        let mut revisions = BTreeMap::new();
        for loaded in load_result.plugins {
            let Some(revision) = source_revisions.get(loaded.plugin_id.as_str()) else {
                errors.push(format!(
                    "{}: runtime returned an unknown plugin",
                    loaded.plugin_id
                ));
                continue;
            };
            if !seen_plugins.insert(loaded.plugin_id.clone()) {
                errors.push(format!(
                    "{}: runtime returned the plugin more than once",
                    loaded.plugin_id
                ));
                continue;
            }
            let plugin_commands =
                match serde_json::from_value::<Vec<PluginCommand>>(loaded.commands) {
                    Ok(commands) => commands,
                    Err(error) => {
                        errors.push(format!(
                            "{}: invalid command descriptor: {error}",
                            loaded.plugin_id
                        ));
                        continue;
                    }
                };
            if plugin_commands
                .iter()
                .any(|command| command.plugin_id != loaded.plugin_id)
            {
                errors.push(format!(
                    "{}: command descriptor used the wrong plugin ID",
                    loaded.plugin_id
                ));
                continue;
            }
            if let Err(error) = validate_commands(&plugin_commands) {
                errors.push(format!("{}: {error}", loaded.plugin_id));
                continue;
            }
            commands.extend(plugin_commands);
            revisions.insert(loaded.plugin_id, (*revision).to_string());
        }
        validate_commands(&commands)?;
        let mut catalog = self
            .inner
            .catalog
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        catalog.fingerprint = errors.is_empty().then_some(discovered.fingerprint);
        catalog.commands = commands;
        catalog.revisions = revisions;
        Ok(PluginRefresh {
            changed: true,
            errors,
        })
    }

    pub fn invoke(
        &self,
        plugin_id: &str,
        command_id: &str,
        expected_revision: &str,
        inputs: BTreeMap<String, Value>,
        context: PluginContext,
    ) -> Result<Vec<PluginAction>, String> {
        let (command, current_revision) = self
            .command_with_revision(plugin_id, command_id)
            .ok_or_else(|| format!("Plugin command {plugin_id}.{command_id} is not available"))?;
        if current_revision != expected_revision {
            return Err(
                "Plugin changed while its input form was open; run the command again".to_string(),
            );
        }
        validate_inputs(&command, &inputs)?;
        let timeout_ms = command.timeout_ms.clamp(100, MAX_INVOKE_TIMEOUT_MS);
        let (request_id, connection) = {
            let mut host = self
                .inner
                .host
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let request_id = host.next_id();
            let Some(connection) = host.connection.as_ref().map(Arc::clone) else {
                return Err(
                    "Plugin runtime is unavailable; reopen the command palette to reload plugins"
                        .to_string(),
                );
            };
            (request_id, connection)
        };
        let request = HostRequest::Invoke {
            id: request_id,
            plugin_id,
            command_id,
            revision: expected_revision,
            inputs: &inputs,
            context: &context,
        };
        let timeout = Duration::from_millis(
            timeout_ms
                .saturating_add(MAX_INVOKE_QUEUE_WAIT_MS)
                .saturating_add(1_000),
        );
        let invoke_result = match connection.request::<HostInvokeResult>(&request, timeout) {
            Ok(result) => result,
            Err(error) => {
                if matches!(error, HostRequestError::Transport(_)) {
                    let mut host = self
                        .inner
                        .host
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    if host
                        .connection
                        .as_ref()
                        .is_some_and(|current| Arc::ptr_eq(current, &connection))
                    {
                        host.connection.take();
                    }
                }
                if !matches!(error, HostRequestError::Local(_)) {
                    self.inner
                        .catalog
                        .write()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .fingerprint = None;
                }
                return Err(error.into_message());
            }
        };
        validate_actions(&invoke_result.actions)?;
        Ok(invoke_result.actions)
    }
}

impl PluginHostState {
    fn next_id(&mut self) -> u64 {
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        self.next_request_id
    }
}

struct HostConnection {
    child: Arc<Mutex<Child>>,
    writer: Mutex<TcpStream>,
    pending: PendingHostRequests,
    failed: Arc<AtomicBool>,
    failure: Arc<Mutex<Option<String>>>,
}

#[derive(Clone, Debug)]
enum HostRequestError {
    Local(String),
    Remote(String),
    Transport(String),
}

type HostResponseSender = mpsc::Sender<Result<Value, HostRequestError>>;
type PendingHostRequests = Arc<Mutex<HashMap<u64, HostResponseSender>>>;

impl HostRequestError {
    fn into_message(self) -> String {
        match self {
            Self::Local(message) | Self::Remote(message) | Self::Transport(message) => message,
        }
    }
}

fn valid_protocol_handshake(line: &str, bytes: usize, protocol_secret: &str) -> bool {
    bytes <= MAX_PROTOCOL_HANDSHAKE_BYTES
        && line.ends_with('\n')
        && serde_json::from_str::<Value>(line)
            .ok()
            .and_then(|value| {
                value
                    .get("secret")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .as_deref()
            == Some(protocol_secret)
}

fn read_protocol_handshake(
    stream: &mut TcpStream,
    protocol_secret: &str,
    deadline: Instant,
) -> bool {
    let mut frame = Vec::with_capacity(MAX_PROTOCOL_HANDSHAKE_BYTES);
    let mut buffer = [0_u8; 256];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return false;
        }
        match stream.read(&mut buffer) {
            Ok(0) => return false,
            Ok(bytes) => {
                if frame.len().saturating_add(bytes) > MAX_PROTOCOL_HANDSHAKE_BYTES {
                    return false;
                }
                frame.extend_from_slice(&buffer[..bytes]);
                if let Some(newline) = frame.iter().position(|byte| *byte == b'\n') {
                    if newline + 1 != frame.len() {
                        return false;
                    }
                    let Ok(line) = std::str::from_utf8(&frame) else {
                        return false;
                    };
                    return valid_protocol_handshake(line, frame.len(), protocol_secret);
                }
                if frame.len() == MAX_PROTOCOL_HANDSHAKE_BYTES {
                    return false;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(remaining.min(Duration::from_millis(2)));
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return false,
        }
    }
}

fn write_protocol_frame(
    stream: &mut TcpStream,
    frame: &[u8],
    deadline: Instant,
) -> std::io::Result<()> {
    let mut written = 0;
    while written < frame.len() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "plugin protocol write deadline elapsed",
            ));
        }
        stream.set_write_timeout(Some(remaining))?;
        match stream.write(&frame[written..]) {
            Ok(0) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "failed to write plugin protocol frame",
                ));
            }
            Ok(bytes) => written += bytes,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    stream.flush()
}

impl HostConnection {
    fn spawn(plugins_dir: &Path) -> Result<Self, String> {
        let bun = resolve_bun_binary()?.ok_or_else(|| {
            "TypeScript plugins require Bun; install Bun or set TERMY_BUN_PATH".to_string()
        })?;
        let runtime_dir = plugins_dir.join(".termy-runtime");
        let host_path = runtime_dir.join("host.ts");
        let worker_path = runtime_dir.join("worker.ts");
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|error| format!("Failed to create plugin protocol socket: {error}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("Failed to configure plugin protocol socket: {error}"))?;
        let protocol_port = listener
            .local_addr()
            .map_err(|error| format!("Failed to inspect plugin protocol socket: {error}"))?
            .port();
        let mut secret = [0_u8; 32];
        OsRng.fill_bytes(&mut secret);
        let protocol_secret = hex_digest(&secret);
        let mut command = Command::new(&bun);
        command
            .arg("run")
            .arg("--no-env-file")
            .arg("--no-install")
            .arg(&host_path)
            .current_dir(plugins_dir)
            .env_clear()
            .env("TERMY_PLUGIN_WORKER_PATH", &worker_path)
            .env("TERMY_PLUGIN_PROTOCOL_PORT", protocol_port.to_string())
            .env("TERMY_PLUGIN_PROTOCOL_SECRET", &protocol_secret)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        copy_safe_environment(&mut command);
        let child = command
            .spawn()
            .map_err(|error| format!("Failed to start Bun plugin runtime: {error}"))?;
        let child = Arc::new(Mutex::new(child));
        let deadline = Instant::now() + HOST_CONNECT_TIMEOUT;
        let (writer, mut reader) = loop {
            if Instant::now() >= deadline {
                if let Ok(mut child) = child.lock() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                return Err("Bun plugin runtime did not connect in time".to_string());
            }
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream.set_nonblocking(true).map_err(|error| {
                        format!("Failed to configure plugin protocol handshake: {error}")
                    })?;
                    stream.set_nodelay(true).map_err(|error| {
                        format!("Failed to configure plugin protocol connection: {error}")
                    })?;
                    if !read_protocol_handshake(&mut stream, protocol_secret.as_str(), deadline) {
                        continue;
                    }
                    stream.set_nonblocking(false).map_err(|error| {
                        format!("Failed to configure plugin protocol connection: {error}")
                    })?;
                    let reader_stream = stream.try_clone().map_err(|error| {
                        format!("Failed to clone plugin protocol connection: {error}")
                    })?;
                    break (stream, BufReader::new(reader_stream));
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if let Ok(mut child) = child.lock()
                        && let Ok(Some(status)) = child.try_wait()
                    {
                        return Err(format!(
                            "Bun plugin runtime exited before connecting ({status})"
                        ));
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => {
                    return Err(format!(
                        "Failed to accept plugin protocol connection: {error}"
                    ));
                }
            }
        };
        reader
            .get_mut()
            .set_read_timeout(None)
            .map_err(|error| format!("Failed to configure plugin protocol reader: {error}"))?;
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let failed = Arc::new(AtomicBool::new(false));
        let failure = Arc::new(Mutex::new(None));
        spawn_host_reader(
            reader,
            Arc::clone(&child),
            Arc::clone(&pending),
            Arc::clone(&failed),
            Arc::clone(&failure),
        );
        Ok(Self {
            child,
            writer: Mutex::new(writer),
            pending,
            failed,
            failure,
        })
    }

    fn is_failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }

    fn request<T: DeserializeOwned>(
        &self,
        request: &HostRequest<'_>,
        timeout: Duration,
    ) -> Result<T, HostRequestError> {
        let deadline = Instant::now() + timeout;
        let mut encoded = serde_json::to_vec(request).map_err(|error| {
            HostRequestError::Local(format!("Failed to encode plugin request: {error}"))
        })?;
        encoded.push(b'\n');
        if encoded.len() > MAX_PROTOCOL_BYTES {
            return Err(HostRequestError::Local(
                "Plugin request exceeds the 1 MiB protocol limit".to_string(),
            ));
        }
        let (response_tx, response_rx) = mpsc::channel();
        {
            let mut pending = self
                .pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if self.failed.load(Ordering::Acquire) {
                return Err(HostRequestError::Transport(self.failure_message()));
            }
            pending.insert(request.id(), response_tx);
        }

        let timeout_error = || {
            self.pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(&request.id());
            let message = format!("Plugin runtime timed out after {} ms", timeout.as_millis());
            fail_host_connection(
                &self.child,
                &self.pending,
                &self.failed,
                &self.failure,
                &message,
            );
            HostRequestError::Transport(message)
        };

        let mut writer = loop {
            if self.failed.load(Ordering::Acquire) {
                self.pending
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .remove(&request.id());
                return Err(HostRequestError::Transport(self.failure_message()));
            }
            match self.writer.try_lock() {
                Ok(writer) => break writer,
                Err(TryLockError::Poisoned(poisoned)) => break poisoned.into_inner(),
                Err(TryLockError::WouldBlock) => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return Err(timeout_error());
                    }
                    thread::sleep(remaining.min(Duration::from_millis(2)));
                }
            }
        };
        let write_timeout = deadline.saturating_duration_since(Instant::now());
        if write_timeout.is_zero() {
            return Err(timeout_error());
        }
        let write_result = write_protocol_frame(&mut writer, &encoded, deadline);
        let _ = writer.set_write_timeout(None);
        drop(writer);
        if let Err(error) = write_result {
            self.pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(&request.id());
            let message = format!("Failed to write to plugin runtime: {error}");
            fail_host_connection(
                &self.child,
                &self.pending,
                &self.failed,
                &self.failure,
                &message,
            );
            return Err(HostRequestError::Transport(message));
        }
        let response_timeout = deadline.saturating_duration_since(Instant::now());
        if response_timeout.is_zero() {
            return Err(timeout_error());
        }
        let value = match response_rx.recv_timeout(response_timeout) {
            Ok(result) => result?,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err(timeout_error());
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(HostRequestError::Transport(self.failure_message()));
            }
        };
        serde_json::from_value(value).map_err(|error| {
            HostRequestError::Remote(format!(
                "Plugin runtime returned an invalid result: {error}"
            ))
        })
    }

    fn failure_message(&self) -> String {
        self.failure
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
            .unwrap_or_else(|| "Plugin runtime connection is unavailable".to_string())
    }
}

fn spawn_host_reader(
    mut reader: BufReader<TcpStream>,
    child: Arc<Mutex<Child>>,
    pending: PendingHostRequests,
    failed: Arc<AtomicBool>,
    failure: Arc<Mutex<Option<String>>>,
) {
    thread::spawn(move || {
        loop {
            let mut line = String::new();
            let bytes = match (&mut reader)
                .take((MAX_PROTOCOL_BYTES + 1) as u64)
                .read_line(&mut line)
            {
                Ok(bytes) => bytes,
                Err(error) => {
                    fail_host_connection(
                        &child,
                        &pending,
                        &failed,
                        &failure,
                        &format!("Failed to read from plugin runtime: {error}"),
                    );
                    return;
                }
            };
            if bytes == 0 {
                fail_host_connection(
                    &child,
                    &pending,
                    &failed,
                    &failure,
                    "Plugin runtime closed its protocol connection unexpectedly",
                );
                return;
            }
            if bytes > MAX_PROTOCOL_BYTES || !line.ends_with('\n') {
                fail_host_connection(
                    &child,
                    &pending,
                    &failed,
                    &failure,
                    "Plugin response exceeds the 1 MiB protocol limit",
                );
                return;
            }
            let response: HostResponse = match serde_json::from_str(&line) {
                Ok(response) => response,
                Err(error) => {
                    fail_host_connection(
                        &child,
                        &pending,
                        &failed,
                        &failure,
                        &format!("Plugin runtime returned invalid JSON: {error}"),
                    );
                    return;
                }
            };
            let Some(response_tx) = pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(&response.id)
            else {
                fail_host_connection(
                    &child,
                    &pending,
                    &failed,
                    &failure,
                    &format!(
                        "Plugin runtime returned unknown response ID {}",
                        response.id
                    ),
                );
                return;
            };
            let result = if response.ok {
                response.result.ok_or_else(|| {
                    HostRequestError::Transport("Plugin runtime response has no result".to_string())
                })
            } else {
                Err(HostRequestError::Remote(
                    response
                        .error
                        .unwrap_or_else(|| "Plugin command failed".to_string()),
                ))
            };
            let _ = response_tx.send(result);
        }
    });
}

fn fail_host_connection(
    child: &Arc<Mutex<Child>>,
    pending: &PendingHostRequests,
    failed: &Arc<AtomicBool>,
    failure: &Arc<Mutex<Option<String>>>,
    message: &str,
) {
    if failed.swap(true, Ordering::AcqRel) {
        return;
    }
    *failure
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(message.to_string());
    let requests = pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .drain()
        .map(|(_, sender)| sender)
        .collect::<Vec<_>>();
    for sender in requests {
        let _ = sender.send(Err(HostRequestError::Transport(message.to_string())));
    }
    if let Ok(mut child) = child.lock() {
        let _ = child.kill();
    }
}

impl Drop for HostConnection {
    fn drop(&mut self) {
        if let Ok(stream) = self.writer.lock() {
            let _ = stream.shutdown(Shutdown::Both);
        }
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[derive(Debug)]
struct DiscoveredPlugins {
    sources: Vec<PluginSource>,
    fingerprint: [u8; 32],
}

fn write_plugin_installation(
    destination: &Path,
    files: &PluginFiles,
    source_metadata: Option<&PluginSourceMetadata>,
    enabled: bool,
) -> Result<(), String> {
    fs::create_dir(destination)
        .map_err(|error| format!("Failed to prepare plugin installation: {error}"))?;
    for (relative_path, contents) in files {
        let target = relative_path
            .split('/')
            .fold(destination.to_path_buf(), |path, component| {
                path.join(component)
            });
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "Failed to create plugin directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        fs::write(&target, contents)
            .map_err(|error| format!("Failed to copy plugin file {}: {error}", target.display()))?;
    }
    if let Some(source_metadata) = source_metadata {
        let contents = serde_json::to_vec_pretty(source_metadata)
            .map_err(|error| format!("Failed to encode plugin source metadata: {error}"))?;
        fs::write(destination.join(SOURCE_METADATA_FILE), contents)
            .map_err(|error| format!("Failed to save plugin source metadata: {error}"))?;
    }
    if !enabled {
        fs::write(
            destination.join(DISABLED_MARKER),
            b"Managed by Termy settings.\n",
        )
        .map_err(|error| format!("Failed to preserve disabled plugin state: {error}"))?;
    }
    Ok(())
}

fn validate_source_metadata(metadata: &PluginSourceMetadata) -> Result<(), String> {
    let repository = url::Url::parse(&metadata.repository_url)
        .map_err(|error| format!("Plugin repository URL is invalid: {error}"))?;
    if repository.scheme() != "https" || repository.host_str() != Some("github.com") {
        return Err("Plugin repository URL must use https://github.com".to_string());
    }
    if metadata.revision.len() < 40
        || metadata.revision.len() > 64
        || !metadata
            .revision
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err("Plugin source revision must be a full Git commit hash".to_string());
    }
    if let Some(requested_ref) = metadata.requested_ref.as_deref() {
        validate_text(requested_ref, 256, "source ref")?;
    }
    if !metadata.subdirectory.is_empty()
        && normalized_relative_plugin_path(&metadata.subdirectory).as_deref()
            != Some(metadata.subdirectory.as_str())
    {
        return Err("Plugin source subdirectory must be a normalized relative path".to_string());
    }
    Ok(())
}

fn read_source_metadata(root: &Path) -> Result<Option<PluginSourceMetadata>, String> {
    let path = root.join(SOURCE_METADATA_FILE);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Failed to inspect plugin source metadata: {error}")),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("{SOURCE_METADATA_FILE} must be a regular file"));
    }
    let source_metadata: PluginSourceMetadata = serde_json::from_slice(
        &fs::read(&path)
            .map_err(|error| format!("Failed to read plugin source metadata: {error}"))?,
    )
    .map_err(|error| format!("Invalid plugin source metadata: {error}"))?;
    validate_source_metadata(&source_metadata)?;
    Ok(Some(source_metadata))
}

fn inventory_plugins(plugins_dir: &Path) -> Result<PluginInventory, String> {
    fs::create_dir_all(plugins_dir).map_err(|error| {
        format!(
            "Failed to create plugin directory {}: {error}",
            plugins_dir.display()
        )
    })?;
    let mut entries = fs::read_dir(plugins_dir)
        .map_err(|error| format!("Failed to read {}: {error}", plugins_dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to inspect {}: {error}", plugins_dir.display()))?;
    entries.sort_by_key(|entry| entry.file_name());

    let mut inventory = PluginInventory::default();
    for entry in entries {
        let id = entry.file_name().to_string_lossy().into_owned();
        if id.starts_with('.') {
            continue;
        }
        let root = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("Failed to inspect {}: {error}", root.display()))?;
        if file_type.is_symlink() {
            inventory.errors.push(format!(
                "Plugin source cannot be a symlink: {}",
                root.display()
            ));
            continue;
        }
        if !file_type.is_dir() || !root.join("plugin.json").is_file() {
            continue;
        }
        if !valid_id(&id) {
            inventory
                .errors
                .push(format!("Invalid plugin directory ID `{id}`"));
            continue;
        }
        let enabled = match plugin_is_enabled(&root, &id) {
            Ok(enabled) => enabled,
            Err(error) => {
                inventory.plugins.push(InstalledPlugin {
                    id: id.clone(),
                    name: id,
                    version: None,
                    enabled: false,
                    path: root,
                    source: None,
                    error: Some(error),
                });
                continue;
            }
        };
        match validated_plugin_manifest(&root, &id) {
            Ok((manifest, _)) => match read_source_metadata(&root) {
                Ok(source) => inventory.plugins.push(installed_plugin_from_manifest(
                    manifest, root, enabled, source, None,
                )),
                Err(error) => inventory.plugins.push(installed_plugin_from_manifest(
                    manifest,
                    root,
                    enabled,
                    None,
                    Some(error),
                )),
            },
            Err(error) => inventory.plugins.push(InstalledPlugin {
                id: id.clone(),
                name: id,
                version: None,
                enabled,
                path: root,
                source: None,
                error: Some(error),
            }),
        }
    }
    Ok(inventory)
}

fn installed_plugin_from_manifest(
    manifest: PluginManifestFile,
    path: PathBuf,
    enabled: bool,
    source: Option<PluginSourceMetadata>,
    error: Option<String>,
) -> InstalledPlugin {
    InstalledPlugin {
        id: manifest.id,
        name: manifest.name,
        version: manifest.version,
        enabled,
        path,
        source,
        error,
    }
}

fn inspect_plugin_root(
    root: &Path,
    expected_id: &str,
) -> Result<(PluginManifestFile, PluginFiles), String> {
    let (manifest, entrypoint) = validated_plugin_manifest(root, expected_id)?;
    let files = plugin_tree_files(root)?;
    if !files.iter().any(|(path, _)| path == &entrypoint) {
        return Err(format!("Plugin entrypoint does not exist: {entrypoint}"));
    }
    Ok((manifest, files))
}

fn validated_plugin_manifest(
    root: &Path,
    expected_id: &str,
) -> Result<(PluginManifestFile, String), String> {
    if !valid_id(expected_id) {
        return Err(format!("Invalid plugin ID `{expected_id}`"));
    }
    let manifest_path = root.join("plugin.json");
    let manifest_metadata = fs::symlink_metadata(&manifest_path)
        .map_err(|error| format!("Plugin source is missing plugin.json: {error}"))?;
    if manifest_metadata.file_type().is_symlink() || !manifest_metadata.is_file() {
        return Err("plugin.json must be a regular file".to_string());
    }
    let manifest: PluginManifestFile = serde_json::from_slice(
        &fs::read(&manifest_path)
            .map_err(|error| format!("Failed to read plugin.json: {error}"))?,
    )
    .map_err(|error| format!("Invalid plugin.json: {error}"))?;
    if manifest.api_version != 1 {
        return Err("plugin.json apiVersion must be 1".to_string());
    }
    if manifest.id != expected_id {
        return Err(format!(
            "plugin.json id `{}` must match plugin directory `{expected_id}`",
            manifest.id
        ));
    }
    validate_text(&manifest.name, 200, "name")?;
    if let Some(version) = manifest.version.as_deref() {
        validate_text(version, 100, "version")?;
    }
    let main = manifest.main.as_deref().unwrap_or("plugin.ts");
    validate_text(main, 1_024, "entrypoint")?;
    let entrypoint = normalized_relative_plugin_path(main)
        .ok_or_else(|| "plugin.json main must stay inside the plugin directory".to_string())?;
    let components = entrypoint.split('/').collect::<Vec<_>>();
    let mut entrypoint_path = root.to_path_buf();
    for (index, component) in components.iter().enumerate() {
        entrypoint_path.push(component);
        let metadata = fs::symlink_metadata(&entrypoint_path)
            .map_err(|_| format!("Plugin entrypoint does not exist: {main}"))?;
        let is_last = index + 1 == components.len();
        if metadata.file_type().is_symlink()
            || (is_last && !metadata.is_file())
            || (!is_last && !metadata.is_dir())
        {
            return Err(format!(
                "Plugin entrypoint must be a regular file inside the plugin directory: {main}"
            ));
        }
    }
    Ok((manifest, entrypoint))
}

fn plugin_is_enabled(root: &Path, id: &str) -> Result<bool, String> {
    let marker = root.join(DISABLED_MARKER);
    match fs::symlink_metadata(&marker) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => Err(format!(
            "Plugin `{id}` has an invalid {DISABLED_MARKER} marker"
        )),
        Ok(_) => Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(error) => Err(format!(
            "Failed to inspect disabled state for plugin `{id}`: {error}"
        )),
    }
}

fn normalized_relative_plugin_path(path: &str) -> Option<String> {
    let mut components = Vec::new();
    for component in Path::new(path).components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => components.push(value.to_str()?.to_string()),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!components.is_empty()).then(|| components.join("/"))
}

fn discover_plugins(plugins_dir: &Path) -> Result<DiscoveredPlugins, String> {
    if !plugins_dir.exists() {
        fs::create_dir_all(plugins_dir).map_err(|error| {
            format!(
                "Failed to create plugin directory {}: {error}",
                plugins_dir.display()
            )
        })?;
    }
    let mut entries = fs::read_dir(plugins_dir)
        .map_err(|error| format!("Failed to read {}: {error}", plugins_dir.display()))?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());

    let mut candidates = Vec::new();
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let root = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("Failed to inspect {}: {error}", root.display()))?;
        if file_type.is_symlink() {
            return Err(format!(
                "Plugin source cannot be a symlink: {}",
                root.display()
            ));
        }
        if !file_type.is_dir() || !root.join("plugin.json").is_file() {
            continue;
        }
        let id = name;
        if !valid_id(&id) {
            return Err(format!(
                "Invalid plugin path ID `{id}`; use lowercase letters, numbers, dots, underscores, or hyphens"
            ));
        }
        if !plugin_is_enabled(&root, &id)? {
            continue;
        }
        candidates.push((id, root));
    }
    candidates.sort_by(|left, right| left.0.cmp(&right.0));
    if candidates.len() > MAX_INSTALLED_PLUGINS {
        return Err(format!(
            "Plugin directory contains {} plugins; maximum is {MAX_INSTALLED_PLUGINS}",
            candidates.len()
        ));
    }

    let mut seen = HashSet::new();
    let mut hasher = Sha256::new();
    let mut sources = Vec::with_capacity(candidates.len());
    for (id, root) in candidates {
        if !seen.insert(id.clone()) {
            return Err(format!("Duplicate plugin ID `{id}`"));
        }
        let files = plugin_tree_files(&root)?;
        let mut source_hasher = Sha256::new();
        source_hasher.update(BUNDLE_CACHE_FORMAT);
        for (relative_path, contents) in &files {
            source_hasher.update(relative_path.as_bytes());
            source_hasher.update([0]);
            source_hasher.update(contents);
            source_hasher.update([0]);
        }
        let source_hash = source_hasher.finalize();
        let cache_key = hex_digest(source_hash.as_slice());
        hasher.update(id.as_bytes());
        hasher.update([0]);
        hasher.update(root.to_string_lossy().as_bytes());
        hasher.update([0]);
        hasher.update(source_hash);
        sources.push(PluginSource {
            id,
            root: root.to_string_lossy().into_owned(),
            cache_key,
        });
    }
    Ok(DiscoveredPlugins {
        sources,
        fingerprint: hasher.finalize().into(),
    })
}

fn plugin_tree_files(root: &Path) -> Result<PluginFiles, String> {
    fn visit(
        root: &Path,
        directory: &Path,
        total_bytes: &mut u64,
        files: &mut Vec<(String, Vec<u8>)>,
    ) -> Result<(), String> {
        let mut entries = fs::read_dir(directory)
            .map_err(|error| format!("Failed to read {}: {error}", directory.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Failed to inspect {}: {error}", directory.display()))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let name = entry.file_name();
            if matches!(
                name.to_str(),
                Some(".git" | "node_modules" | DISABLED_MARKER | SOURCE_METADATA_FILE)
            ) {
                continue;
            }
            let file_type = entry.file_type().map_err(|error| {
                format!("Failed to inspect {}: {error}", entry.path().display())
            })?;
            let path = entry.path();
            if file_type.is_dir() {
                visit(root, &path, total_bytes, files)?;
                continue;
            }
            if !file_type.is_file() {
                return Err(format!(
                    "Plugin source contains unsupported file or symlink {}",
                    path.display()
                ));
            }
            let contents = fs::read(&path).map_err(|error| {
                format!("Failed to read plugin file {}: {error}", path.display())
            })?;
            *total_bytes = total_bytes.saturating_add(contents.len() as u64);
            if *total_bytes > MAX_PLUGIN_SOURCE_BYTES {
                return Err(format!(
                    "Plugin {} exceeds the 16 MiB source-tree limit",
                    root.display()
                ));
            }
            let relative_path = path
                .strip_prefix(root)
                .map_err(|_| format!("Plugin file escaped its root: {}", path.display()))?
                .components()
                .map(|component| {
                    component.as_os_str().to_str().ok_or_else(|| {
                        format!("Plugin source path must be valid UTF-8: {}", path.display())
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
                .join("/");
            files.push((relative_path, contents));
            if files.len() > MAX_PLUGIN_SOURCE_FILES {
                return Err(format!(
                    "Plugin {} exceeds the {MAX_PLUGIN_SOURCE_FILES}-file source-tree limit",
                    root.display()
                ));
            }
        }
        Ok(())
    }

    let mut files = Vec::new();
    let mut total_bytes = 0;
    visit(root, root, &mut total_bytes, &mut files)?;
    files.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
    Ok(files)
}

fn ensure_managed_files(plugins_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(plugins_dir).map_err(|error| {
        format!(
            "Failed to create plugin directory {}: {error}",
            plugins_dir.display()
        )
    })?;
    write_if_changed(&plugins_dir.join("termy.d.ts"), TYPE_DECLARATIONS)?;
    let runtime_dir = plugins_dir.join(".termy-runtime");
    fs::create_dir_all(&runtime_dir).map_err(|error| {
        format!(
            "Failed to create plugin runtime directory {}: {error}",
            runtime_dir.display()
        )
    })?;
    write_if_changed(&runtime_dir.join("host.ts"), HOST_SOURCE)?;
    write_if_changed(&runtime_dir.join("worker.ts"), WORKER_SOURCE)
}

fn write_if_changed(path: &Path, contents: &str) -> Result<(), String> {
    if fs::read_to_string(path).ok().as_deref() == Some(contents) {
        return Ok(());
    }
    fs::write(path, contents)
        .map_err(|error| format!("Failed to write managed file {}: {error}", path.display()))
}

fn resolve_bun_binary() -> Result<Option<PathBuf>, String> {
    if let Some(value) = std::env::var_os("TERMY_BUN_PATH") {
        let path = PathBuf::from(value);
        if !path.is_absolute() {
            return Err("TERMY_BUN_PATH must be an absolute path".to_string());
        }
        if !is_executable_file(&path) {
            return Err(format!(
                "TERMY_BUN_PATH is not an executable file: {}",
                path.display()
            ));
        }
        return Ok(Some(path));
    }
    if let Ok(executable) = std::env::current_exe()
        && let Some(parent) = executable.parent()
        && let Some(path) = bun_candidate_in_dir(parent)
    {
        return Ok(Some(path));
    }
    if let Some(path_env) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path_env) {
            if let Some(path) = bun_candidate_in_dir(&directory) {
                return Ok(Some(path));
            }
        }
    }
    let home = dirs::home_dir();
    if let Some(home) = home {
        let candidate = if cfg!(target_os = "windows") {
            home.join(".bun/bin/bun.exe")
        } else {
            home.join(".bun/bin/bun")
        };
        if is_executable_file(&candidate) {
            return Ok(Some(candidate));
        }
    }
    for candidate in ["/opt/homebrew/bin/bun", "/usr/local/bin/bun"] {
        let path = PathBuf::from(candidate);
        if is_executable_file(&path) {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn bun_candidate_in_dir(directory: &Path) -> Option<PathBuf> {
    let names: &[&str] = if cfg!(target_os = "windows") {
        &["bun.exe", "bun"]
    } else {
        &["bun"]
    };
    names
        .iter()
        .map(|name| directory.join(name))
        .find(|path| is_executable_file(path))
}

fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        path.metadata()
            .is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn copy_safe_environment(command: &mut Command) {
    for key in [
        "HOME",
        "USERPROFILE",
        "PATH",
        "SHELL",
        "TMPDIR",
        "TMP",
        "TEMP",
        "LANG",
        "LC_ALL",
        "TERM",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    #[cfg(target_os = "windows")]
    for key in ["SYSTEMROOT", "WINDIR", "COMSPEC", "PATHEXT"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command.env("DO_NOT_TRACK", "1");
}

fn validate_commands(commands: &[PluginCommand]) -> Result<(), String> {
    if commands.len() > MAX_PLUGIN_COMMANDS {
        return Err(format!(
            "Plugin catalog has {} commands; maximum is {MAX_PLUGIN_COMMANDS}",
            commands.len()
        ));
    }
    let mut command_ids = HashSet::new();
    for command in commands {
        if !valid_id(&command.plugin_id) || !valid_id(&command.id) {
            return Err(format!(
                "Plugin command `{}` has an invalid ID",
                command.qualified_id()
            ));
        }
        if !command_ids.insert(command.qualified_id()) {
            return Err(format!(
                "Duplicate plugin command `{}`",
                command.qualified_id()
            ));
        }
        validate_text(&command.plugin_name, 200, "plugin name")?;
        validate_text(&command.title, 300, "command title")?;
        if command.inputs.len() > MAX_INPUTS_PER_COMMAND {
            return Err(format!(
                "Plugin command `{}` has too many inputs",
                command.qualified_id()
            ));
        }
        let mut input_ids = HashSet::new();
        for input in &command.inputs {
            if !valid_id(input.id()) || !input_ids.insert(input.id().to_string()) {
                return Err(format!(
                    "Plugin command `{}` has an invalid or duplicate input ID `{}`",
                    command.qualified_id(),
                    input.id()
                ));
            }
            validate_text(input.label(), 200, "input label")?;
            match input {
                PluginInput::Text { max_length, .. }
                    if *max_length == 0 || *max_length > 16_384 =>
                {
                    return Err(format!(
                        "Plugin command `{}` has an invalid text input maxLength",
                        command.qualified_id()
                    ));
                }
                PluginInput::Select { options, .. } => {
                    if options.is_empty() || options.len() > MAX_SELECT_OPTIONS {
                        return Err(format!(
                            "Plugin command `{}` has an invalid select option count",
                            command.qualified_id()
                        ));
                    }
                    let mut option_values = HashSet::new();
                    for option in options {
                        validate_text(&option.value, 1_024, "select option value")?;
                        validate_text(&option.label, 200, "select option label")?;
                        if !option_values.insert(option.value.clone()) {
                            return Err(format!(
                                "Plugin command `{}` has duplicate select value `{}`",
                                command.qualified_id(),
                                option.value
                            ));
                        }
                    }
                }
                PluginInput::Text { .. } | PluginInput::Confirm { .. } => {}
            }
        }
    }
    Ok(())
}

fn validate_inputs(
    command: &PluginCommand,
    inputs: &BTreeMap<String, Value>,
) -> Result<(), String> {
    let expected = command
        .inputs
        .iter()
        .map(PluginInput::id)
        .collect::<HashSet<_>>();
    if let Some(unknown) = inputs.keys().find(|id| !expected.contains(id.as_str())) {
        return Err(format!(
            "Plugin command `{}` received unknown input `{unknown}`",
            command.qualified_id()
        ));
    }
    for input in &command.inputs {
        match input {
            PluginInput::Text {
                id,
                required,
                max_length,
                ..
            } => {
                let Some(value) = inputs.get(id) else {
                    if *required {
                        return Err(format!("Plugin input `{id}` is required"));
                    }
                    continue;
                };
                let Some(text) = value.as_str() else {
                    return Err(format!("Plugin input `{id}` must be text"));
                };
                if *required && text.trim().is_empty() {
                    return Err(format!("Plugin input `{id}` is required"));
                }
                if text.chars().count() > *max_length {
                    return Err(format!(
                        "Plugin input `{id}` exceeds {max_length} characters"
                    ));
                }
            }
            PluginInput::Select {
                id,
                required,
                options,
                ..
            } => {
                let Some(value) = inputs.get(id) else {
                    if *required {
                        return Err(format!("Plugin input `{id}` is required"));
                    }
                    continue;
                };
                let Some(selected) = value.as_str() else {
                    return Err(format!("Plugin input `{id}` must be a select option"));
                };
                if !options.iter().any(|option| option.value == selected) {
                    return Err(format!(
                        "Plugin input `{id}` contains an unknown select value"
                    ));
                }
            }
            PluginInput::Confirm { id, .. } => {
                if inputs.get(id).and_then(Value::as_bool).is_none() {
                    return Err(format!("Plugin input `{id}` must be true or false"));
                }
            }
        }
    }
    Ok(())
}

fn validate_actions(actions: &[PluginAction]) -> Result<(), String> {
    if actions.len() > MAX_ACTIONS {
        return Err(format!(
            "Plugin returned {} actions; maximum is {MAX_ACTIONS}",
            actions.len()
        ));
    }
    for action in actions {
        match action {
            PluginAction::TerminalRun {
                command,
                working_directory,
            } => {
                validate_text(command, 65_536, "terminal command")?;
                if let Some(directory) = working_directory {
                    validate_text(directory, 4_096, "working directory")?;
                }
            }
            PluginAction::TermyCommand { command } => {
                validate_text(command, 128, "Termy command")?;
            }
            PluginAction::ClipboardWrite { text } => {
                validate_text(text, 262_144, "clipboard text")?;
            }
            PluginAction::UrlOpen { url } => {
                validate_text(url, 8_192, "URL")?;
                let parsed = url::Url::parse(url)
                    .map_err(|error| format!("Plugin returned an invalid URL: {error}"))?;
                if !matches!(parsed.scheme(), "http" | "https") {
                    return Err("Plugin URLs must use http or https".to_string());
                }
            }
            PluginAction::Toast { message, .. } => {
                validate_text(message, 4_096, "toast message")?;
            }
        }
    }
    Ok(())
}

fn validate_text(value: &str, max_chars: usize, label: &str) -> Result<(), String> {
    let length = value.chars().count();
    if value.trim().is_empty() {
        return Err(format!("Plugin {label} cannot be empty"));
    }
    if length > max_chars {
        return Err(format!("Plugin {label} exceeds {max_chars} characters"));
    }
    Ok(())
}

pub fn valid_plugin_id(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    value.len() <= 64
        && (first.is_ascii_lowercase() || first.is_ascii_digit())
        && chars.all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '_' | '-')
        })
}

fn valid_id(value: &str) -> bool {
    valid_plugin_id(value)
}

fn default_invoke_timeout_ms() -> u64 {
    DEFAULT_INVOKE_TIMEOUT_MS
}

fn default_text_max_length() -> usize {
    1_024
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_plugin(plugins: &Path, id: &str, name: &str, source: &str) -> PathBuf {
        let plugin_dir = plugins.join(id);
        fs::create_dir_all(&plugin_dir).expect("create plugin directory");
        fs::write(
            plugin_dir.join("plugin.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "apiVersion": 1,
                "id": id,
                "name": name,
            }))
            .expect("encode manifest"),
        )
        .expect("write manifest");
        fs::write(plugin_dir.join("plugin.ts"), source).expect("write plugin source");
        plugin_dir
    }

    fn bun_is_available() -> bool {
        match resolve_bun_binary() {
            Ok(Some(_)) => true,
            Ok(None) if std::env::var_os("CI").is_some() => {
                panic!("Bun is required for plugin runtime tests in CI")
            }
            Ok(None) => false,
            Err(error) => panic!("Invalid Bun runtime configuration: {error}"),
        }
    }

    #[test]
    fn discovery_is_sorted_and_fingerprints_contents() {
        let temp = TempDir::new().expect("temp dir");
        let plugins = temp.path().join("plugins");
        write_plugin(
            &plugins,
            "z-last",
            "Z Last",
            "export default { commands: [] };",
        );
        let first_dir = write_plugin(
            &plugins,
            "a-first",
            "A First",
            "export { value } from './helper'; export default { commands: [] };",
        );
        fs::write(first_dir.join("helper.ts"), "export const value = 1;")
            .expect("write imported module");

        let first = discover_plugins(&plugins).expect("discover plugins");
        assert_eq!(
            first
                .sources
                .iter()
                .map(|source| source.id.as_str())
                .collect::<Vec<_>>(),
            ["a-first", "z-last"]
        );
        fs::write(first_dir.join("helper.ts"), "export const value = 2;")
            .expect("change imported module");
        let second = discover_plugins(&plugins).expect("rediscover plugins");
        assert_ne!(first.fingerprint, second.fingerprint);
    }

    #[test]
    fn discovery_requires_a_manifest_and_ignores_loose_typescript() {
        let temp = TempDir::new().expect("temp dir");
        let plugins = temp.path().join("plugins");
        fs::create_dir_all(&plugins).expect("create plugins directory");
        fs::write(plugins.join("hello.ts"), "export default {};").expect("write file plugin");
        let ignored = plugins.join("ignored");
        fs::create_dir_all(&ignored).expect("create ignored directory");
        fs::write(ignored.join("plugin.ts"), "export default {};")
            .expect("write unmanifested plugin");
        write_plugin(
            &plugins,
            "hello",
            "Hello",
            "export default { commands: [] };",
        );

        let discovered = discover_plugins(&plugins).expect("discover plugins");
        assert_eq!(discovered.sources.len(), 1);
        assert_eq!(discovered.sources[0].id, "hello");
    }

    #[test]
    fn local_plugin_management_installs_toggles_and_uninstalls() {
        let temp = TempDir::new().expect("temp dir");
        let config_dir = temp.path().join("config");
        fs::create_dir_all(&config_dir).expect("create config directory");
        let config_path = config_dir.join("config.txt");
        fs::write(&config_path, "").expect("write config");
        let source_parent = temp.path().join("source");
        let source = write_plugin(
            &source_parent,
            "managed",
            "Managed Plugin",
            "export default definePlugin({ commands: [] });",
        );
        fs::write(
            source.join("plugin.json"),
            r#"{"apiVersion":1,"id":"managed","name":"Managed Plugin","version":"1.2.3"}"#,
        )
        .expect("write versioned manifest");

        let runtime = PluginRuntime::new(Some(&config_path));
        assert!(
            runtime
                .installed_plugins()
                .expect("empty inventory")
                .plugins
                .is_empty()
        );
        let installed = runtime
            .install_from_directory(&source)
            .expect("install local plugin");
        assert_eq!(installed.id, "managed");
        assert_eq!(installed.version.as_deref(), Some("1.2.3"));
        assert!(installed.enabled);
        assert!(installed.path.join("plugin.ts").is_file());

        runtime
            .set_plugin_enabled("managed", false)
            .expect("disable plugin");
        let disabled = runtime.installed_plugins().expect("disabled inventory");
        assert!(!disabled.plugins[0].enabled);
        assert!(
            discover_plugins(&config_dir.join("plugins"))
                .expect("discover disabled catalog")
                .sources
                .is_empty()
        );

        runtime
            .set_plugin_enabled("managed", true)
            .expect("enable plugin");
        assert_eq!(
            discover_plugins(&config_dir.join("plugins"))
                .expect("discover enabled catalog")
                .sources
                .len(),
            1
        );
        runtime
            .uninstall_plugin("managed")
            .expect("uninstall plugin");
        assert!(
            runtime
                .installed_plugins()
                .expect("inventory after uninstall")
                .plugins
                .is_empty()
        );
        assert!(
            source.is_dir(),
            "uninstall must preserve the selected source"
        );
    }

    #[test]
    fn github_plugin_management_tracks_source_and_updates_atomically() {
        let temp = TempDir::new().expect("temp dir");
        let config_dir = temp.path().join("config");
        fs::create_dir_all(&config_dir).expect("create config directory");
        let config_path = config_dir.join("config.txt");
        fs::write(&config_path, "").expect("write config");
        let first_source = write_plugin(
            &temp.path().join("first-source"),
            "github-plugin",
            "GitHub Plugin",
            "export const revision = 1; export default definePlugin({ commands: [] });",
        );
        let first_metadata = PluginSourceMetadata {
            repository_url: "https://github.com/termy-org/plugins".to_string(),
            requested_ref: Some("main".to_string()),
            revision: "1111111111111111111111111111111111111111".to_string(),
            subdirectory: "github-plugin".to_string(),
        };
        let runtime = PluginRuntime::new(Some(&config_path));
        let installed = runtime
            .install_from_directory_with_source(&first_source, first_metadata.clone())
            .expect("install GitHub plugin");
        assert_eq!(installed.source.as_ref(), Some(&first_metadata));

        let conflicting_source = write_plugin(
            &temp.path().join("conflicting-source"),
            "github-plugin",
            "Replacement",
            "export const revision = 99; export default definePlugin({ commands: [] });",
        );
        let error = runtime
            .install_from_directory_with_source(&conflicting_source, first_metadata.clone())
            .expect_err("conflicting install");
        assert!(error.contains("already installed"));
        assert!(
            fs::read_to_string(installed.path.join("plugin.ts"))
                .expect("installed source")
                .contains("revision = 1")
        );

        runtime
            .set_plugin_enabled("github-plugin", false)
            .expect("disable before update");
        let updated_source = write_plugin(
            &temp.path().join("updated-source"),
            "github-plugin",
            "GitHub Plugin",
            "export const revision = 2; export default definePlugin({ commands: [] });",
        );
        let updated_metadata = PluginSourceMetadata {
            revision: "2222222222222222222222222222222222222222".to_string(),
            ..first_metadata
        };
        let updated = runtime
            .update_plugin_from_directory(
                "github-plugin",
                &updated_source,
                updated_metadata.clone(),
            )
            .expect("update GitHub plugin");
        assert!(!updated.enabled);
        assert_eq!(updated.source.as_ref(), Some(&updated_metadata));
        assert!(
            fs::read_to_string(updated.path.join("plugin.ts"))
                .expect("updated source")
                .contains("revision = 2")
        );
        let inventory = runtime.installed_plugins().expect("updated inventory");
        assert_eq!(
            inventory.plugins[0].source.as_ref(),
            Some(&updated_metadata)
        );
        assert!(!inventory.plugins[0].enabled);
        assert!(
            plugin_tree_files(&updated.path)
                .expect("managed source tree")
                .iter()
                .all(|(path, _)| path != SOURCE_METADATA_FILE)
        );
    }

    #[cfg(unix)]
    #[test]
    fn invalid_disabled_marker_is_reported_and_not_loaded() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("temp dir");
        let plugins = temp.path().join("plugins");
        let plugin = write_plugin(
            &plugins,
            "unsafe-marker",
            "Unsafe marker",
            "export default { commands: [] };",
        );
        let outside = temp.path().join("outside");
        fs::write(&outside, "outside").expect("write marker target");
        symlink(&outside, plugin.join(DISABLED_MARKER)).expect("create marker symlink");

        let inventory = inventory_plugins(&plugins).expect("inventory plugins");
        assert_eq!(inventory.plugins.len(), 1);
        assert!(!inventory.plugins[0].enabled);
        assert!(
            inventory.plugins[0]
                .error
                .as_deref()
                .is_some_and(|error| error.contains("invalid .termy-disabled marker"))
        );
        let error = discover_plugins(&plugins).expect_err("invalid marker must block loading");
        assert!(error.contains("invalid .termy-disabled marker"));
    }

    #[cfg(unix)]
    #[test]
    fn discovery_rejects_symlinked_plugin_sources() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("temp dir");
        let plugins = temp.path().join("plugins");
        let plugin_dir = write_plugin(
            &plugins,
            "linked",
            "Linked",
            "export { value } from './helper.ts'; export default { commands: [] };",
        );
        let outside = temp.path().join("outside.ts");
        fs::write(&outside, "export const value = 1;").expect("write symlink target");
        symlink(&outside, plugin_dir.join("helper.ts")).expect("create source symlink");

        let error = discover_plugins(&plugins).expect_err("symlink must be rejected");
        assert!(error.contains("symlink"), "unexpected error: {error}");
    }

    #[cfg(unix)]
    #[test]
    fn discovery_rejects_symlinked_plugin_roots() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("temp dir");
        let plugins = temp.path().join("plugins");
        let external_plugins = temp.path().join("external-plugins");
        let external = write_plugin(
            &external_plugins,
            "linked",
            "Linked",
            "export default definePlugin({ commands: [] });",
        );
        fs::create_dir_all(&plugins).expect("create plugins directory");
        symlink(external, plugins.join("linked")).expect("create plugin root symlink");

        let error = discover_plugins(&plugins).expect_err("plugin root symlink must be rejected");
        assert!(error.contains("symlink"), "unexpected error: {error}");
    }

    #[test]
    fn protocol_handshake_is_bounded_and_requires_authentication() {
        let secret = "0123456789abcdef";
        let valid = format!("{{\"secret\":\"{secret}\"}}\n");
        assert!(valid_protocol_handshake(&valid, valid.len(), secret));
        assert!(!valid_protocol_handshake(
            valid.trim_end(),
            valid.trim_end().len(),
            secret
        ));
        assert!(!valid_protocol_handshake(
            &valid,
            MAX_PROTOCOL_HANDSHAKE_BYTES + 1,
            secret
        ));
        assert!(!valid_protocol_handshake(&valid, valid.len(), "wrong"));
    }

    #[test]
    fn protocol_handshake_honors_its_absolute_deadline() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind handshake listener");
        let address = listener.local_addr().expect("handshake listener address");
        let peer = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).expect("connect handshake peer");
            stream.write_all(b"{").expect("write partial handshake");
            thread::sleep(Duration::from_millis(250));
        });
        let (mut stream, _) = listener.accept().expect("accept handshake peer");
        stream
            .set_nonblocking(true)
            .expect("make handshake peer nonblocking");
        let started = Instant::now();
        assert!(!read_protocol_handshake(
            &mut stream,
            "secret",
            started + Duration::from_millis(30)
        ));
        assert!(
            started.elapsed() < Duration::from_millis(150),
            "partial handshake exceeded its absolute deadline"
        );
        peer.join().expect("join handshake peer");
    }

    #[test]
    fn discovery_caps_plugin_count_before_starting_workers() {
        let temp = TempDir::new().expect("temp dir");
        let plugins = temp.path().join("plugins");
        for index in 0..=MAX_INSTALLED_PLUGINS {
            write_plugin(
                &plugins,
                &format!("plugin-{index:02}"),
                &format!("Plugin {index}"),
                "export default { commands: [] };",
            );
        }

        let error = discover_plugins(&plugins).expect_err("plugin count must be capped");
        assert!(error.contains("maximum is 32"), "unexpected error: {error}");
    }

    #[test]
    fn managed_sdk_files_are_stable() {
        let temp = TempDir::new().expect("temp dir");
        let plugins = temp.path().join("plugins");
        ensure_managed_files(&plugins).expect("write managed files");
        ensure_managed_files(&plugins).expect("rewrite managed files");
        assert_eq!(
            fs::read_to_string(plugins.join("termy.d.ts")).expect("read declarations"),
            TYPE_DECLARATIONS
        );
        assert_eq!(
            fs::read_to_string(plugins.join(".termy-runtime/host.ts")).expect("read host"),
            HOST_SOURCE
        );
    }

    #[test]
    fn ids_are_strict_and_stable() {
        for valid in ["git-tools", "git.tools", "git_tools", "v2"] {
            assert!(valid_id(valid), "expected valid ID: {valid}");
        }
        for invalid in ["", "Git", "-git", "git tools", "git/tool"] {
            assert!(!valid_id(invalid), "expected invalid ID: {invalid}");
        }
    }

    #[test]
    fn action_validation_rejects_unsafe_url_schemes() {
        let actions = vec![PluginAction::UrlOpen {
            url: "file:///tmp/private".to_string(),
        }];
        assert_eq!(
            validate_actions(&actions),
            Err("Plugin URLs must use http or https".to_string())
        );
    }

    #[test]
    fn concurrent_refreshes_share_one_catalog_load() {
        if !bun_is_available() {
            return;
        }
        let temp = TempDir::new().expect("temp dir");
        let config_path = temp.path().join("config.txt");
        fs::write(&config_path, "").expect("write config");
        let plugins = temp.path().join("plugins");
        write_plugin(
            &plugins,
            "slow-load",
            "Slow Load",
            r#"
await Bun.sleep(300);
export default definePlugin({
  commands: [{ id: "run", title: "Slow Load: Run", run() {} }],
});
"#,
        );

        let runtime = PluginRuntime::new(Some(&config_path));
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let refreshes = (0..2)
            .map(|_| {
                let runtime = runtime.clone();
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    runtime.refresh_if_changed()
                })
            })
            .collect::<Vec<_>>();

        barrier.wait();
        let refreshes = refreshes
            .into_iter()
            .map(|refresh| refresh.join().expect("join refresh"))
            .collect::<Vec<_>>();
        assert!(
            refreshes.iter().all(|refresh| refresh.errors.is_empty()),
            "errors: {:?}",
            refreshes
                .iter()
                .flat_map(|refresh| &refresh.errors)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            refreshes.iter().filter(|refresh| refresh.changed).count(),
            1
        );
        assert_eq!(runtime.commands().len(), 1);
    }

    #[test]
    fn unchanged_refresh_restarts_a_failed_host() {
        if !bun_is_available() {
            return;
        }
        let temp = TempDir::new().expect("temp dir");
        let config_path = temp.path().join("config.txt");
        fs::write(&config_path, "").expect("write config");
        let plugins = temp.path().join("plugins");
        write_plugin(
            &plugins,
            "healthy",
            "Healthy",
            "export default definePlugin({ commands: [] });",
        );

        let runtime = PluginRuntime::new(Some(&config_path));
        let initial = runtime.refresh_if_changed();
        assert!(initial.errors.is_empty(), "errors: {:?}", initial.errors);
        let first_connection = runtime
            .inner
            .host
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .connection
            .as_ref()
            .map(Arc::clone)
            .expect("initial host connection");
        fail_host_connection(
            &first_connection.child,
            &first_connection.pending,
            &first_connection.failed,
            &first_connection.failure,
            "simulated idle host failure",
        );

        let refresh = runtime.refresh_if_changed();
        assert!(refresh.changed);
        assert!(refresh.errors.is_empty(), "errors: {:?}", refresh.errors);
        let next_connection = runtime
            .inner
            .host
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .connection
            .as_ref()
            .map(Arc::clone)
            .expect("replacement host connection");
        assert!(!Arc::ptr_eq(&first_connection, &next_connection));
    }

    #[test]
    fn oversized_local_request_keeps_the_host_and_catalog_healthy() {
        if !bun_is_available() {
            return;
        }
        let temp = TempDir::new().expect("temp dir");
        let config_path = temp.path().join("config.txt");
        fs::write(&config_path, "").expect("write config");
        let plugins = temp.path().join("plugins");
        write_plugin(
            &plugins,
            "large-input",
            "Large Input",
            r#"
export default definePlugin({
  commands: [{
    id: "run",
    title: "Large Input: Run",
    inputs: Array.from({ length: 16 }, (_, index) => ({
      id: `input-${index}`,
      type: "text",
      label: `Input ${index}`,
      maxLength: 16384,
    })),
    run() { return { type: "toast", level: "info", message: "still healthy" }; },
  }],
});
"#,
        );

        let runtime = PluginRuntime::new(Some(&config_path));
        let refresh = runtime.refresh_if_changed();
        assert!(refresh.errors.is_empty(), "errors: {:?}", refresh.errors);
        let revision = runtime
            .command_with_revision("large-input", "run")
            .expect("large-input command")
            .1;
        let connection = runtime
            .inner
            .host
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .connection
            .as_ref()
            .map(Arc::clone)
            .expect("host connection");
        let large_value = Value::String("😀".repeat(16_384));
        let inputs = (0..16)
            .map(|index| (format!("input-{index}"), large_value.clone()))
            .collect();
        let context = || PluginContext {
            working_directory: None,
            active_command: None,
            platform: std::env::consts::OS.to_string(),
            app_version: "test".to_string(),
        };

        let error = runtime
            .invoke("large-input", "run", &revision, inputs, context())
            .expect_err("oversized request must be rejected locally");
        assert!(error.contains("1 MiB protocol limit"));
        assert!(!connection.is_failed());
        assert!(
            runtime
                .inner
                .catalog
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .fingerprint
                .is_some()
        );
        let actions = runtime
            .invoke("large-input", "run", &revision, BTreeMap::new(), context())
            .expect("host remains usable after local request rejection");
        assert_eq!(
            actions,
            vec![PluginAction::Toast {
                level: PluginToastLevel::Info,
                message: "still healthy".to_string(),
            }]
        );
    }

    #[test]
    fn runtime_loads_and_invokes_plain_typescript_when_bun_is_available() {
        if !bun_is_available() {
            return;
        }
        let temp = TempDir::new().expect("temp dir");
        let config_path = temp.path().join("config.txt");
        fs::write(&config_path, "").expect("write config");
        let plugins = temp.path().join("plugins");
        let plugin_dir = write_plugin(
            &plugins,
            "hello",
            "Hello",
            r#"
import { greeting } from "./helper.ts";

let invocationCount = 0;

export default definePlugin({
  commands: [
    {
      id: "fail",
      title: "Hello: Fail",
      run() {
        process.stdout.write("plugin stdout must not corrupt the protocol\n");
        throw new Error("expected plugin failure");
      },
    },
    {
      id: "invalid",
      title: "Hello: Invalid action",
      run() {
        return { type: "toats", message: "typo" };
      },
    },
    {
      id: "noisy",
      title: "Hello: Noisy output",
      async run() {
        await Bun.stdout.write("direct Bun stdout must not corrupt the protocol\n");
        const { writeSync } = await import("node:fs");
        writeSync(1, "direct fd 1 must not corrupt the protocol\n");
        return { type: "toast", level: "info", message: "Still connected" };
      },
    },
    {
      id: "greet",
      title: "Hello: Greet",
      inputs: [{ id: "name", type: "text", label: "Name", required: true }],
      async run({ inputs }) {
        invocationCount += 1;
        return { type: "toast", level: "success", message: `${invocationCount}: ${greeting(inputs.name)}` };
      },
    },
  ],
});
"#,
        );
        fs::write(
            plugin_dir.join("helper.ts"),
            r#"export const greeting = (name: unknown) => `Hello ${String(name)}`;"#,
        )
        .expect("write imported helper");
        let cache_key = discover_plugins(&plugins)
            .expect("discover plugin before loading")
            .sources
            .into_iter()
            .find(|source| source.id == "hello")
            .expect("hello source")
            .cache_key;

        let runtime = PluginRuntime::new(Some(&config_path));
        let refresh = runtime.refresh_if_changed();
        assert!(refresh.changed, "refresh errors: {:?}", refresh.errors);
        assert!(
            refresh.errors.is_empty(),
            "refresh errors: {:?}",
            refresh.errors
        );
        assert!(
            plugins
                .join(".termy-cache/bundles/hello")
                .join(format!("{cache_key}.mjs"))
                .is_file(),
            "plugin bundle should be cached by content hash"
        );
        assert_eq!(runtime.commands().len(), 4);
        let revision = runtime
            .command_with_revision("hello", "greet")
            .expect("hello command revision")
            .1;
        let context = PluginContext {
            working_directory: None,
            active_command: None,
            platform: std::env::consts::OS.to_string(),
            app_version: "test".to_string(),
        };
        let error = runtime
            .invoke("hello", "fail", &revision, BTreeMap::new(), context.clone())
            .expect_err("plugin failure should be reported");
        assert!(error.contains("expected plugin failure"));
        let actions = runtime
            .invoke(
                "hello",
                "noisy",
                &revision,
                BTreeMap::new(),
                context.clone(),
            )
            .expect("direct stdout must stay outside the protocol");
        assert_eq!(
            actions,
            vec![PluginAction::Toast {
                level: PluginToastLevel::Info,
                message: "Still connected".to_string(),
            }]
        );
        let error = runtime
            .invoke(
                "hello",
                "invalid",
                &revision,
                BTreeMap::new(),
                context.clone(),
            )
            .expect_err("invalid plugin action should be reported");
        assert!(error.contains("invalid result"));
        let error = runtime
            .invoke(
                "hello",
                "greet",
                &revision,
                BTreeMap::from([("name".to_string(), Value::Bool(true))]),
                context.clone(),
            )
            .expect_err("input types must be validated before invocation");
        assert!(error.contains("must be text"));
        let actions = runtime
            .invoke(
                "hello",
                "greet",
                &revision,
                BTreeMap::from([("name".to_string(), Value::String("Termy".to_string()))]),
                context,
            )
            .expect("invoke plugin");
        assert_eq!(
            actions,
            vec![PluginAction::Toast {
                level: PluginToastLevel::Success,
                message: "1: Hello Termy".to_string(),
            }]
        );

        write_plugin(
            &plugins,
            "unchanged-trigger",
            "Reload Trigger",
            r#"
export default definePlugin({
  commands: [],
});
"#,
        );
        let refresh = runtime.refresh_if_changed();
        assert!(refresh.changed, "refresh errors: {:?}", refresh.errors);
        assert!(
            refresh.errors.is_empty(),
            "refresh errors: {:?}",
            refresh.errors
        );

        let actions = runtime
            .invoke(
                "hello",
                "greet",
                &revision,
                BTreeMap::from([("name".to_string(), Value::String("Again".to_string()))]),
                PluginContext {
                    working_directory: None,
                    active_command: None,
                    platform: std::env::consts::OS.to_string(),
                    app_version: "test".to_string(),
                },
            )
            .expect("invoke unchanged plugin after catalog reload");
        assert_eq!(
            actions,
            vec![PluginAction::Toast {
                level: PluginToastLevel::Success,
                message: "2: Hello Again".to_string(),
            }]
        );

        fs::write(
            plugin_dir.join("helper.ts"),
            r#"export const greeting = (name: unknown) => `Hi ${String(name)}`;"#,
        )
        .expect("change imported helper");
        let refresh = runtime.refresh_if_changed();
        assert!(refresh.changed, "refresh errors: {:?}", refresh.errors);
        assert!(refresh.errors.is_empty());
        let error = runtime
            .invoke(
                "hello",
                "greet",
                &revision,
                BTreeMap::from([("name".to_string(), Value::String("Old".to_string()))]),
                PluginContext {
                    working_directory: None,
                    active_command: None,
                    platform: std::env::consts::OS.to_string(),
                    app_version: "test".to_string(),
                },
            )
            .expect_err("stale command schema revision must be rejected");
        assert!(error.contains("Plugin changed"));
        let new_revision = runtime
            .command_with_revision("hello", "greet")
            .expect("reloaded command revision")
            .1;
        assert_ne!(revision, new_revision);
    }

    #[test]
    fn malformed_plugins_are_isolated_and_empty_refresh_clears_the_catalog() {
        if !bun_is_available() {
            return;
        }
        let temp = TempDir::new().expect("temp dir");
        let config_path = temp.path().join("config.txt");
        fs::write(&config_path, "").expect("write config");
        let plugins = temp.path().join("plugins");
        let valid_dir = write_plugin(
            &plugins,
            "valid",
            "Valid",
            r#"
export default definePlugin({
  commands: [{ id: "run", title: "Valid: Run", run() {} }],
});
"#,
        );
        let broken_dir = write_plugin(
            &plugins,
            "broken",
            "Broken",
            r#"
export default definePlugin({
  commands: [{
    id: "run",
    title: "Broken: Run",
    enabled: false,
    disabledReason: 42,
    run() {},
  }],
});
"#,
        );

        let runtime = PluginRuntime::new(Some(&config_path));
        let refresh = runtime.refresh_if_changed();
        assert!(refresh.changed);
        assert_eq!(refresh.errors.len(), 1, "errors: {:?}", refresh.errors);
        assert!(refresh.errors[0].contains("broken"));
        assert_eq!(runtime.commands().len(), 1);
        assert_eq!(runtime.commands()[0].plugin_id, "valid");

        fs::remove_dir_all(valid_dir).expect("remove valid plugin");
        fs::remove_dir_all(broken_dir).expect("remove broken plugin");
        let refresh = runtime.refresh_if_changed();
        assert!(refresh.changed, "clearing a non-empty catalog is a change");
        assert!(refresh.errors.is_empty());
        assert!(runtime.commands().is_empty());
        assert!(!plugins.join(".termy-cache/bundles").exists());
    }

    #[test]
    fn plugin_load_rejects_imports_outside_the_plugin_directory() {
        if !bun_is_available() {
            return;
        }
        let temp = TempDir::new().expect("temp dir");
        let config_path = temp.path().join("config.txt");
        fs::write(&config_path, "").expect("write config");
        let plugins = temp.path().join("plugins");
        fs::write(
            temp.path().join("outside.ts"),
            "export const outside = true;",
        )
        .expect("write outside module");
        write_plugin(
            &plugins,
            "escape",
            "Escape",
            r#"
import { outside } from "../../outside.ts";
export default definePlugin({
  commands: [{ id: "run", title: "Escape: Run", run() { return outside; } }],
});
"#,
        );

        let runtime = PluginRuntime::new(Some(&config_path));
        let refresh = runtime.refresh_if_changed();
        assert_eq!(refresh.errors.len(), 1, "errors: {:?}", refresh.errors);
        assert!(refresh.errors[0].contains("escapes its directory"));
        assert!(runtime.commands().is_empty());
    }

    #[test]
    fn plugin_invocations_run_concurrently_across_workers() {
        if !bun_is_available() {
            return;
        }
        let temp = TempDir::new().expect("temp dir");
        let config_path = temp.path().join("config.txt");
        fs::write(&config_path, "").expect("write config");
        let plugins = temp.path().join("plugins");
        write_plugin(
            &plugins,
            "slow",
            "Slow",
            r#"
export default definePlugin({
  commands: [{
    id: "run",
    title: "Slow: Run",
    timeoutMs: 2000,
    async run() {
      await Bun.sleep(500);
      return { type: "toast", level: "info", message: "slow" };
    },
  }],
});
"#,
        );
        write_plugin(
            &plugins,
            "fast",
            "Fast",
            r#"
export default definePlugin({
  commands: [{
    id: "run",
    title: "Fast: Run",
    run() { return { type: "toast", level: "info", message: "fast" }; },
  }],
});
"#,
        );

        let runtime = PluginRuntime::new(Some(&config_path));
        let refresh = runtime.refresh_if_changed();
        assert!(refresh.errors.is_empty(), "errors: {:?}", refresh.errors);
        let slow_revision = runtime
            .command_with_revision("slow", "run")
            .expect("slow command")
            .1;
        let fast_revision = runtime
            .command_with_revision("fast", "run")
            .expect("fast command")
            .1;
        let context = || PluginContext {
            working_directory: None,
            active_command: None,
            platform: std::env::consts::OS.to_string(),
            app_version: "test".to_string(),
        };
        let slow_runtime = runtime.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let slow = thread::spawn(move || {
            started_tx.send(()).expect("signal slow invocation");
            slow_runtime.invoke(
                "slow",
                "run",
                &slow_revision,
                BTreeMap::new(),
                PluginContext {
                    working_directory: None,
                    active_command: None,
                    platform: std::env::consts::OS.to_string(),
                    app_version: "test".to_string(),
                },
            )
        });
        started_rx.recv().expect("wait for slow invocation");
        thread::sleep(Duration::from_millis(75));
        let started = Instant::now();
        let fast_actions = runtime
            .invoke("fast", "run", &fast_revision, BTreeMap::new(), context())
            .expect("fast plugin invocation");
        assert!(
            started.elapsed() < Duration::from_millis(300),
            "fast plugin waited behind slow plugin for {:?}",
            started.elapsed()
        );
        assert_eq!(
            fast_actions,
            vec![PluginAction::Toast {
                level: PluginToastLevel::Info,
                message: "fast".to_string(),
            }]
        );
        slow.join()
            .expect("join slow invocation")
            .expect("slow invocation succeeds");
    }

    #[test]
    fn queued_plugin_invocation_starts_its_timeout_when_execution_begins() {
        if !bun_is_available() {
            return;
        }
        let temp = TempDir::new().expect("temp dir");
        let config_path = temp.path().join("config.txt");
        fs::write(&config_path, "").expect("write config");
        let plugins = temp.path().join("plugins");
        let marker = temp.path().join("slow-started");
        let marker_json =
            serde_json::to_string(marker.to_string_lossy().as_ref()).expect("encode marker path");
        let source = r#"
const markerPath = __MARKER__;
export default definePlugin({
  commands: [
    {
      id: "slow",
      title: "Queued: Slow",
      timeoutMs: 1000,
      async run() {
        await Bun.write(markerPath, "started");
        await Bun.sleep(350);
        return { type: "toast", level: "info", message: "slow" };
      },
    },
    {
      id: "quick",
      title: "Queued: Quick",
      timeoutMs: 100,
      async run() {
        await Bun.sleep(25);
        return { type: "toast", level: "info", message: "quick" };
      },
    },
  ],
});
"#
        .replace("__MARKER__", &marker_json);
        write_plugin(&plugins, "queued", "Queued", &source);

        let runtime = PluginRuntime::new(Some(&config_path));
        let refresh = runtime.refresh_if_changed();
        assert!(refresh.errors.is_empty(), "errors: {:?}", refresh.errors);
        let revision = runtime
            .command_with_revision("queued", "slow")
            .expect("queued command")
            .1;
        let slow_runtime = runtime.clone();
        let slow_revision = revision.clone();
        let slow = thread::spawn(move || {
            slow_runtime.invoke(
                "queued",
                "slow",
                &slow_revision,
                BTreeMap::new(),
                PluginContext {
                    working_directory: None,
                    active_command: None,
                    platform: std::env::consts::OS.to_string(),
                    app_version: "test".to_string(),
                },
            )
        });
        let marker_deadline = Instant::now() + Duration::from_secs(2);
        while !marker.exists() {
            assert!(
                Instant::now() < marker_deadline,
                "slow plugin did not start in time"
            );
            thread::sleep(Duration::from_millis(10));
        }

        let started = Instant::now();
        let quick = runtime
            .invoke(
                "queued",
                "quick",
                &revision,
                BTreeMap::new(),
                PluginContext {
                    working_directory: None,
                    active_command: None,
                    platform: std::env::consts::OS.to_string(),
                    app_version: "test".to_string(),
                },
            )
            .expect("queued quick invocation should receive its full execution timeout");
        assert!(
            started.elapsed() >= Duration::from_millis(200),
            "same-plugin invocation did not wait for the active command"
        );
        assert_eq!(
            quick,
            vec![PluginAction::Toast {
                level: PluginToastLevel::Info,
                message: "quick".to_string(),
            }]
        );
        slow.join()
            .expect("join slow invocation")
            .expect("slow invocation succeeds");
    }

    #[test]
    fn plugin_load_removes_abandoned_build_artifacts() {
        if !bun_is_available() {
            return;
        }
        let temp = TempDir::new().expect("temp dir");
        let config_path = temp.path().join("config.txt");
        fs::write(&config_path, "").expect("write config");
        let plugins = temp.path().join("plugins");
        write_plugin(
            &plugins,
            "clean",
            "Clean",
            "export default definePlugin({ commands: [] });",
        );
        let bundle_dir = plugins.join(".termy-cache/bundles/clean");
        let capture = bundle_dir.join(".capture-abandoned");
        let temporary = bundle_dir.join("abandoned.mjs.1.tmp");
        fs::create_dir_all(&capture).expect("create abandoned capture");
        fs::write(capture.join("plugin.ts"), "stale").expect("write abandoned capture");
        fs::write(&temporary, "stale").expect("write abandoned bundle");

        let runtime = PluginRuntime::new(Some(&config_path));
        let refresh = runtime.refresh_if_changed();
        assert!(refresh.errors.is_empty(), "errors: {:?}", refresh.errors);
        assert!(!capture.exists(), "abandoned capture should be removed");
        assert!(!temporary.exists(), "abandoned bundle should be removed");
    }

    #[test]
    fn failed_plugin_loads_are_retried_when_bun_is_available() {
        if !bun_is_available() {
            return;
        }
        let temp = TempDir::new().expect("temp dir");
        let config_path = temp.path().join("config.txt");
        fs::write(&config_path, "").expect("write config");
        let plugins = temp.path().join("plugins");
        let plugin_dir = write_plugin(
            &plugins,
            "broken",
            "Broken",
            "export default definePlugin({ commands: [] });",
        );
        fs::write(
            plugin_dir.join("plugin.json"),
            r#"{"apiVersion":99,"id":"broken","name":"Broken"}"#,
        )
        .expect("write broken manifest");

        let runtime = PluginRuntime::new(Some(&config_path));
        let first = runtime.refresh_if_changed();
        assert!(!first.errors.is_empty());
        let second = runtime.refresh_if_changed();
        assert!(
            !second.errors.is_empty(),
            "unchanged failed plugins must be retried"
        );
    }
}
