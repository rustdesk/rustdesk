# macOS Window Targeting Rules Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace RDH's blanket macOS layer/Dock click filter with a configurable, deterministic window-targeting rule engine that preserves transient UI, skips confirmed passive overlays, supports a passthrough A/B baseline, and reloads safely through the existing CLI/IPC path.

**Architecture:** Objective-C++ collects bounded CoreGraphics/Accessibility candidate facts and performs only the selected activation/raise operation. A macOS-only Rust module parses and validates TOML, owns immutable generation state, matches ordered rules, exposes diagnostics, and preprocesses remote left clicks without changing original mouse delivery. Dedicated CLI/IPC requests validate, query, and atomically reload the active configuration without restarting `--server`.

**Tech Stack:** Rust 2021, serde/serde_derive, toml 0.8.2, sha2 0.10, Objective-C++17, CoreGraphics, AppKit, public Accessibility APIs, parity-tokio-ipc, Python fork-invariant tests, GitHub Actions macOS 14 CI.

## Global Constraints

- Implement only on macOS; other platforms retain their current input path and build surface.
- Use only public CoreGraphics, AppKit, and Accessibility APIs.
- The original `en.mouse_down(MouseButton::Left)` remains unconditional and stays after targeting preprocessing.
- `passthrough` performs no CoreGraphics or Accessibility target lookup.
- Only `skip` may traverse through a candidate; unknown non-zero-layer candidates use `forward_only`.
- Do not restore a blanket `layer == 0` filter or blanket Dock exclusion.
- Do not add file watching, scripting, a GUI editor, random A/B assignment, automatic learning, or official-application switching.
- Do not log window titles or remote content.
- Do not add a runtime dependency beyond `toml = "0.8.2"`; reuse existing `sha2`, `serde`, and synchronization primitives.
- Do not use `unwrap()` or `expect()` in production paths except lock poisoning, per repository guidance.
- Do not read or parse configuration in the per-click path.
- Keep `implementation-notes.md` current during execution, but preserve its existing dirty content and do not include it in implementation commits.
- Build the full macOS application in GitHub Actions; do not install local build dependencies.
- Keep official RustDesk available as the independent rescue route during candidate installation and live acceptance.

---

## File and Responsibility Map

- Modify `Cargo.toml`
  - Add the already-locked TOML parser as a direct dependency.
- Modify `src/lib.rs`
  - Register the macOS-only `window_targeting` module.
- Create `src/window_targeting.rs`
  - Public feature facade, runtime state, generation/hash/status, disk path, safe initialize/validate/reload, click preprocessing, diagnostics.
- Create `src/window_targeting/config.rs`
  - TOML schema, strict deserialization, semantic validation, template text, compiled effective configuration.
- Create `src/window_targeting/rules.rs`
  - Candidate facts, built-in rules, ordered matching, conservative defaults, decision traces.
- Modify `src/platform/macos.rs`
  - C-compatible candidate record, safe string conversion, collector and activation wrappers.
- Modify `src/platform/macos.mm`
  - Unfiltered bounded candidate collection and selected-candidate activation executor.
- Modify `src/server.rs`
  - Initialize the rule runtime once in the user `--server` process.
- Modify `src/server/input_service.rs`
  - Replace direct activation with the new preprocessing facade immediately before mouse-down.
- Modify `src/ipc.rs`
  - Dedicated status/reload request and response variants, handler, client.
- Modify `src/core_main.rs`
  - `--window-targeting status|validate|reload`, active-user IPC routing, exact exit status.
- Modify `tests/test_herbin_branding.py`
  - Replace obsolete layer/Dock assertions with the new source/ordering/privacy contract.
- Modify `docs/rdh-upgrade-runbook.md`
  - Update the RDH patch contract, upgrade conflict guidance, CLI operations, and runtime acceptance.
- Maintain but do not commit `implementation-notes.md`
  - Record design linkage, deviations, CI artifact, installation, A/B results, and rollback evidence.

---

### Task 1: Strict TOML Schema and Semantic Validation

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/lib.rs`
- Create: `src/window_targeting.rs`
- Create: `src/window_targeting/config.rs`
- Create: `src/window_targeting/rules.rs`
- Test: `src/window_targeting/config.rs`

**Interfaces:**
- Produces: `WindowTargetingMode`, `WindowTargetAction`, `ActivationPolicy`, `RuleMatcher`, `CompiledRule`, `ValidatedUserConfig`.
- Produces: `config::parse_user_config(text: &str) -> Result<ValidatedUserConfig, ConfigError>`.
- Produces: `config::DEFAULT_TEMPLATE: &str`.
- Consumes: no earlier task interface.

- [ ] **Step 1: Register the feature module and write schema tests that fail**

Add to `src/lib.rs`:

```rust
#[cfg(target_os = "macos")]
pub(crate) mod window_targeting;
```

Create `src/window_targeting.rs`:

```rust
#[cfg(target_os = "macos")]
mod config;
#[cfg(target_os = "macos")]
mod rules;

pub(crate) use config::{
    ActivationPolicy, CompiledRule, ConfigError, RuleMatcher, ValidatedUserConfig,
    WindowTargetAction, WindowTargetingMode,
};
```

Create `src/window_targeting/config.rs` with tests covering the exact accepted and rejected shapes:

```rust
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
}
```

- [ ] **Step 2: Run the focused tests and verify the red state**

Run:

```bash
cargo test --lib window_targeting::config::tests -- --nocapture
```

Expected: compilation fails because the schema types and `parse_user_config` are not implemented.

- [ ] **Step 3: Add the TOML dependency and implement the exact schema**

Add under root `[dependencies]` in `Cargo.toml`:

```toml
toml = "0.8.2"
```

Implement these types in `src/window_targeting/config.rs` with
`serde_derive::{Deserialize, Serialize}` and `#[serde(deny_unknown_fields)]`:

```rust
use serde_derive::{Deserialize, Serialize};
use std::{collections::HashSet, fmt};

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
    pub fn new(
        id: impl Into<String>,
        action: WindowTargetAction,
        matcher: RuleMatcher,
    ) -> Self {
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
```

Implement `RuleMatcher::is_empty` by checking every matcher field explicitly.
`has_bundle_matcher` is true only for non-empty `bundle_ids` or
`bundle_id_prefixes`. `has_structural_matcher` is true only for non-empty
`layers`, either layer bound, or `covers_display`; AX role and activation policy
alone do not make a `skip` rule safe.
Implement `parse_user_config` with these exact checks:

```rust
pub fn parse_user_config(text: &str) -> Result<ValidatedUserConfig, ConfigError> {
    let parsed: UserConfig =
        toml::from_str(text).map_err(|error| ConfigError(error.to_string()))?;
    if parsed.version != 1 {
        return Err(ConfigError(format!(
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
            return Err(ConfigError("rule id must not be empty".to_owned()));
        }
        if !ids.insert(rule.id.clone()) {
            return Err(ConfigError(format!("duplicate rule id: {}", rule.id)));
        }
        if matcher.is_empty() {
            return Err(ConfigError(format!(
                "rule {} requires at least one matcher",
                rule.id
            )));
        }
        if !matcher.layers.is_empty()
            && (matcher.layer_min.is_some() || matcher.layer_max.is_some())
        {
            return Err(ConfigError(format!(
                "rule {}: layers cannot be combined with layer_min or layer_max",
                rule.id
            )));
        }
        if rule.action == WindowTargetAction::Skip
            && (!matcher.has_bundle_matcher()
                || !matcher.has_structural_matcher())
        {
            return Err(ConfigError(format!(
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
```

