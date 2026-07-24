use serde_derive::{Deserialize, Serialize};
use std::{collections::HashSet, fmt};

use super::rules::WindowCandidate;

#[allow(dead_code)]
pub const DEFAULT_TEMPLATE: &str = r#"# RustDesk-Herbin macOS window targeting
version = 1
mode = "rules"
diagnostics = false
"#;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowTargetingMode {
    Rules,
    Passthrough,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowTargetAction {
    Skip,
    ForwardOnly,
    Activate,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivationPolicy {
    Regular,
    Accessory,
    Prohibited,
    #[allow(dead_code)]
    #[serde(skip_deserializing)]
    Unknown,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct RuleMatcher {
    pub bundle_ids: Vec<String>,
    pub bundle_id_prefixes: Vec<String>,
    pub process_names: Vec<String>,
    pub layers: Vec<i32>,
    pub layer_min: Option<i32>,
    pub layer_max: Option<i32>,
    pub ax_roles: Vec<String>,
    pub ax_subroles: Vec<String>,
    pub activation_policies: Vec<ActivationPolicy>,
    pub covers_display: Option<bool>,
}

impl RuleMatcher {
    pub(crate) fn matches(&self, candidate: &WindowCandidate) -> bool {
        (self.bundle_ids.is_empty()
            || self
                .bundle_ids
                .iter()
                .any(|bundle_id| bundle_id == &candidate.bundle_id))
            && (self.bundle_id_prefixes.is_empty()
                || self
                    .bundle_id_prefixes
                    .iter()
                    .any(|bundle_id_prefix| candidate.bundle_id.starts_with(bundle_id_prefix)))
            && (self.process_names.is_empty()
                || self
                    .process_names
                    .iter()
                    .any(|process_name| process_name == &candidate.process_name))
            && (self.layers.is_empty() || self.layers.contains(&candidate.layer))
            && self
                .layer_min
                .map_or(true, |layer_min| candidate.layer >= layer_min)
            && self
                .layer_max
                .map_or(true, |layer_max| candidate.layer <= layer_max)
            && (self.ax_roles.is_empty()
                || candidate.ax_role.as_ref().map_or(false, |ax_role| {
                    self.ax_roles.iter().any(|role| role == ax_role)
                }))
            && (self.ax_subroles.is_empty()
                || candidate.ax_subrole.as_ref().map_or(false, |ax_subrole| {
                    self.ax_subroles.iter().any(|subrole| subrole == ax_subrole)
                }))
            && (self.activation_policies.is_empty()
                || self
                    .activation_policies
                    .contains(&candidate.activation_policy))
            && self.covers_display.map_or(true, |covers_display| {
                candidate.covers_display == covers_display
            })
    }

    fn is_empty(&self) -> bool {
        self.bundle_ids.is_empty()
            && self.bundle_id_prefixes.is_empty()
            && self.process_names.is_empty()
            && self.layers.is_empty()
            && self.layer_min.is_none()
            && self.layer_max.is_none()
            && self.ax_roles.is_empty()
            && self.ax_subroles.is_empty()
            && self.activation_policies.is_empty()
            && self.covers_display.is_none()
    }

    fn has_bundle_matcher(&self) -> bool {
        !self.bundle_ids.is_empty() || !self.bundle_id_prefixes.is_empty()
    }

    fn has_structural_matcher(&self) -> bool {
        !self.layers.is_empty()
            || self.layer_min.is_some()
            || self.layer_max.is_some()
            || self.covers_display.is_some()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct UserRule {
    id: String,
    #[serde(default)]
    priority: i32,
    action: WindowTargetAction,
    #[serde(default)]
    bundle_ids: Vec<String>,
    #[serde(default)]
    bundle_id_prefixes: Vec<String>,
    #[serde(default)]
    process_names: Vec<String>,
    #[serde(default)]
    layers: Vec<i32>,
    layer_min: Option<i32>,
    layer_max: Option<i32>,
    #[serde(default)]
    ax_roles: Vec<String>,
    #[serde(default)]
    ax_subroles: Vec<String>,
    #[serde(default)]
    activation_policies: Vec<ActivationPolicy>,
    covers_display: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UserConfig {
    version: u32,
    mode: WindowTargetingMode,
    #[serde(default)]
    diagnostics: bool,
    #[serde(default)]
    rules: Vec<UserRule>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CompiledRule {
    pub id: String,
    pub priority: i32,
    pub action: WindowTargetAction,
    pub matcher: RuleMatcher,
}

impl CompiledRule {
    #[allow(dead_code)]
    pub fn new(id: impl Into<String>, action: WindowTargetAction, matcher: RuleMatcher) -> Self {
        Self {
            id: id.into(),
            priority: 0,
            action,
            matcher,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ValidatedUserConfig {
    pub mode: WindowTargetingMode,
    pub diagnostics: bool,
    pub rules: Vec<CompiledRule>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigError(String);

impl ConfigError {
    pub(crate) fn new(message: String) -> Self {
        Self(message)
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ConfigError {}

pub fn parse_user_config(text: &str) -> Result<ValidatedUserConfig, ConfigError> {
    let parsed: UserConfig =
        toml::from_str(text).map_err(|error| ConfigError::new(error.to_string()))?;
    if parsed.version != 1 {
        return Err(ConfigError::new(format!(
            "unsupported window-targeting version: {}",
            parsed.version
        )));
    }

    let mut ids = HashSet::new();
    let mut rules = Vec::with_capacity(parsed.rules.len());
    for rule in parsed.rules {
        let matcher = RuleMatcher {
            bundle_ids: rule.bundle_ids,
            bundle_id_prefixes: rule.bundle_id_prefixes,
            process_names: rule.process_names,
            layers: rule.layers,
            layer_min: rule.layer_min,
            layer_max: rule.layer_max,
            ax_roles: rule.ax_roles,
            ax_subroles: rule.ax_subroles,
            activation_policies: rule.activation_policies,
            covers_display: rule.covers_display,
        };
        if rule.id.trim().is_empty() {
            return Err(ConfigError::new("rule id must not be empty".to_owned()));
        }
        if !ids.insert(rule.id.clone()) {
            return Err(ConfigError::new(format!("duplicate rule id: {}", rule.id)));
        }
        if matcher
            .bundle_ids
            .iter()
            .any(|bundle_id| bundle_id.trim().is_empty())
        {
            return Err(ConfigError::new(format!(
                "rule {}: bundle_ids contains an empty or whitespace-only value",
                rule.id
            )));
        }
        if matcher
            .bundle_id_prefixes
            .iter()
            .any(|bundle_id_prefix| bundle_id_prefix.trim().is_empty())
        {
            return Err(ConfigError::new(format!(
                "rule {}: bundle_id_prefixes contains an empty or whitespace-only value",
                rule.id
            )));
        }
        if matcher.is_empty() {
            return Err(ConfigError::new(format!(
                "rule {} requires at least one matcher",
                rule.id
            )));
        }
        if !matcher.layers.is_empty()
            && (matcher.layer_min.is_some() || matcher.layer_max.is_some())
        {
            return Err(ConfigError::new(format!(
                "rule {}: layers cannot be combined with layer_min or layer_max",
                rule.id
            )));
        }
        if let (Some(layer_min), Some(layer_max)) = (matcher.layer_min, matcher.layer_max) {
            if layer_min > layer_max {
                return Err(ConfigError::new(format!(
                    "rule {}: layer_min cannot exceed layer_max",
                    rule.id
                )));
            }
        }
        if rule.action == WindowTargetAction::Skip
            && (!matcher.has_bundle_matcher() || !matcher.has_structural_matcher())
        {
            return Err(ConfigError::new(format!(
                "rule {}: skip requires bundle and structural matchers",
                rule.id
            )));
        }
        rules.push(CompiledRule {
            id: rule.id,
            priority: rule.priority,
            action: rule.action,
            matcher,
        });
    }
    rules.sort_by(|left, right| right.priority.cmp(&left.priority));
    Ok(ValidatedUserConfig {
        mode: parsed.mode,
        diagnostics: parsed.diagnostics,
        rules,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"
version = 1
mode = "rules"
diagnostics = false

[[rules]]
id = "dock"
priority = 1000
action = "forward_only"
bundle_id_prefixes = ["com.apple.dock"]
layer_min = 1
"#;

    #[test]
    fn parses_valid_rule_config() {
        let parsed = parse_user_config(VALID).unwrap();
        assert_eq!(parsed.mode, WindowTargetingMode::Rules);
        assert_eq!(parsed.rules.len(), 1);
        assert_eq!(parsed.rules[0].id, "dock");
    }

    #[test]
    fn rejects_unknown_fields() {
        let error = parse_user_config(
            "version = 1\nmode = \"rules\"\ndiagnostics = false\nunknown = true\n",
        )
        .unwrap_err();
        assert!(error.to_string().contains("unknown"));
    }

    #[test]
    fn rejects_duplicate_ids() {
        let text = VALID.to_owned()
            + "\n[[rules]]\nid = \"dock\"\naction = \"forward_only\"\nax_roles = [\"AXMenu\"]\n";
        assert!(parse_user_config(&text)
            .unwrap_err()
            .to_string()
            .contains("duplicate rule id"));
    }

    #[test]
    fn rejects_empty_matcher() {
        let text =
            "version = 1\nmode = \"rules\"\n[[rules]]\nid = \"all\"\naction = \"activate\"\n";
        assert!(parse_user_config(text)
            .unwrap_err()
            .to_string()
            .contains("at least one matcher"));
    }

    #[test]
    fn rejects_unsafe_skip() {
        let text = r#"
version = 1
mode = "rules"
[[rules]]
id = "unsafe"
action = "skip"
layer_min = 1
"#;
        assert!(parse_user_config(text)
            .unwrap_err()
            .to_string()
            .contains("skip requires bundle and structural matchers"));
    }

    #[test]
    fn rejects_blank_bundle_id_in_skip_rule() {
        let text = r#"
version = 1
mode = "rules"
[[rules]]
id = "blank_bundle"
action = "skip"
bundle_ids = ["   "]
layer_min = 1
"#;
        let error = parse_user_config(text).unwrap_err().to_string();
        assert!(error.contains("rule blank_bundle: bundle_ids"));
    }

    #[test]
    fn rejects_blank_bundle_id_prefix_in_skip_rule() {
        let text = r#"
version = 1
mode = "rules"
[[rules]]
id = "blank_prefix"
action = "skip"
bundle_id_prefixes = [""]
layer_min = 1
"#;
        let error = parse_user_config(text).unwrap_err().to_string();
        assert!(error.contains("rule blank_prefix: bundle_id_prefixes"));
    }

    #[test]
    fn rejects_exact_and_range_layers_together() {
        let text = r#"
version = 1
mode = "rules"
[[rules]]
id = "layers"
action = "forward_only"
layers = [3]
layer_min = 1
"#;
        assert!(parse_user_config(text)
            .unwrap_err()
            .to_string()
            .contains("layers cannot be combined"));
    }

    #[test]
    fn rejects_contradictory_layer_bounds() {
        let text = r#"
version = 1
mode = "rules"
[[rules]]
id = "bounds"
action = "forward_only"
layer_min = 3
layer_max = 1
"#;
        assert!(parse_user_config(text)
            .unwrap_err()
            .to_string()
            .contains("layer_min cannot exceed layer_max"));
    }
}
