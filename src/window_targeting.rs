#[cfg(target_os = "macos")]
mod config;
#[cfg(target_os = "macos")]
mod rules;

#[allow(unused_imports)]
pub(crate) use config::{
    ActivationPolicy, CompiledRule, ConfigError, RuleMatcher, ValidatedUserConfig,
    WindowTargetAction, WindowTargetingMode,
};
use hbb_common::log;
use lazy_static::lazy_static;
#[allow(unused_imports)]
pub(crate) use rules::{DecisionStep, EffectiveConfig, WindowCandidate, WindowDecision};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

#[derive(Clone, Debug, Eq, PartialEq, serde_derive::Serialize)]
pub struct WindowTargetingStatus {
    pub mode: WindowTargetingMode,
    pub generation: u64,
    pub rule_count: usize,
    pub hash: String,
    pub diagnostics: bool,
    pub source: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde_derive::Serialize)]
pub struct WindowTargetingValidation {
    pub mode: WindowTargetingMode,
    pub rule_count: usize,
    pub hash: String,
    pub diagnostics: bool,
}

#[derive(Clone)]
pub(crate) struct ActiveGeneration {
    pub(crate) generation: u64,
    pub(crate) hash: String,
    pub(crate) source: String,
    pub(crate) effective: EffectiveConfig,
}

#[cfg(test)]
#[allow(dead_code)]
impl ActiveGeneration {
    pub(crate) fn for_test(mode: WindowTargetingMode) -> Self {
        let user = ValidatedUserConfig {
            mode,
            diagnostics: false,
            rules: Vec::new(),
        };
        build_generation(1, Some(user), "test")
    }

    pub(crate) fn builtin_for_test() -> Self {
        build_generation(1, None, "test")
    }
}

struct RuntimeState {
    active: RwLock<Arc<ActiveGeneration>>,
}

impl RuntimeState {
    fn new_builtin() -> Self {
        Self {
            active: RwLock::new(Arc::new(build_generation(1, None, "builtin"))),
        }
    }

    fn snapshot(&self) -> Arc<ActiveGeneration> {
        self.active.read().unwrap().clone()
    }

    fn status(&self) -> WindowTargetingStatus {
        status_from_generation(&self.snapshot())
    }

    fn load_user_config(path: &Path) -> Result<ValidatedUserConfig, ConfigError> {
        let text = fs::read_to_string(path)
            .map_err(|error| ConfigError::new(format!("read {}: {error}", path.display())))?;
        config::parse_user_config(&text)
    }

    fn initialize_from_path(&self, path: &Path) -> Result<WindowTargetingStatus, ConfigError> {
        let replacement = Arc::new(build_generation(
            1,
            Some(Self::load_user_config(path)?),
            "builtin+user",
        ));
        *self.active.write().unwrap() = Arc::clone(&replacement);
        Ok(status_from_generation(&replacement))
    }

    fn validate_path(&self, path: &Path) -> Result<WindowTargetingValidation, ConfigError> {
        let candidate = build_generation(0, Some(Self::load_user_config(path)?), "validation");
        Ok(validation_from_generation(&candidate))
    }

    fn reload_from_path(&self, path: &Path) -> Result<WindowTargetingStatus, ConfigError> {
        let candidate = build_generation(0, Some(Self::load_user_config(path)?), "builtin+user");
        let mut active = self.active.write().unwrap();
        let generation = active
            .generation
            .checked_add(1)
            .ok_or_else(|| ConfigError::new("window-targeting generation overflow".to_owned()))?;
        let replacement = Arc::new(ActiveGeneration {
            generation,
            ..candidate
        });
        *active = Arc::clone(&replacement);
        Ok(status_from_generation(&replacement))
    }
}

lazy_static! {
    static ref RUNTIME: RuntimeState = RuntimeState::new_builtin();
}

pub(crate) fn config_path() -> Result<PathBuf, ConfigError> {
    let home = crate::platform::get_active_user_home()
        .ok_or_else(|| ConfigError::new("active user home is unavailable".to_owned()))?;
    Ok(home
        .join("Library")
        .join("Application Support")
        .join("RustDesk-Herbin")
        .join("window-targeting.toml"))
}

fn ensure_template_at(path: &Path) -> Result<(), ConfigError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| {
            ConfigError::new(format!(
                "create window-targeting directory {}: {error}",
                parent.display()
            ))
        })?;
    }

    let mut file = match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(()),
        Err(error) => {
            return Err(ConfigError::new(format!(
                "create window-targeting template {}: {error}",
                path.display()
            )))
        }
    };
    file.write_all(config::DEFAULT_TEMPLATE.as_bytes())
        .map_err(|error| {
            ConfigError::new(format!(
                "write window-targeting template {}: {error}",
                path.display()
            ))
        })
}