The sort must remain stable so equal-priority file order is preserved.

- [ ] **Step 4: Run schema tests and formatting**

Run:

```bash
cargo test --lib window_targeting::config::tests -- --nocapture
cargo fmt -- --check
```

Expected: all config tests pass; formatting check exits zero.

- [ ] **Step 5: Record the decision and commit**

Append an unstaged `macOS window-targeting rules` section to
`implementation-notes.md` linking the approved design and recording that TOML
0.8.2 is already present in `Cargo.lock`. Do not stage that file.

Commit:

```bash
git add Cargo.toml Cargo.lock src/lib.rs src/window_targeting.rs src/window_targeting/config.rs src/window_targeting/rules.rs
git commit -m "feat: validate macOS window targeting config"
```

---

### Task 2: Ordered Rule Engine and Recorded Candidate Fixtures

**Files:**
- Modify: `src/window_targeting.rs`
- Modify: `src/window_targeting/config.rs`
- Modify: `src/window_targeting/rules.rs`
- Test: `src/window_targeting/rules.rs`

**Interfaces:**
- Consumes: `ValidatedUserConfig`, `CompiledRule`, `RuleMatcher`, `WindowTargetingMode`, `WindowTargetAction`, `ActivationPolicy`.
- Produces: `WindowCandidate`, `EffectiveConfig`, `DecisionStep`, `WindowDecision`.
- Produces: `rules::compile_effective(user: Option<ValidatedUserConfig>) -> EffectiveConfig`.
- Produces: `rules::decide(config: &EffectiveConfig, candidates: &[WindowCandidate]) -> WindowDecision`.

- [ ] **Step 1: Write fixture tests for every required candidate class**

Define `WindowCandidate` and tests in `src/window_targeting/rules.rs`:

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct WindowCandidate {
    pub pid: i32,
    pub window_id: u32,
    pub bundle_id: String,
    pub process_name: String,
    pub layer: i32,
    pub alpha: f64,
    pub activation_policy: ActivationPolicy,
    pub covers_display: bool,
    pub ax_role: Option<String>,
    pub ax_subrole: Option<String>,
}

#[cfg(test)]
impl WindowCandidate {
    fn dock_menu_fixture() -> Self {
        Self {
            pid: 64334,
            window_id: 64334,
            bundle_id: "com.apple.dock.helper".to_owned(),
            process_name: "DockHelper".to_owned(),
            layer: 101,
            alpha: 1.0,
            activation_policy: ActivationPolicy::Accessory,
            covers_display: false,
            ax_role: Some("AXMenuItem".to_owned()),
            ax_subrole: None,
        }
    }

