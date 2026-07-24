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
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
};

const COLLECTOR_ERROR_KEY: &str = "collector";
const EXECUTOR_ERROR_KEY: &str = "executor";
const ERROR_RATE_LIMIT: Duration = Duration::from_secs(60);

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

#[derive(Clone, Debug, Eq, PartialEq, serde_derive::Deserialize, serde_derive::Serialize)]
pub enum WindowTargetingRequest {
    Status,
    Reload,
}

#[derive(Clone, Debug, Eq, PartialEq, serde_derive::Deserialize, serde_derive::Serialize)]
pub struct WindowTargetingResponse {
    pub ok: bool,
    pub lines: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CliOperation {
    Status,
    Validate,
    Reload,
}

fn parse_cli_operation(args: &[String]) -> Result<CliOperation, String> {
    match args {
        [operation] if operation == "status" => Ok(CliOperation::Status),
        [operation] if operation == "validate" => Ok(CliOperation::Validate),
        [operation] if operation == "reload" => Ok(CliOperation::Reload),
        _ => Err("usage: --window-targeting status|validate|reload".to_owned()),
    }
}

#[derive(Clone, Debug)]
pub struct PreprocessOutcome {
    pub mode: WindowTargetingMode,
    pub generation: u64,
    pub hash: String,
    pub action: WindowTargetAction,
    pub rule_id: String,
    pub candidate: Option<WindowCandidate>,
    pub activation: Option<crate::platform::macos::MacWindowActivationOutcome>,
    pub error: Option<String>,
    pub elapsed_micros: u128,
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
    static ref ERROR_LOG_TIMES: Mutex<HashMap<&'static str, Instant>> =
        Mutex::new(HashMap::with_capacity(2));
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

pub(crate) fn handle_ipc_request(request: WindowTargetingRequest) -> WindowTargetingResponse {
    handle_ipc_request_with(request, status, reload_from_disk)
}

fn handle_ipc_request_with<S, R>(
    request: WindowTargetingRequest,
    current_status: S,
    reload: R,
) -> WindowTargetingResponse
where
    S: Fn() -> WindowTargetingStatus,
    R: FnOnce() -> Result<WindowTargetingStatus, ConfigError>,
{
    match request {
        WindowTargetingRequest::Status => WindowTargetingResponse {
            ok: true,
            lines: vec![format_status_line("OK", &current_status(), false)],
        },
        WindowTargetingRequest::Reload => match reload() {
            Ok(status) => WindowTargetingResponse {
                ok: true,
                lines: vec![format_status_line("OK", &status, false)],
            },
            Err(error) => WindowTargetingResponse {
                ok: false,
                lines: vec![
                    format_reload_error(&error.to_string()),
                    format_status_line("ACTIVE", &current_status(), true),
                ],
            },
        },
    }
}

fn format_status_line(prefix: &str, status: &WindowTargetingStatus, unchanged: bool) -> String {
    let unchanged = if unchanged { " unchanged=true" } else { "" };
    format!(
        "{prefix} mode={} rules={} generation={} hash={} diagnostics={} source={}{}",
        mode_name(status.mode),
        status.rule_count,
        status.generation,
        status.hash,
        status.diagnostics,
        status.source,
        unchanged
    )
}

fn format_reload_error(error: &str) -> String {
    let mut escaped = String::with_capacity(error.len());
    for character in error.chars() {
        match character {
            '\r' | '\n' => escaped.push(' '),
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            _ => escaped.push(character),
        }
    }
    format!("ERROR reason=\"{escaped}\"")
}

fn format_validation_line(validation: &WindowTargetingValidation) -> String {
    format!(
        "OK mode={} rules={} hash={} diagnostics={}",
        mode_name(validation.mode),
        validation.rule_count,
        validation.hash,
        validation.diagnostics
    )
}

pub(crate) fn run_cli(args: &[String]) -> i32 {
    run_cli_with(
        args,
        validate_from_disk,
        crate::ipc::request_window_targeting,
    )
}

fn run_cli_with<V, I>(args: &[String], validate: V, send_request: I) -> i32
where
    V: FnOnce() -> Result<WindowTargetingValidation, ConfigError>,
    I: FnOnce(WindowTargetingRequest) -> hbb_common::ResultType<WindowTargetingResponse>,
{
    let operation = match parse_cli_operation(args) {
        Ok(operation) => operation,
        Err(error) => {
            println!("{}", format_reload_error(&error));
            return 1;
        }
    };

    match operation {
        CliOperation::Validate => match validate() {
            Ok(validation) => {
                println!("{}", format_validation_line(&validation));
                0
            }
            Err(error) => {
                println!("{}", format_reload_error(&error.to_string()));
                1
            }
        },
        CliOperation::Status | CliOperation::Reload => {
            let request = if operation == CliOperation::Status {
                WindowTargetingRequest::Status
            } else {
                WindowTargetingRequest::Reload
            };
            match send_request(request) {
                Ok(response) => {
                    for line in response.lines {
                        println!("{line}");
                    }
                    i32::from(!response.ok)
                }
                Err(error) => {
                    println!("{}", format_reload_error(&error.to_string()));
                    1
                }
            }
        }
    }
}

fn preprocess_with<C, E>(
    active: &ActiveGeneration,
    x: i32,
    y: i32,
    collect: C,
    execute: E,
) -> PreprocessOutcome
where
    C: FnOnce(
        i32,
        i32,
    )
        -> Result<Vec<WindowCandidate>, crate::platform::macos::MacWindowCollectionError>,
    E: FnOnce(i32, i32, i32) -> crate::platform::macos::MacWindowActivationOutcome,
{
    let started = Instant::now();
    if active.effective.mode == WindowTargetingMode::Passthrough {
        let elapsed_micros = started.elapsed().as_micros();
        return PreprocessOutcome {
            mode: active.effective.mode,
            generation: active.generation,
            hash: active.hash.clone(),
            action: WindowTargetAction::ForwardOnly,
            rule_id: "mode.passthrough".to_owned(),
            candidate: None,
            activation: None,
            error: None,
            elapsed_micros,
        };
    }

    let candidates = match collect(x, y) {
        Ok(candidates) => candidates,
        Err(error) => {
            let error = format!("collect window candidates: {error}");
            let elapsed_micros = started.elapsed().as_micros();
            return PreprocessOutcome {
                mode: active.effective.mode,
                generation: active.generation,
                hash: active.hash.clone(),
                action: WindowTargetAction::ForwardOnly,
                rule_id: "error.collector".to_owned(),
                candidate: None,
                activation: None,
                error: Some(error),
                elapsed_micros,
            };
        }
    };

    let decision = rules::decide(&active.effective, &candidates);
    let candidate = decision
        .candidate_index
        .and_then(|index| candidates.into_iter().nth(index));
    let activation = match (decision.action, candidate.as_ref()) {
        (WindowTargetAction::Activate, Some(candidate)) => Some(execute(x, y, candidate.pid)),
        _ => None,
    };
    let error = match (candidate.as_ref(), activation.as_ref()) {
        (Some(candidate), Some(activation)) if activation.result < 0 => {
            Some(format!("activation failed for pid={}", candidate.pid))
        }
        _ => None,
    };
    let elapsed_micros = started.elapsed().as_micros();

    PreprocessOutcome {
        mode: active.effective.mode,
        generation: active.generation,
        hash: active.hash.clone(),
        action: decision.action,
        rule_id: decision.rule_id,
        candidate,
        activation,
        error,
        elapsed_micros,
    }
}

pub(crate) fn preprocess_remote_left_click(x: i32, y: i32) -> PreprocessOutcome {
    let active = snapshot();
    let outcome = preprocess_with(
        &active,
        x,
        y,
        crate::platform::macos::collect_window_candidates_at_point,
        crate::platform::macos::activate_window_candidate_at_point,
    );

    if let Some(error) = outcome.error.as_deref() {
        let key = if outcome.rule_id == "error.collector" {
            Some(COLLECTOR_ERROR_KEY)
        } else if outcome
            .activation
            .as_ref()
            .map_or(false, |activation| activation.result < 0)
        {
            Some(EXECUTOR_ERROR_KEY)
        } else {
            None
        };
        if let Some(key) = key {
            log_rate_limited_error(key, error);
        }
    }

    if active.effective.diagnostics {
        log::debug!("{}", format_diagnostic_line(&outcome));
    }

    outcome
}

fn format_diagnostic_line(outcome: &PreprocessOutcome) -> String {
    let (
        pid,
        bundle_id,
        process_name,
        layer,
        activation_policy,
        covers_display,
        ax_role,
        ax_subrole,
    ) = match outcome.candidate.as_ref() {
        Some(candidate) => (
            candidate.pid.to_string(),
            candidate.bundle_id.as_str(),
            candidate.process_name.as_str(),
            candidate.layer.to_string(),
            activation_policy_name(candidate.activation_policy),
            candidate.covers_display.to_string(),
            candidate.ax_role.as_deref().unwrap_or("-"),
            candidate.ax_subrole.as_deref().unwrap_or("-"),
        ),
        None => (
            "-".to_owned(),
            "-",
            "-",
            "-".to_owned(),
            "-",
            "-".to_owned(),
            "-",
            "-",
        ),
    };
    let (activation_result, application_activation_attempted, window_raise_attempted) =
        match outcome.activation.as_ref() {
            Some(activation) => (
                activation.result.to_string(),
                activation.application_activation_attempted.to_string(),
                activation.window_raise_attempted.to_string(),
            ),
            None => ("-".to_owned(), "-".to_owned(), "-".to_owned()),
        };

    format!(
        "macOS window targeting decision: mode={} generation={} hash={} pid={} bundle_id={} \
         process_name={} layer={} activation_policy={} covers_display={} ax_role={} ax_subrole={} \
         rule_id={} action={} activation_result={} application_activation_attempted={} \
         window_raise_attempted={} elapsed_micros={}",
        mode_name(outcome.mode),
        outcome.generation,
        outcome.hash,
        pid,
        bundle_id,
        process_name,
        layer,
        activation_policy,
        covers_display,
        ax_role,
        ax_subrole,
        outcome.rule_id,
        action_name(outcome.action),
        activation_result,
        application_activation_attempted,
        window_raise_attempted,
        outcome.elapsed_micros,
    )
}

fn record_error_emission(
    emissions: &mut HashMap<&'static str, Instant>,
    key: &'static str,
    now: Instant,
) -> bool {
    if key != COLLECTOR_ERROR_KEY && key != EXECUTOR_ERROR_KEY {
        return false;
    }
    if emissions.get(key).map_or(false, |previous| {
        now.duration_since(*previous) < ERROR_RATE_LIMIT
    }) {
        return false;
    }
    emissions.insert(key, now);
    true
}

fn log_rate_limited_error(key: &'static str, error: &str) {
    let should_emit =
        record_error_emission(&mut ERROR_LOG_TIMES.lock().unwrap(), key, Instant::now());
    if should_emit {
        log::error!("{}", format_error_log_line(key, error));
    }
}

fn format_error_log_line(key: &'static str, error: &str) -> String {
    format!("macOS window targeting {key} error: {error}")
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

    #[test]
    fn parses_window_targeting_cli_operations() {
        assert_eq!(
            parse_cli_operation(&["status".to_owned()]).unwrap(),
            CliOperation::Status
        );
        assert_eq!(
            parse_cli_operation(&["validate".to_owned()]).unwrap(),
            CliOperation::Validate
        );
        assert_eq!(
            parse_cli_operation(&["reload".to_owned()]).unwrap(),
            CliOperation::Reload
        );
        assert!(parse_cli_operation(&[]).is_err());
        assert!(parse_cli_operation(&["watch".to_owned()]).is_err());
    }

    #[test]
    fn status_ipc_request_does_not_reload_or_mutate_state() {
        let state = RuntimeState::new_builtin();
        let before = state.status();
        let response = handle_ipc_request_with(
            WindowTargetingRequest::Status,
            || state.status(),
            || panic!("status must not call reload"),
        );

        assert!(response.ok);
        assert_eq!(response.lines.len(), 1);
        assert_eq!(state.status(), before);
    }

    #[test]
    fn failed_ipc_reload_preserves_generation_and_hash() {
        let path = test_path("invalid-ipc-reload");
        fs::write(&path, "version = 1\nmode = \"broken\"\n").unwrap();
        let state = RuntimeState::new_builtin();
        let before = state.status();

        let response = handle_ipc_request_with(
            WindowTargetingRequest::Reload,
            || state.status(),
            || state.reload_from_path(&path),
        );
        let after = state.status();

        assert!(!response.ok);
        assert_eq!(response.lines.len(), 2);
        assert!(response.lines[0].starts_with("ERROR reason=\""));
        assert!(!response.lines[0].contains(['\r', '\n']));
        assert_eq!(after.generation, before.generation);
        assert_eq!(after.hash, before.hash);
        assert!(response.lines[1].contains(&format!(
            "generation={} hash={} ",
            before.generation, before.hash
        )));
        assert!(response.lines[1].ends_with(" unchanged=true"));
    }

    #[test]
    fn reload_errors_are_quoted_and_single_line() {
        assert_eq!(
            format_reload_error("bad\\path \"quoted\"\r\nnext"),
            "ERROR reason=\"bad\\\\path \\\"quoted\\\"  next\""
        );
    }

    #[test]
    fn validation_cli_is_local_and_formats_the_exact_ok_line() {
        let validation = WindowTargetingValidation {
            mode: WindowTargetingMode::Rules,
            rule_count: 3,
            hash: "a".repeat(64),
            diagnostics: false,
        };
        assert_eq!(
            format_validation_line(&validation),
            format!(
                "OK mode=rules rules=3 hash={} diagnostics=false",
                "a".repeat(64)
            )
        );

        let exit_code = run_cli_with(
            &["validate".to_owned()],
            || Ok(validation),
            |_| -> hbb_common::ResultType<WindowTargetingResponse> {
                panic!("validate must not use IPC")
            },
        );
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn cli_exit_code_tracks_ipc_response_and_rejects_invalid_operations() {
        let validate = || -> Result<WindowTargetingValidation, ConfigError> {
            panic!("status and reload must not validate locally")
        };
        let status_exit = run_cli_with(&["status".to_owned()], validate, |request| {
            assert_eq!(request, WindowTargetingRequest::Status);
            Ok(WindowTargetingResponse {
                ok: true,
                lines: vec!["OK status".to_owned()],
            })
        });
        assert_eq!(status_exit, 0);

        let reload_exit = run_cli_with(
            &["reload".to_owned()],
            || -> Result<WindowTargetingValidation, ConfigError> {
                panic!("reload must not validate locally")
            },
            |request| {
                assert_eq!(request, WindowTargetingRequest::Reload);
                Ok(WindowTargetingResponse {
                    ok: false,
                    lines: vec!["ERROR reload".to_owned()],
                })
            },
        );
        assert_eq!(reload_exit, 1);

        let validation_error_exit = run_cli_with(
            &["validate".to_owned()],
            || Err(ConfigError::new("invalid config".to_owned())),
            |_| -> hbb_common::ResultType<WindowTargetingResponse> {
                panic!("validate must not use IPC")
            },
        );
        assert_eq!(validation_error_exit, 1);

        let ipc_error_exit = run_cli_with(
            &["status".to_owned()],
            || -> Result<WindowTargetingValidation, ConfigError> {
                panic!("status must not validate locally")
            },
            |_| Err(hbb_common::anyhow::anyhow!("IPC unavailable")),
        );
        assert_eq!(ipc_error_exit, 1);

        let invalid_exit = run_cli_with(
            &["watch".to_owned()],
            || -> Result<WindowTargetingValidation, ConfigError> {
                panic!("invalid operation must not validate")
            },
            |_| -> hbb_common::ResultType<WindowTargetingResponse> {
                panic!("invalid operation must not use IPC")
            },
        );
        assert_eq!(invalid_exit, 1);
    }

    fn activation_outcome(result: i32) -> crate::platform::macos::MacWindowActivationOutcome {
        crate::platform::macos::MacWindowActivationOutcome {
            result,
            application_activation_attempted: result != 0,
            window_raise_attempted: false,
        }
    }

    fn candidate(
        pid: i32,
        bundle_id: &str,
        process_name: &str,
        layer: i32,
        policy: ActivationPolicy,
        ax_role: Option<&str>,
    ) -> WindowCandidate {
        WindowCandidate {
            pid,
            window_id: pid as u32,
            bundle_id: bundle_id.to_owned(),
            process_name: process_name.to_owned(),
            layer,
            alpha: 1.0,
            activation_policy: policy,
            covers_display: false,
            ax_role: ax_role.map(str::to_owned),
            ax_subrole: None,
        }
    }

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
    fn passthrough_calls_neither_collector_nor_executor() {
        let active = ActiveGeneration::for_test(WindowTargetingMode::Passthrough);
        let collected = std::cell::Cell::new(false);
        let executed = std::cell::Cell::new(false);
        let outcome = preprocess_with(
            &active,
            10,
            20,
            |_, _| {
                collected.set(true);
                Ok(Vec::new())
            },
            |_, _, _| {
                executed.set(true);
                activation_outcome(0)
            },
        );
        assert!(!collected.get());
        assert!(!executed.get());
        assert_eq!(outcome.mode, WindowTargetingMode::Passthrough);
    }

    #[test]
    fn forward_only_never_calls_executor() {
        let active = ActiveGeneration::builtin_for_test();
        let executed = std::cell::Cell::new(false);
        let outcome = preprocess_with(
            &active,
            1215,
            978,
            |_, _| {
                Ok(vec![candidate(
                    64334,
                    "com.apple.dock.helper",
                    "DockHelper",
                    101,
                    ActivationPolicy::Accessory,
                    Some("AXMenuItem"),
                )])
            },
            |_, _, _| {
                executed.set(true);
                activation_outcome(0)
            },
        );
        assert_eq!(outcome.action, WindowTargetAction::ForwardOnly);
        assert!(!executed.get());
    }

    #[test]
    fn activate_calls_executor_with_selected_pid_once() {
        let active = ActiveGeneration::builtin_for_test();
        let collections = std::cell::Cell::new(0);
        let calls = std::cell::RefCell::new(Vec::new());
        let outcome = preprocess_with(
            &active,
            500,
            500,
            |_, _| {
                collections.set(collections.get() + 1);
                Ok(vec![candidate(
                    80988,
                    "com.openai.chat",
                    "ChatGPT",
                    0,
                    ActivationPolicy::Regular,
                    Some("AXWindow"),
                )])
            },
            |x, y, pid| {
                calls.borrow_mut().push((x, y, pid));
                activation_outcome(pid)
            },
        );
        assert_eq!(collections.get(), 1);
        assert_eq!(calls.borrow().as_slice(), &[(500, 500, 80988)]);
        assert_eq!(outcome.action, WindowTargetAction::Activate);
    }

    #[test]
    fn collector_failure_forwards_raw_click_without_executor() {
        let active = ActiveGeneration::builtin_for_test();
        let executed = std::cell::Cell::new(false);
        let outcome = preprocess_with(
            &active,
            500,
            500,
            |_, _| Err(crate::platform::macos::MacWindowCollectionError),
            |_, _, _| {
                executed.set(true);
                activation_outcome(0)
            },
        );
        assert_eq!(outcome.action, WindowTargetAction::ForwardOnly);
        assert_eq!(outcome.rule_id, "error.collector");
        assert!(outcome.error.is_some());
        assert!(!executed.get());
    }

    #[test]
    fn collector_failure_payloads_do_not_expose_click_coordinates() {
        const X: i32 = 987_654_321;
        const Y: i32 = 123_456_789;
        let active = ActiveGeneration::builtin_for_test();
        let outcome = preprocess_with(
            &active,
            X,
            Y,
            |_, _| Err(crate::platform::macos::MacWindowCollectionError),
            |_, _, _| panic!("collector failure must not invoke the executor"),
        );
        assert_eq!(outcome.action, WindowTargetAction::ForwardOnly);
        assert_eq!(outcome.rule_id, "error.collector");

        let error = outcome.error.as_deref().unwrap();
        let diagnostic = format_diagnostic_line(&outcome);
        let error_log = format_error_log_line(COLLECTOR_ERROR_KEY, error);
        for payload in [error, diagnostic.as_str(), error_log.as_str()] {
            for coordinate_value in [X.to_string(), Y.to_string()] {
                assert!(
                    !payload.contains(&coordinate_value),
                    "coordinate value leaked in payload: {payload}"
                );
            }
            for token in payload.split_whitespace() {
                for separator in ['=', ':'] {
                    if let Some((label, _)) = token.split_once(separator) {
                        let label = label
                            .trim_matches(|character: char| !character.is_ascii_alphanumeric())
                            .to_ascii_lowercase();
                        assert!(
                            !matches!(
                                label.as_str(),
                                "x" | "y"
                                    | "point"
                                    | "coordinate"
                                    | "coordinates"
                                    | "cursorposition"
                            ),
                            "coordinate field label leaked in payload: {payload}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn diagnostics_use_only_allowlisted_candidate_fields() {
        let active = ActiveGeneration::builtin_for_test();
        let outcome = preprocess_with(
            &active,
            500,
            500,
            |_, _| {
                Ok(vec![candidate(
                    80988,
                    "com.openai.chat",
                    "ChatGPT",
                    0,
                    ActivationPolicy::Regular,
                    Some("AXWindow"),
                )])
            },
            |_, _, pid| activation_outcome(pid),
        );
        let line = format_diagnostic_line(&outcome);
        assert!(line.contains("bundle_id=com.openai.chat"));
        assert!(line.contains("pid=80988"));
        assert!(line.contains("rule_id="));
        assert!(!line.contains("title="));
        assert!(!line.contains("peer="));
    }

    #[test]
    fn failed_activation_retains_executor_error_without_retry() {
        let active = ActiveGeneration::builtin_for_test();
        let calls = std::cell::Cell::new(0);
        let outcome = preprocess_with(
            &active,
            500,
            500,
            |_, _| {
                Ok(vec![candidate(
                    80988,
                    "com.openai.chat",
                    "ChatGPT",
                    0,
                    ActivationPolicy::Regular,
                    Some("AXWindow"),
                )])
            },
            |_, _, _| {
                calls.set(calls.get() + 1);
                activation_outcome(-80988)
            },
        );
        assert_eq!(calls.get(), 1);
        assert_eq!(
            outcome.error.as_deref(),
            Some("activation failed for pid=80988")
        );
        assert_eq!(outcome.activation, Some(activation_outcome(-80988)));
    }

    #[test]
    fn error_rate_limit_has_only_fixed_keys_and_reopens_after_sixty_seconds() {
        let start = std::time::Instant::now();
        let mut emissions = std::collections::HashMap::new();

        assert!(record_error_emission(
            &mut emissions,
            COLLECTOR_ERROR_KEY,
            start
        ));
        assert!(!record_error_emission(
            &mut emissions,
            COLLECTOR_ERROR_KEY,
            start + std::time::Duration::from_secs(59)
        ));
        assert!(record_error_emission(
            &mut emissions,
            EXECUTOR_ERROR_KEY,
            start
        ));
        assert!(!record_error_emission(
            &mut emissions,
            "unknown",
            start + std::time::Duration::from_secs(60)
        ));
        assert_eq!(emissions.len(), 2);
        assert!(record_error_emission(
            &mut emissions,
            COLLECTOR_ERROR_KEY,
            start + std::time::Duration::from_secs(60)
        ));
        assert_eq!(emissions.len(), 2);
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