pub(crate) fn initialize() {
    match config_path() {
        Ok(path) => match ensure_template_at(&path) {
            Ok(()) => {
                if let Err(error) = RUNTIME.initialize_from_path(&path) {
                    log::error!(
                        "Failed to initialize macOS window targeting from {}: {error}",
                        path.display()
                    );
                }
            }
            Err(error) => {
                log::error!("Failed to create macOS window-targeting template: {error}");
            }
        },
        Err(error) => {
            log::error!("Failed to resolve macOS window-targeting config: {error}");
        }
    }

    let status = status();
    log::info!(
        "macOS window targeting: mode={:?} generation={} rules={} hash={} diagnostics={} source={}",
        status.mode,
        status.generation,
        status.rule_count,
        status.hash,
        status.diagnostics,
        status.source
    );
}

#[allow(dead_code)]
pub(crate) fn snapshot() -> Arc<ActiveGeneration> {
    RUNTIME.snapshot()
}

pub(crate) fn status() -> WindowTargetingStatus {
    RUNTIME.status()
}

#[allow(dead_code)]
pub(crate) fn validate_from_disk() -> Result<WindowTargetingValidation, ConfigError> {
    RUNTIME.validate_path(&config_path()?)
}

#[allow(dead_code)]
pub(crate) fn reload_from_disk() -> Result<WindowTargetingStatus, ConfigError> {
    RUNTIME.reload_from_path(&config_path()?)
}

fn build_generation(
    generation: u64,
    user: Option<ValidatedUserConfig>,
    source: &str,
) -> ActiveGeneration {
    let effective = rules::compile_effective(user);
    let hash = EffectiveHashEncoder::hash(&effective);
    ActiveGeneration {
        generation,
        hash,
        source: source.to_owned(),
        effective,
    }
}

fn status_from_generation(generation: &ActiveGeneration) -> WindowTargetingStatus {
    WindowTargetingStatus {
        mode: generation.effective.mode,
        generation: generation.generation,
        rule_count: generation.effective.rules.len(),
        hash: generation.hash.clone(),
        diagnostics: generation.effective.diagnostics,
        source: generation.source.clone(),
    }
}

fn validation_from_generation(generation: &ActiveGeneration) -> WindowTargetingValidation {
    WindowTargetingValidation {
        mode: generation.effective.mode,
        rule_count: generation.effective.rules.len(),
        hash: generation.hash.clone(),
        diagnostics: generation.effective.diagnostics,
    }
}

struct EffectiveHashEncoder {
    hasher: Sha256,
}

impl EffectiveHashEncoder {
    fn hash(effective: &EffectiveConfig) -> String {
        let mut encoder = Self {
            hasher: Sha256::new(),
        };
        encoder.string(0x01, mode_name(effective.mode));
        encoder.boolean(0x02, effective.diagnostics);
        encoder.array_len(0x03, effective.rules.len());
        for rule in &effective.rules {
            encoder.string(0x04, &rule.id);
            encoder.integer(0x05, rule.priority);
            encoder.string(0x06, action_name(rule.action));
            encoder.strings(0x07, 0x08, &rule.matcher.bundle_ids);
            encoder.strings(0x09, 0x0a, &rule.matcher.bundle_id_prefixes);
            encoder.strings(0x0b, 0x0c, &rule.matcher.process_names);
            encoder.integers(0x0d, 0x0e, &rule.matcher.layers);
            encoder.optional_integer(0x0f, 0x10, rule.matcher.layer_min);
            encoder.optional_integer(0x11, 0x12, rule.matcher.layer_max);
            encoder.strings(0x13, 0x14, &rule.matcher.ax_roles);
            encoder.strings(0x15, 0x16, &rule.matcher.ax_subroles);
            encoder.array_len(0x17, rule.matcher.activation_policies.len());
            for policy in &rule.matcher.activation_policies {
                encoder.string(0x18, activation_policy_name(*policy));
            }
            encoder.optional_boolean(0x19, 0x1a, rule.matcher.covers_display);
        }
        hex::encode(encoder.hasher.finalize())
    }

    fn tag(&mut self, tag: u8) {
        self.hasher.update([tag]);
    }

    fn boolean(&mut self, tag: u8, value: bool) {
        self.tag(tag);
        self.hasher.update([u8::from(value)]);
    }

    fn integer(&mut self, tag: u8, value: i32) {
        self.tag(tag);
        self.hasher.update(value.to_le_bytes());
    }

    fn string(&mut self, tag: u8, value: &str) {
        self.tag(tag);
        self.hasher.update((value.len() as u64).to_le_bytes());
        self.hasher.update(value.as_bytes());
    }

    fn array_len(&mut self, tag: u8, len: usize) {
        self.tag(tag);
        self.hasher.update((len as u64).to_le_bytes());
    }

    fn strings(&mut self, array_tag: u8, element_tag: u8, values: &[String]) {
        self.array_len(array_tag, values.len());
        for value in values {
            self.string(element_tag, value);
        }
    }