    fn regular_fixture(pid: i32) -> Self {
        Self {
            pid,
            window_id: pid as u32,
            bundle_id: "com.openai.chat".to_owned(),
            process_name: "ChatGPT".to_owned(),
            layer: 0,
            alpha: 1.0,
            activation_policy: ActivationPolicy::Regular,
            covers_display: false,
            ax_role: Some("AXWindow".to_owned()),
            ax_subrole: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        pid: i32,
        bundle_id: &str,
        layer: i32,
        policy: ActivationPolicy,
        role: Option<&str>,
        covers_display: bool,
    ) -> WindowCandidate {
        WindowCandidate {
            pid,
            window_id: pid as u32,
            bundle_id: bundle_id.to_owned(),
            process_name: bundle_id.to_owned(),
            layer,
            alpha: 1.0,
            activation_policy: policy,
            covers_display,
            ax_role: role.map(str::to_owned),
            ax_subrole: None,
        }
    }

    #[test]
    fn dock_menu_stops_without_activating_chatgpt_below() {
        let config = compile_effective(None);
        let candidates = vec![
            candidate(
                64334,
                "com.apple.dock.helper",
                101,
                ActivationPolicy::Accessory,
                Some("AXMenuItem"),
                false,
            ),
            candidate(
                80988,
                "com.openai.chat",
                0,
                ActivationPolicy::Regular,
                Some("AXWindow"),
                false,
            ),
        ];
        let decision = decide(&config, &candidates);
        assert_eq!(decision.action, WindowTargetAction::ForwardOnly);
        assert_eq!(decision.candidate_index, Some(0));
        assert_eq!(decision.rule_id, "builtin.interactive-transient");
    }

    #[test]
    fn notification_overlay_skips_to_regular_window() {
        let config = compile_effective(None);
        let candidates = vec![
            candidate(
                1119,
                "com.apple.notificationcenterui",
                21,
                ActivationPolicy::Accessory,
                None,
                true,
            ),
            candidate(
                80988,
                "com.openai.chat",
                0,
                ActivationPolicy::Regular,
                Some("AXWindow"),
                false,
            ),
        ];
        let decision = decide(&config, &candidates);
        assert_eq!(decision.action, WindowTargetAction::Activate);
        assert_eq!(decision.candidate_index, Some(1));
        assert_eq!(decision.trace[0].action, WindowTargetAction::Skip);
    }

    #[test]
    fn unknown_nonzero_layer_is_conservative() {
        let config = compile_effective(None);
        let candidates = vec![candidate(
            7,
            "com.example.unknown",
            8,
            ActivationPolicy::Accessory,
            None,
            false,
        )];
        assert_eq!(
            decide(&config, &candidates).action,
            WindowTargetAction::ForwardOnly
        );
    }

    #[test]
    fn regular_layer_zero_activates() {
        let config = compile_effective(None);
        let candidates = vec![candidate(
            9,
            "com.openai.chat",
            0,
            ActivationPolicy::Regular,
            Some("AXWindow"),
            false,
        )];
        assert_eq!(
            decide(&config, &candidates).action,
            WindowTargetAction::Activate
        );
    }

    #[test]
    fn user_rule_precedes_builtin_rule() {
        let user = crate::window_targeting::config::parse_user_config(
            r#"
version = 1
mode = "rules"
[[rules]]
id = "dock-experiment"
priority = 50
action = "activate"
bundle_id_prefixes = ["com.apple.dock"]
"#,
        )
        .unwrap();
        let config = compile_effective(Some(user));
        let candidates = vec![candidate(
            64334,
            "com.apple.dock.helper",
            101,
            ActivationPolicy::Accessory,
            Some("AXMenuItem"),
            false,
        )];
        assert_eq!(decide(&config, &candidates).rule_id, "dock-experiment");
    }

    #[test]
    fn finder_menu_and_popover_are_forward_only() {
        for role in ["AXMenuItem", "AXPopover"] {
            let candidates = vec![
                candidate(
                    501,
                    "com.apple.finder",
                    25,
                    ActivationPolicy::Regular,
                    Some(role),
                    false,
                ),
                candidate(
                    80988,
                    "com.openai.chat",
                    0,
                    ActivationPolicy::Regular,
                    Some("AXWindow"),
                    false,
                ),
            ];
            let decision = decide(&compile_effective(None), &candidates);
            assert_eq!(decision.action, WindowTargetAction::ForwardOnly);
            assert_eq!(decision.candidate_index, Some(0));
        }
    }

    #[test]
    fn same_application_uses_topmost_concrete_candidate() {
        let candidates = vec![
            candidate(
                80988,
                "com.openai.chat",
                0,
                ActivationPolicy::Regular,
                Some("AXWindow"),
                false,
            ),
            candidate(
                80988,
                "com.openai.chat",
                0,
                ActivationPolicy::Regular,
                Some("AXWindow"),
                false,
            ),
        ];
        let decision = decide(&compile_effective(None), &candidates);
        assert_eq!(decision.action, WindowTargetAction::Activate);
        assert_eq!(decision.candidate_index, Some(0));
    }

    #[test]
    fn equal_priority_preserves_file_order() {
        let user = crate::window_targeting::config::parse_user_config(
            r#"
version = 1
mode = "rules"
[[rules]]
id = "first"
priority = 10
action = "forward_only"
bundle_ids = ["com.example.target"]
[[rules]]
id = "second"
priority = 10
action = "activate"
bundle_ids = ["com.example.target"]
"#,
        )
        .unwrap();
        let candidates = vec![candidate(
            7,
            "com.example.target",
            0,
            ActivationPolicy::Regular,
            Some("AXWindow"),
            false,
        )];
        assert_eq!(
            decide(&compile_effective(Some(user)), &candidates).rule_id,
            "first"
        );
    }
}
```

- [ ] **Step 2: Run fixture tests and verify they fail**

Run:

```bash
cargo test --lib window_targeting::rules::tests -- --nocapture
```

Expected: failure because `compile_effective`, `decide`, and decision types are
not implemented.

- [ ] **Step 3: Implement built-ins, AND/OR matching, and first-match traversal**

Implement:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionStep {
    pub candidate_index: usize,
    pub rule_id: String,
    pub action: WindowTargetAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowDecision {
    pub action: WindowTargetAction,
    pub candidate_index: Option<usize>,
    pub rule_id: String,
    pub trace: Vec<DecisionStep>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EffectiveConfig {
    pub mode: WindowTargetingMode,
    pub diagnostics: bool,
    pub rules: Vec<CompiledRule>,
}
```

At the facade in `src/window_targeting.rs`, add:

```rust
pub(crate) use rules::{
    DecisionStep, EffectiveConfig, WindowCandidate, WindowDecision,
};
```

Implement `RuleMatcher::matches` so every non-empty matcher field must match,
while values inside a field are alternatives. Implement built-ins in this exact
order:

```rust
fn built_in_rules() -> Vec<CompiledRule> {
    vec![
        CompiledRule::new(
            "builtin.interactive-transient",
            WindowTargetAction::ForwardOnly,
            RuleMatcher {
                ax_roles: vec![
                    "AXMenu".to_owned(),
                    "AXMenuItem".to_owned(),
                    "AXPopover".to_owned(),
                ],
                ..RuleMatcher::default()
            },
        ),
        CompiledRule::new(
            "builtin.dock-ui",
            WindowTargetAction::ForwardOnly,
            RuleMatcher {
                bundle_id_prefixes: vec!["com.apple.dock".to_owned()],
                ..RuleMatcher::default()
            },
        ),
        CompiledRule::new(
            "builtin.notification-center-overlay",
            WindowTargetAction::Skip,
            RuleMatcher {
                bundle_id_prefixes: vec![
                    "com.apple.notificationcenterui".to_owned(),
                ],
                layer_min: Some(1),
                covers_display: Some(true),
                ..RuleMatcher::default()
            },
        ),
    ]
}
```

`compile_effective` appends built-ins after already priority-sorted user rules.
`decide` evaluates each candidate against all effective rules. A matched `skip`
adds a trace step and continues. A matched non-skip action returns immediately.
When no explicit rule matches one candidate:

```rust
let default_action = if candidate.layer == 0
    && candidate.activation_policy == ActivationPolicy::Regular
{
    WindowTargetAction::Activate
} else {
    WindowTargetAction::ForwardOnly
};
```

If every candidate was skipped or there are no candidates, return
`forward_only` with rule ID `default.no-target`.

Add focused matcher tests proving that two populated fields are AND conditions
and multiple values within one field are OR conditions. Each test must assert
both its matching and non-matching cases rather than checking only the happy
path.

- [ ] **Step 4: Run all window-targeting unit tests**

Run:

```bash
cargo test --lib window_targeting -- --nocapture
cargo fmt -- --check
```

Expected: config and rule tests pass; formatting exits zero.

- [ ] **Step 5: Commit**

```bash
git add src/window_targeting.rs src/window_targeting/config.rs src/window_targeting/rules.rs
git commit -m "feat: classify macOS window candidates"
```

---

### Task 3: Immutable Runtime State, Template, Hash, Validate, and Reload

**Files:**
- Modify: `src/window_targeting.rs`
- Modify: `src/window_targeting/config.rs`
- Modify: `src/window_targeting/rules.rs`
- Modify: `src/server.rs`
- Test: `src/window_targeting.rs`

**Interfaces:**
- Consumes: `compile_effective`, `parse_user_config`, `EffectiveConfig`.
- Produces: `WindowTargetingStatus`.
- Produces: `initialize()`, `snapshot()`, `status()`, `validate_from_disk()`, `reload_from_disk()`.
- Produces: an internal `RuntimeState` with path-injected methods for isolated unit tests.

- [ ] **Step 1: Write runtime-state tests**

Add tests in `src/window_targeting.rs` using a unique directory under
`std::env::temp_dir()`:

```rust
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
```

- [ ] **Step 2: Run tests and verify they fail**

Run:

```bash
cargo test --lib window_targeting::tests -- --nocapture
```

Expected: failure because `RuntimeState`, status, template, and reload functions
do not exist.

- [ ] **Step 3: Implement runtime state and deterministic effective hash**

Implement in `src/window_targeting.rs`:

```rust
use lazy_static::lazy_static;
use sha2::{Digest, Sha256};
use std::{
    fs,
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

#[derive(Clone)]
struct ActiveGeneration {
    generation: u64,
    hash: String,
    source: String,
    effective: EffectiveConfig,
}

#[cfg(test)]
impl ActiveGeneration {
    fn for_test(mode: WindowTargetingMode) -> Self {
        let user = ValidatedUserConfig {
            mode,
            diagnostics: false,
            rules: Vec::new(),
        };
        build_generation(1, Some(user), "test")
    }

    fn builtin_for_test() -> Self {
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
        parse_user_config(&text)
    }

    fn validate_path(
        &self,
        path: &Path,
    ) -> Result<WindowTargetingValidation, ConfigError> {
        let candidate = build_generation(
            0,
            Some(Self::load_user_config(path)?),
            "validation",
        );
        Ok(validation_from_generation(&candidate))
    }

    fn reload_from_path(&self, path: &Path) -> Result<WindowTargetingStatus, ConfigError> {
        let user = Self::load_user_config(path)?;
        let current = self.snapshot();
        let replacement = Arc::new(build_generation(
            current.generation + 1,
            Some(user),
            "builtin+user",
        ));
        *self.active.write().unwrap() = replacement;
        Ok(self.status())
    }
}
```

`build_generation` must call `compile_effective` and hash an infallible
length-prefixed canonical encoding with `Sha256`, then hex-encode it using the
existing `hex` dependency. Implement one small `EffectiveHashEncoder` that:

- prefixes every scalar with a fixed one-byte field/type tag;
- encodes integers as fixed-width little-endian bytes;
- encodes strings as `u64` byte length plus UTF-8 bytes;
- encodes arrays as `u64` element count followed by elements in order;
- encodes `Option` as a `0`/`1` byte followed by its value when present;
- encodes enums with their stable snake-case names;
- visits mode, diagnostics, and every ordered `CompiledRule`/`RuleMatcher`
  field in source declaration order.

This avoids a fallible serializer in the built-in startup path and prevents
ambiguous concatenations. Hash input includes effective mode, diagnostics
state, and ordered effective rules; it excludes `generation` and `source`.

Define one global:

```rust
lazy_static! {
    static ref RUNTIME: RuntimeState = RuntimeState::new_builtin();
}
```

Implement the config path with the existing trusted active-user home lookup:

```rust
pub fn config_path() -> Result<PathBuf, ConfigError> {
    let home = crate::platform::get_active_user_home()
        .ok_or_else(|| ConfigError::new("active user home is unavailable".to_owned()))?;
    Ok(home
        .join("Library")
        .join("Application Support")
        .join("RustDesk-Herbin")
        .join("window-targeting.toml"))
}
```

`ensure_template_at` must create parent directories, then use
`OpenOptions::new().write(true).create_new(true)` so an existing file is never
rewritten. Treat `AlreadyExists` as success.

`initialize` behavior:

- ensure the minimal template when absent;
- load one validated user generation when possible;
- on any startup read/validation error, log the error and retain generation 1
  built-ins;
- log one concise status line.

Define:

```rust
#[derive(Clone, Debug, Eq, PartialEq, serde_derive::Serialize)]
pub struct WindowTargetingValidation {
    pub mode: WindowTargetingMode,
    pub rule_count: usize,
    pub hash: String,
    pub diagnostics: bool,
}
```

Implement `status_from_generation` and `validation_from_generation` as direct
field projections. Both report `effective.rules.len()` and the stored hash;
only status includes `generation` and `source`.

`validate_path` and `validate_from_disk` must compile the complete effective
configuration (user rules plus built-ins) and return
`WindowTargetingValidation`, not merely the parsed user schema. The deterministic
hash therefore matches the generation that `reload` would install, while
validation never mutates `RUNTIME`.
`reload_from_disk` delegates to `RUNTIME.reload_from_path`.

- [ ] **Step 4: Initialize only in the user server startup path**

In `src/server.rs`, after `wait_initial_config_sync().await` and before
`memory_watchdog::start()`:

```rust
#[cfg(target_os = "macos")]
crate::window_targeting::initialize();
```

Do not initialize from the root service or from each connection.

- [ ] **Step 5: Run runtime and full feature tests**

Run:

```bash
cargo test --lib window_targeting -- --nocapture
cargo fmt -- --check
```

Expected: every window-targeting test passes; invalid reload preserves the
previous status exactly.

- [ ] **Step 6: Commit**

```bash
git add src/window_targeting.rs src/window_targeting/config.rs src/window_targeting/rules.rs src/server.rs
git commit -m "feat: reload macOS window targeting rules safely"
```

---

### Task 4: Bounded macOS Candidate Collector and Selected Executor

**Files:**
- Modify: `src/platform/macos.rs`
- Modify: `src/platform/macos.mm`
- Modify: `tests/test_herbin_branding.py`
- Test: `tests/test_herbin_branding.py`

**Interfaces:**
- Consumes: `window_targeting::WindowCandidate`, `ActivationPolicy`.
- Produces: `platform::macos::collect_window_candidates_at_point(x: i32, y: i32) -> Result<Vec<WindowCandidate>, MacWindowCollectionError>`.
- Produces: `platform::macos::activate_window_candidate_at_point(x: i32, y: i32, expected_pid: i32) -> MacWindowActivationOutcome`.
- Removes: direct `MacActivateApplicationAtPoint` and `activate_application_at_point`.

- [ ] **Step 1: Change the fork contract first and verify it fails**

Replace obsolete assertions in `tests/test_herbin_branding.py` with:

```python
    assert "fn MacCollectWindowCandidatesAtPoint" in macos_rs
    assert "fn MacActivateWindowCandidateAtPoint" in macos_rs
    assert "pub fn collect_window_candidates_at_point" in macos_rs
    assert "pub fn activate_window_candidate_at_point" in macos_rs
    assert "MacCollectWindowCandidatesAtPoint" in macos_mm
    assert "MacActivateWindowCandidateAtPoint" in macos_mm
    assert "layer.intValue != 0" not in macos_mm
    assert 'bundleIdentifier isEqualToString:@"com.apple.dock"' not in macos_mm
    assert "MAX_MAC_WINDOW_CANDIDATES" in macos_rs
    assert "NSApplicationActivationPolicyRegular" in macos_mm
    assert "activateWithOptions" in macos_mm
```

Keep the public Accessibility markers and private API rejection assertions.

Run:

```bash
python3 tests/test_herbin_branding.py
```

Expected: assertion failure because the production collector/executor symbols do
not exist and the old blanket filters remain.

- [ ] **Step 2: Define one bounded C ABI record on both sides**

Use `MAX_MAC_WINDOW_CANDIDATES = 64` and
`MAC_WINDOW_STRING_CAPACITY = 256`.

Define in `src/platform/macos.rs`:

```rust
const MAX_MAC_WINDOW_CANDIDATES: usize = 64;
const MAC_WINDOW_STRING_CAPACITY: usize = 256;

#[repr(C)]
#[derive(Clone, Copy)]
struct MacWindowCandidateRecord {
    pid: i32,
    window_id: u32,
    layer: i32,
    activation_policy: i32,
    alpha: f64,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    covers_display: u8,
    reserved: [u8; 7],
    bundle_id: [u8; MAC_WINDOW_STRING_CAPACITY],
    process_name: [u8; MAC_WINDOW_STRING_CAPACITY],
    ax_role: [u8; MAC_WINDOW_STRING_CAPACITY],
    ax_subrole: [u8; MAC_WINDOW_STRING_CAPACITY],
}

#[derive(Debug)]
pub struct MacWindowCollectionError;

impl std::fmt::Display for MacWindowCollectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("macOS window candidate collection failed")
    }
}

impl std::error::Error for MacWindowCollectionError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MacWindowActivationOutcome {
    pub result: i32,
    pub application_activation_attempted: bool,
    pub window_raise_attempted: bool,
}
```

Declare the same field order and fixed sizes in `src/platform/macos.mm` with
`uint8_t` and `char[256]`. Add compile-time C++ `static_assert` checks for record
size (`1088` bytes) and string capacity (`256` bytes). Zero each output record
before filling it.

- [ ] **Step 3: Implement collection without blanket layer or Dock filtering**

Expose:

```cpp
extern "C" int32_t MacCollectWindowCandidatesAtPoint(
    double x,
    double y,
    MacWindowCandidateRecord *records,
    size_t capacity
);
```

The implementation must:

1. return zero for null output or zero capacity;
2. call `CGWindowListCopyWindowInfo` with on-screen and desktop-exclusion flags,
   returning `-1` if collection fails;
3. capture one system-wide AX hit PID, role, and subrole before traversal;
4. traverse CoreGraphics order without rejecting non-zero layers or Dock owners;
5. retain the existing alpha `<= 0.01` and pointer-outside-bounds rejection;
6. fill no more than `capacity` records;
7. map AppKit activation policies to `0=regular`, `1=accessory`,
   `2=prohibited`, `3=unknown`;
8. set AX role/subrole only when the AX hit PID equals that candidate's owner PID;
9. set `covers_display` using `CGGetDisplaysWithPoint` and `CGDisplayBounds`,
   allowing one logical point on every edge;
10. use bounded string copying and always NUL-terminate;
11. release the window list and every retained AX/Core Foundation value.

- [ ] **Step 4: Implement the selected-candidate executor**

Replace `MacActivateApplicationAtPoint` with:

```cpp
extern "C" int32_t MacActivateWindowCandidateAtPoint(
    double x,
    double y,
    int32_t expectedPid,
    uint8_t *applicationActivationAttempted,
    uint8_t *windowRaiseAttempted
);
```

The function must never re-run CoreGraphics owner selection. It must:

- initialize both non-null attempt outputs to zero before any early return;
- resolve exactly `expectedPid`;
- reject missing, terminated, or non-regular applications without activating a
  substitute;
- attempt to capture the AX hit window at the coordinate and retain it only
  when its PID equals `expectedPid`;
- when AX access/hit testing is unavailable or the PID does not match, skip
  window raising but still allow activation of the already selected
  `expectedPid`;
- retain current frontmost and focused-window comparisons when a PID-validated
  AX window is available;
- activate only `expectedPid` when it is not frontmost;
- raise only the PID-validated target AX window when needed;
- set the activation-attempt output immediately before `activateWithOptions`
  and the raise-attempt output immediately before `AXRaise`;
- preserve the existing result convention: positive PID when ordering changed,
  zero when no ordering changed, negative PID when activation/raise failed;
- always release retained values.

- [ ] **Step 5: Implement safe Rust wrappers**

Add both extern declarations in `src/platform/macos.rs`. Use an uninitialized
fixed array and pass its pointer and capacity. Treat a negative return value as
`Err(MacWindowCollectionError)`; cap every non-negative count again in Rust and
convert only initialized records.

String conversion must stop at the first NUL byte and use
`String::from_utf8_lossy`. Map activation policy integers explicitly; unknown
values become `ActivationPolicy::Unknown`.

Return `Result<Vec<WindowCandidate>, MacWindowCollectionError>` from the
collector wrapper and retain the existing autorelease pool around both wrapper
calls. The executor wrapper passes two local `u8` outputs and returns
`MacWindowActivationOutcome`; only zero means false. A failed CoreGraphics
collection must remain distinguishable from a successful empty result.

- [ ] **Step 6: Run the source contract**

Run:

```bash
python3 tests/test_herbin_branding.py
cargo fmt -- --check
git diff --check
```

Expected: all fork invariants pass, the old blanket filter markers are absent,
and public AX/private API contracts remain intact.

- [ ] **Step 7: Commit**

```bash
git add src/platform/macos.rs src/platform/macos.mm tests/test_herbin_branding.py
git commit -m "feat: collect macOS click candidates"
```

---

### Task 5: Click Preprocessing, Passthrough, and Opt-In Diagnostics

**Files:**
- Modify: `src/window_targeting.rs`
- Modify: `src/server/input_service.rs`
- Modify: `tests/test_herbin_branding.py`
- Test: `src/window_targeting.rs`
- Test: `tests/test_herbin_branding.py`

**Interfaces:**
- Consumes: runtime snapshot, platform collector/executor, `rules::decide`.
- Produces: `preprocess_remote_left_click(x: i32, y: i32) -> PreprocessOutcome`.
- Produces: internal injected `preprocess_with` seam for unit tests.

- [ ] **Step 1: Write tests proving passthrough and raw-event-safe decisions**

Add tests with counters:

```rust
fn activation_outcome(result: i32) -> crate::platform::macos::MacWindowActivationOutcome {
    crate::platform::macos::MacWindowActivationOutcome {
        result,
        application_activation_attempted: result != 0,
        window_raise_attempted: false,
    }
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
        |_, _| Ok(vec![WindowCandidate::dock_menu_fixture()]),
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
    let calls = std::cell::RefCell::new(Vec::new());
    let outcome = preprocess_with(
        &active,
        500,
        500,
        |_, _| Ok(vec![WindowCandidate::regular_fixture(80988)]),
        |x, y, pid| {
            calls.borrow_mut().push((x, y, pid));
            activation_outcome(pid)
        },
    );
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
fn diagnostics_use_only_allowlisted_candidate_fields() {
    let active = ActiveGeneration::builtin_for_test();
    let outcome = preprocess_with(
        &active,
        500,
        500,
        |_, _| Ok(vec![WindowCandidate::regular_fixture(80988)]),
        |_, _, pid| activation_outcome(pid),
    );
    let line = format_diagnostic_line(&outcome);
    assert!(line.contains("bundle_id=com.openai.chat"));
    assert!(line.contains("pid=80988"));
    assert!(line.contains("rule_id="));
    assert!(!line.contains("title="));
    assert!(!line.contains("peer="));
}
```

- [ ] **Step 2: Run tests and verify they fail**

Run:

```bash
cargo test --lib window_targeting::tests -- --nocapture
```

Expected: failure because preprocessing functions and outcome do not exist.

- [ ] **Step 3: Implement one pure decision/execution seam**

Define:

```rust
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
```

Implement:

```rust
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
    ) -> Result<
        Vec<WindowCandidate>,
        crate::platform::macos::MacWindowCollectionError,
    >,
    E: FnOnce(
        i32,
        i32,
        i32,
    ) -> crate::platform::macos::MacWindowActivationOutcome,
```

Behavior:

- capture `Instant::now()` at entry;
- return immediately in passthrough with action `forward_only`, rule ID
  `mode.passthrough`, and no candidate/activation/error, without calling either
  closure;
- collect once in rules mode;
- when collection returns `Err`, return `forward_only` with rule ID
  `error.collector`, the contextual error string, and no candidate or executor
  call;
- decide once against that same active generation;
- call the executor exactly once only for `activate` with a selected candidate;
- never retry;
- move the selected candidate from the collected vector into the outcome after
  the decision, avoiding an additional clone of its strings;
- if the executor outcome contains a negative result, retain
  `activation failed for pid=<pid>` in `error`;
- stop the elapsed timer before any diagnostic formatting or log emission and
  return action/rule/candidate/result/error plus elapsed microseconds.

`preprocess_remote_left_click` clones the active generation, calls
`preprocess_with` with the two platform wrappers, and emits one debug diagnostic
line only when that generation has `diagnostics = true`. The log contains mode,
generation, hash, PID, bundle ID, process name, layer, activation policy,
`covers_display`, AX role/subrole, rule ID, action, activation result, and
the two attempt flags, and elapsed time. It contains no window title or peer
data.
Implement that allowlist in one pure `format_diagnostic_line` helper exercised
by the unit test above.

Use a `lazy_static!` `Mutex<HashMap<&'static str, Instant>>` with exactly two
possible keys: `"collector"` and `"executor"`. Emit an error for the same key at
most once per 60 seconds, so the map can never exceed two entries. Successful
diagnostic decision lines are not error-rate-limit keys.

Construct the same decision trace and `PreprocessOutcome` regardless of the
diagnostics flag; diagnostics may only control formatting and emission after
the elapsed timer has stopped. This keeps the measured core preprocessing path
identical when diagnostics are disabled.

- [ ] **Step 4: Replace the direct activation call**

In `src/server/input_service.rs`, replace the current macOS block with:

```rust
#[cfg(target_os = "macos")]
if let Some((x, y)) = crate::get_cursor_pos() {
    crate::window_targeting::preprocess_remote_left_click(x, y);
}
allow_err!(en.mouse_down(MouseButton::Left));
```

Do not alter right, middle, back, movement, scrolling, or local mouse paths.

Update `tests/test_herbin_branding.py` to assert:

```python
    assert "preprocess_remote_left_click(x, y)" in input_service_rs
    assert_in_order(
        input_service_rs,
        "preprocess_remote_left_click(x, y)",
        "en.mouse_down(MouseButton::Left)",
    )
    assert 'diagnostics = false' in read("src/window_targeting/config.rs")
    assert "window title" not in read("src/window_targeting.rs").lower()
```

- [ ] **Step 5: Run focused verification**

Run:

```bash
cargo test --lib window_targeting -- --nocapture
python3 tests/test_herbin_branding.py
cargo fmt -- --check
git diff --check
```

Expected: all tests pass; passthrough does not call collector/executor; original
left mouse-down follows preprocessing in source order.

- [ ] **Step 6: Commit**

```bash
git add src/window_targeting.rs src/server/input_service.rs tests/test_herbin_branding.py
git commit -m "feat: preprocess remote macOS clicks by rule"
```

---

### Task 6: Dedicated CLI and Active-User IPC Reload

**Files:**
- Modify: `src/window_targeting.rs`
- Modify: `src/ipc.rs`
- Modify: `src/core_main.rs`
- Test: `src/window_targeting.rs`
- Test: `src/core_main.rs`

**Interfaces:**
- Consumes: `status`, `validate_from_disk`, `reload_from_disk`.
- Produces: `WindowTargetingRequest::{Status, Reload}`.
- Produces: `WindowTargetingResponse { ok: bool, lines: Vec<String> }`.
- Produces: `ipc::request_window_targeting(request) -> ResultType<WindowTargetingResponse>`.
- Produces: CLI `--window-targeting status|validate|reload`.

- [ ] **Step 1: Write CLI parser and IPC state-preservation tests**

Add pure parser tests:

```rust
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
```

Extend `core_main.rs`'s existing `user_main_ipc_scope_cli_command_matches_management_commands_only`
test so `--window-targeting` is in the positive list.

Add an IPC handler unit seam test that sends `Reload` with an invalid temporary
file and asserts the response is `ok=false` while status generation/hash remain
unchanged.

- [ ] **Step 2: Run tests and verify they fail**

Run:

```bash
cargo test --lib parses_window_targeting_cli_operations -- --nocapture
cargo test --lib user_main_ipc_scope_cli_command_matches_management_commands_only -- --nocapture
```

Expected: failure because the parser and management-command classification do
not include this feature.

- [ ] **Step 3: Add dedicated serializable IPC types and handler**

In `src/window_targeting.rs`:

```rust
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
```

In `Data` within `src/ipc.rs`, behind `#[cfg(target_os = "macos")]`, add:

```rust
WindowTargetingRequest(crate::window_targeting::WindowTargetingRequest),
WindowTargetingResponse(crate::window_targeting::WindowTargetingResponse),
```

In the main IPC handler, handle only requests:

```rust
#[cfg(target_os = "macos")]
Data::WindowTargetingRequest(request) => {
    let response = crate::window_targeting::handle_ipc_request(request);
    allow_err!(stream
        .send(&Data::WindowTargetingResponse(response))
        .await);
}
```

`handle_ipc_request(Status)` cannot mutate state.
`handle_ipc_request(Reload)` calls the atomic reload and formats either the new
status or two lines: a quoted `ERROR reason=...` line followed by an
`ACTIVE mode=... rules=... generation=... hash=... diagnostics=... source=...
unchanged=true` line. A successful status/reload response contains exactly one
`OK mode=... rules=... generation=... hash=... diagnostics=... source=...`
line. Format error reasons by replacing CR/LF with spaces and escaping
backslashes and double quotes before placing them inside `reason="..."`; never
emit an unescaped multi-line parser error.

Add a current-thread IPC client function that connects to the main postfix,
sends one request, waits up to 1000 ms for exactly
`Data::WindowTargetingResponse`, and errors on timeout or wrong response type.
Do not reuse `Options`.

- [ ] **Step 4: Add CLI execution and exact exit behavior**

Implement:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CliOperation {
    Status,
    Validate,
    Reload,
}
```

`validate` runs the shared disk loader locally and prints:

```text
OK mode=rules rules=3 hash=<64 lowercase hex characters> diagnostics=false
```

`status` and `reload` use the IPC helper and print each response line in order.
Return code is zero only for an `OK` response. Invalid operation, validation
failure, IPC failure, and reload failure return code 1.

Add to `core_main.rs` before generic UI fallthrough:

```rust
#[cfg(target_os = "macos")]
} else if args[0] == "--window-targeting" {
    let exit_code = crate::window_targeting::run_cli(&args[1..]);
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    return None;
```

Add `--window-targeting` to `is_user_main_ipc_scope_cli_command` so an
administrative invocation targets the active user's unique `--server`.

- [ ] **Step 5: Run CLI and IPC tests**

Run:

```bash
cargo test --lib window_targeting -- --nocapture
cargo test --lib user_main_ipc_scope_cli_command_matches_management_commands_only -- --nocapture
python3 tests/test_herbin_branding.py
cargo fmt -- --check
git diff --check
```

Expected: all tests pass; reload failure preserves state; command routing test
includes `--window-targeting`.

- [ ] **Step 6: Commit**

```bash
git add src/window_targeting.rs src/ipc.rs src/core_main.rs
git commit -m "feat: reload window targeting rules over IPC"
```

---

### Task 7: Upgrade Contract, Integrated Checks, and macOS Candidate Build

**Files:**
- Modify: `tests/test_herbin_branding.py`
- Modify: `docs/rdh-upgrade-runbook.md`
- Maintain unstaged: `implementation-notes.md`

**Interfaces:**
- Consumes: all production interfaces from Tasks 1-6.
- Produces: durable fork invariants and upgrade instructions for future upstream merges.
- Produces: CI artifact `rustdesk-herbin-1.4.9-rdh.9-aarch64.dmg`.

- [ ] **Step 1: Update the durable source contract**

Ensure `tests/test_herbin_branding.py` verifies:

- module registration and all three CLI operation strings;
- `passthrough` exists and preprocessing remains before mouse-down;
- original mouse-down is unconditional;
- bounded collector/executor symbols exist;
- blanket layer and Dock exclusion markers are absent;
- built-in Dock, interactive-transient, and Notification Center rule IDs exist;
- private APIs remain absent;
- `diagnostics` defaults false;
- no file watcher crate or watcher marker is introduced;
- memory watchdog, branding, signing, and LaunchAgent invariants remain intact.

Run:

```bash
python3 tests/test_herbin_branding.py
```

Expected: exit zero.

- [ ] **Step 2: Update the upgrade runbook**

Revise `docs/rdh-upgrade-runbook.md` so the patch contract says:

- macOS click preprocessing uses configurable `skip`, `forward_only`, and
  `activate` decisions;
- Dock/transient non-zero-layer UI is not blanket-filtered;
- passive Notification Center overlay recognition remains an explicit built-in
  rule;
- `mode = "passthrough"` is the upstream mouse-behavior baseline;
- `status`, `validate`, and `reload` are the supported management operations;
- future merges must preserve unconditional original mouse delivery and reject
  file watching/private APIs.

Add the Finder menu, Finder popover, Dock menu, Notification Center, and
passthrough/rules A/B cases to runtime acceptance.

- [ ] **Step 3: Run the complete source verification batch**

Run:

```bash
cargo test --lib window_targeting -- --nocapture
cargo test --lib user_main_ipc_scope_cli_command_matches_management_commands_only -- --nocapture
python3 tests/test_herbin_branding.py
cargo fmt -- --check
git diff --check
git status --short
```

Expected:

- every focused test passes;
- formatting and diff checks exit zero;
- only intended source/docs changes plus the preserved unstaged
  `implementation-notes.md` are present.

- [ ] **Step 4: Commit durable docs and contract changes**

```bash
git add tests/test_herbin_branding.py docs/rdh-upgrade-runbook.md
git commit -m "docs: update RDH window targeting contract"
```

- [ ] **Step 5: Push the candidate branch and dispatch CI**

Push:

```bash
git push fork rdh/1.4.9
```

Dispatch:

```bash
gh workflow run codex-macos-herbin.yml \
  --repo Herbin-s/rustdesk \
  --ref master \
  -f source_ref=rdh/1.4.9 \
  -f rdh_revision=9
```

Record the run ID from:

```bash
gh run list \
  --repo Herbin-s/rustdesk \
  --workflow codex-macos-herbin.yml \
  --branch master \
  --limit 5
```

Wait for terminal status and inspect:

```bash
gh run view "$RUN_ID" \
  --repo Herbin-s/rustdesk \
  --json status,conclusion,url
```

Expected: `status=completed` and `conclusion=success`. The dispatch runs from
`master`, so its workflow `headSha` is not source-commit evidence for
`source_ref`; verify that only from the downloaded build metadata below.

- [ ] **Step 6: Download and verify the artifact without installation**

```bash
ARTIFACT_DIR="$HOME/Library/Caches/RustDesk-Herbin/rdh.9-$RUN_ID"
mkdir -p "$ARTIFACT_DIR"
gh run download "$RUN_ID" \
  --repo Herbin-s/rustdesk \
  --dir "$ARTIFACT_DIR"
find "$ARTIFACT_DIR" -type f -print
```

Resolve the unique metadata, checksum, and DMG paths, then verify the checksum
from its own directory:

```bash
METADATA="$(find "$ARTIFACT_DIR" -name rdh-build-metadata.txt -type f -print -quit)"
CHECKSUM="$(find "$ARTIFACT_DIR" -name '*.sha256' -type f -print -quit)"
DMG="$(find "$ARTIFACT_DIR" -name '*.dmg' -type f -print -quit)"
CANDIDATE_COMMIT="$(git rev-parse rdh/1.4.9)"
test -n "$METADATA" && test -n "$CHECKSUM" && test -n "$DMG"
grep -Fx "source_commit=$CANDIDATE_COMMIT" "$METADATA"
(cd "$(dirname "$CHECKSUM")" && shasum -a 256 -c "$(basename "$CHECKSUM")")
```

Mount at a private temporary mount point, inspect, and always detach:

```bash
MOUNT_DIR="$(mktemp -d)"
hdiutil attach -readonly -nobrowse -mountpoint "$MOUNT_DIR" "$DMG"
codesign --verify --deep --strict --verbose=4 "$MOUNT_DIR/RustDesk-Herbin.app"
file "$MOUNT_DIR/RustDesk-Herbin.app/Contents/MacOS/RustDesk-Herbin"
defaults read "$MOUNT_DIR/RustDesk-Herbin.app/Contents/Info" CFBundleIdentifier
hdiutil detach "$MOUNT_DIR"
rmdir "$MOUNT_DIR"
```

Expected:

- checksum matches;
- metadata `source_commit` equals `git rev-parse rdh/1.4.9`;
- executable is arm64;
- bundle ID is `com.herbin.rustdesk`;
- ad-hoc signature verification passes.

Append run URL, source commit, checksum, architecture, bundle ID, and signature
result to the unstaged implementation notes.

---

### Task 8: Transactional Installation, CLI A/B, and Live Remote Acceptance

**Files:**
- Runtime config: `~/Library/Application Support/RustDesk-Herbin/window-targeting.toml`
- Installed app: `/Applications/RustDesk-Herbin.app`
- LaunchAgent: `/Library/LaunchAgents/com.herbin.RustDesk-Herbin_server.plist`
- Maintain unstaged: `implementation-notes.md`

**Interfaces:**
- Consumes: verified rdh.9 CI artifact and all CLI operations.
- Produces: user-confirmed live acceptance or a verified rollback to the prior RDH application.

- [ ] **Step 1: Establish both rescue and rollback boundaries**

Before replacing RDH:

1. Ask the user to switch the active working connection to official RustDesk.
2. Read back official and RDH service/server PIDs and exact executable paths.
3. Confirm official RustDesk can reconnect independently.
4. Record current RDH bundle ID, executable architecture, signature result,
   loaded dylib hash, LaunchAgent PID/runs/last-exit, and version.

Create one fresh bounded rollback copy:

```bash
ROLLBACK_DIR="$HOME/Library/Caches/RustDesk-Herbin/rollback-before-rdh.9-$RUN_ID"
mkdir -p "$ROLLBACK_DIR"
ditto "/Applications/RustDesk-Herbin.app" "$ROLLBACK_DIR/RustDesk-Herbin.app"
codesign --verify --deep --strict --verbose=4 "$ROLLBACK_DIR/RustDesk-Herbin.app"
```

Do not proceed unless the rollback bundle verifies and official RustDesk remains
reachable.

- [ ] **Step 2: Install the verified app and restart only the RDH user server**

Reattach the verified DMG at a fresh private mount point, then define exact
identities:

```bash
MOUNT_DIR="$(mktemp -d)"
hdiutil attach -readonly -nobrowse -mountpoint "$MOUNT_DIR" "$DMG"
APP="/Applications/RustDesk-Herbin.app"
STAGED="/Applications/.RustDesk-Herbin.rdh9-$RUN_ID-staged.app"
OLD_APP="/Applications/.RustDesk-Herbin.pre-rdh9-$RUN_ID.app"
PLIST="/Library/LaunchAgents/com.herbin.RustDesk-Herbin_server.plist"
LABEL="com.herbin.RustDesk-Herbin_server"
ACTIVE_UID="$(stat -f %u /dev/console)"
SERVICE_TARGET="gui/$ACTIVE_UID/$LABEL"
test "$ACTIVE_UID" -gt 0
test -d "$APP"
test -f "$PLIST"
test ! -e "$STAGED"
test ! -e "$OLD_APP"
launchctl print "$SERVICE_TARGET"
```

Stage and verify without changing the running installation:

```bash
sudo ditto "$MOUNT_DIR/RustDesk-Herbin.app" "$STAGED"
sudo codesign --verify --deep --strict --verbose=4 "$STAGED"
CANDIDATE_EXE_SHA="$(
  shasum -a 256 "$STAGED/Contents/MacOS/RustDesk-Herbin" | awk '{print $1}'
)"
test -n "$CANDIDATE_EXE_SHA"
OLD_RDH_PID="$(
  launchctl print "$SERVICE_TARGET" |
    awk '/pid = / { print $3; exit }'
)"
test -n "$OLD_RDH_PID"
hdiutil detach "$MOUNT_DIR"
rmdir "$MOUNT_DIR"
```

Perform only same-volume renames around the RDH user LaunchAgent:

```bash
launchctl bootout "$SERVICE_TARGET"
sudo mv "$APP" "$OLD_APP"
sudo mv "$STAGED" "$APP"
launchctl bootstrap "gui/$ACTIVE_UID" "$PLIST"
launchctl kickstart -k "$SERVICE_TARGET"
```

If any command after `bootout` fails, stop immediately and run this recovery
sequence through the still-active official RustDesk connection:

```bash
launchctl bootout "$SERVICE_TARGET" 2>/dev/null || :
if test -e "$APP"; then
  sudo rm -rf "$APP"
fi
if test -e "$OLD_APP"; then
  sudo mv "$OLD_APP" "$APP"
else
  sudo ditto "$ROLLBACK_DIR/RustDesk-Herbin.app" "$APP"
fi
sudo codesign --verify --deep --strict --verbose=4 "$APP"
launchctl bootstrap "gui/$ACTIVE_UID" "$PLIST"
launchctl kickstart -k "$SERVICE_TARGET"
launchctl print "$SERVICE_TARGET"
```

Before the `rm -rf` line, print and verify that `APP` is exactly
`/Applications/RustDesk-Herbin.app`; never apply it to a computed or empty path.
The recovery sequence is the only authorized destructive removal in this
installation step.

After a successful kickstart, poll `launchctl print "$SERVICE_TARGET"` for at
most 30 seconds, then verify:

- LaunchAgent state is running;
- PID differs from the pre-install RDH server;
- executable resolves inside `/Applications/RustDesk-Herbin.app`;
- the installed executable SHA-256 equals `$CANDIDATE_EXE_SHA`;
- bundle version is `1.4.9` and the artifact metadata says `rdh_revision=9`;
- official RustDesk PIDs remain unchanged.

Only after all checks pass, verify
`OLD_APP=/Applications/.RustDesk-Herbin.pre-rdh9-$RUN_ID.app` and remove that
exact sibling with `sudo rm -rf "$OLD_APP"`; keep the verified
`$ROLLBACK_DIR/RustDesk-Herbin.app` until the next stable RDH cycle. Do not
bootout, bootstrap, kickstart, kill, or otherwise touch
`/Library/LaunchDaemons/com.herbin.RustDesk-Herbin_service.plist`, any official
RustDesk process, or any official RustDesk launchd label.

- [ ] **Step 3: Verify CLI state and safe failure before behavioral tests**

Run with the installed executable:

```bash
"/Applications/RustDesk-Herbin.app/Contents/MacOS/RustDesk-Herbin" \
  --window-targeting status