    fn integers(&mut self, array_tag: u8, element_tag: u8, values: &[i32]) {
        self.array_len(array_tag, values.len());
        for value in values {
            self.integer(element_tag, *value);
        }
    }

    fn optional_integer(&mut self, option_tag: u8, value_tag: u8, value: Option<i32>) {
        self.tag(option_tag);
        self.hasher.update([u8::from(value.is_some())]);
        if let Some(value) = value {
            self.integer(value_tag, value);
        }
    }

    fn optional_boolean(&mut self, option_tag: u8, value_tag: u8, value: Option<bool>) {
        self.tag(option_tag);
        self.hasher.update([u8::from(value.is_some())]);
        if let Some(value) = value {
            self.boolean(value_tag, value);
        }
    }
}

fn mode_name(mode: WindowTargetingMode) -> &'static str {
    match mode {
        WindowTargetingMode::Rules => "rules",
        WindowTargetingMode::Passthrough => "passthrough",
    }
}

fn action_name(action: WindowTargetAction) -> &'static str {
    match action {
        WindowTargetAction::Skip => "skip",
        WindowTargetAction::ForwardOnly => "forward_only",
        WindowTargetAction::Activate => "activate",
    }
}

fn activation_policy_name(policy: ActivationPolicy) -> &'static str {
    match policy {
        ActivationPolicy::Regular => "regular",
        ActivationPolicy::Accessory => "accessory",
        ActivationPolicy::Prohibited => "prohibited",
        ActivationPolicy::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn test_path(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "rdh-window-targeting-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root.join("window-targeting.toml")
    }

    #[test]
    fn successful_reload_changes_generation_and_hash() {
        let path = test_path("success");
        fs::write(
            &path,
            "version = 1\nmode = \"passthrough\"\ndiagnostics = false\n",
        )
        .unwrap();
        let state = RuntimeState::new_builtin();
        let before = state.status();
        let after = state.reload_from_path(&path).unwrap();
        assert_eq!(after.generation, before.generation + 1);
        assert_eq!(after.mode, WindowTargetingMode::Passthrough);
        assert_ne!(after.hash, before.hash);
    }

    #[test]
    fn invalid_reload_preserves_active_generation() {
        let path = test_path("invalid");
        fs::write(&path, "version = 1\nmode = \"broken\"\n").unwrap();
        let state = RuntimeState::new_builtin();
        let before = state.status();
        assert!(state.reload_from_path(&path).is_err());
        assert_eq!(state.status(), before);
    }

    #[test]
    fn initial_load_installs_generation_one() {
        let path = test_path("initial");
        fs::write(
            &path,
            "version = 1\nmode = \"passthrough\"\ndiagnostics = false\n",
        )
        .unwrap();
        let state = RuntimeState::new_builtin();
        let loaded = state.initialize_from_path(&path).unwrap();
        assert_eq!(loaded.generation, 1);
        assert_eq!(loaded.mode, WindowTargetingMode::Passthrough);
    }

    #[test]
    fn validation_never_mutates_state() {
        let path = test_path("validate");
        fs::write(
            &path,
            "version = 1\nmode = \"passthrough\"\ndiagnostics = false\n",
        )
        .unwrap();
        let state = RuntimeState::new_builtin();
        let before = state.status();
        state.validate_path(&path).unwrap();
        assert_eq!(state.status(), before);
    }

    #[test]
    fn validation_hash_matches_the_generation_reload_would_install() {
        let path = test_path("validation-hash");
        fs::write(
            &path,
            "version = 1\nmode = \"rules\"\ndiagnostics = false\n",
        )
        .unwrap();
        let state = RuntimeState::new_builtin();
        let validated = state.validate_path(&path).unwrap();
        let reloaded = state.reload_from_path(&path).unwrap();
        assert_eq!(validated.hash, reloaded.hash);
        assert_eq!(validated.rule_count, reloaded.rule_count);
    }

    #[test]
    fn effective_hash_excludes_generation_and_source() {
        let left = build_generation(1, None, "builtin");
        let right = build_generation(99, None, "different-source");
        assert_eq!(left.hash, right.hash);
    }

    #[test]
    fn effective_hash_includes_diagnostics_state() {
        let left = build_generation(1, None, "builtin");
        let user = ValidatedUserConfig {
            mode: WindowTargetingMode::Rules,
            diagnostics: true,
            rules: Vec::new(),
        };
        let right = build_generation(1, Some(user), "user");
        assert_ne!(left.hash, right.hash);
    }

    #[test]
    fn template_creation_never_overwrites_existing_file() {
        let path = test_path("template");
        fs::write(&path, "sentinel").unwrap();
        ensure_template_at(&path).unwrap();
        assert_eq!(fs::read_to_string(path).unwrap(), "sentinel");
    }
}