"/Applications/RustDesk-Herbin.app/Contents/MacOS/RustDesk-Herbin" \
  --window-targeting validate
```

Expected: both exit zero and report generation 1 in `rules` mode.

Temporarily introduce an invalid value in a separate staged copy of the config,
replace the live file only for the invalid reload test, run reload, and verify:

- CLI exits non-zero;
- output reports `unchanged=true`;
- generation, hash, PID, and connection remain unchanged.

Restore the valid file and validate it before continuing.

- [ ] **Step 4: Run deterministic passthrough baseline**

Set:

```toml
version = 1
mode = "passthrough"
diagnostics = true
```

Run `validate`, `reload`, and `status`. Record generation/hash.

From the remote controller:

- reproduce the upstream cross-application focus failure;
- confirm Finder and Dock transient menus receive raw clicks;
- confirm the same RDH connection remains active.

This pass establishes the baseline; do not treat the expected focus failure as a
candidate failure.

- [ ] **Step 5: Run rules-mode acceptance**

Set:

```toml
version = 1
mode = "rules"
diagnostics = true
```

Run `validate`, `reload`, and `status`. Verify the PID is unchanged and record
generation/hash.

Test remotely:

1. ChatGPT to X and X to ChatGPT.
2. Alternating two ChatGPT windows.
3. Finder path-bar **Copy as Pathname**.
4. Finder preview/display-options popover.
5. Dock **Quit** using a disposable test application.
6. Notification Center overlay over another application.
7. Repeated clicks in an already focused window.
8. Local mouse interaction.
9. Disconnect and reconnect once.

Expected: all menu/popover actions complete, intended application/window order
changes, no lower window activates before transient UI, and notification overlay
does not block the intended target.

- [ ] **Step 6: Measure the diagnostics-off core path**

Temporarily keep `diagnostics = true`, validate, and reload. Perform at least
200 focused remote clicks and collect the emitted `elapsed_micros` values.
Because Task 5 stops this timer before formatting/emitting diagnostics and
constructs the same decision/outcome regardless of the flag, these samples
measure the core preprocessing path that runs when diagnostics are off while
excluding log overhead. Calculate median, p95, and maximum from those values.

Expected: p95 is at or below 10 ms. Do not claim the performance contract from
unit matcher timing alone. After recording the result, set
`diagnostics = false`, validate, reload, and confirm final status reports
`diagnostics=false`.

- [ ] **Step 7: Promote or roll back**

If every acceptance item passes:

- keep `mode = "rules"` and `diagnostics = false`;
- validate and reload once;
- record final status, PID, connection state, and user confirmation;
- retain the rollback bundle until the next stable RDH cycle.

If any input, focus, CLI, IPC, service-health, or latency acceptance item fails:

1. switch to `mode = "passthrough"` and reload if CLI/input remains healthy;
2. remain connected through official RustDesk;
3. restore the verified rollback app;
4. kickstart only `com.herbin.RustDesk-Herbin_server`;
5. verify the old RDH PID, executable, signature, loaded dylib, and connection;
6. keep the failed artifact and diagnostics for root-cause analysis.

Append the exact result and any design deviation to the unstaged
`implementation-notes.md`.
