use super::{
    ActivationPolicy, CompiledRule, RuleMatcher, ValidatedUserConfig, WindowTargetAction,
    WindowTargetingMode,
};
use serde_derive::Serialize;

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
#[allow(dead_code)]
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

pub(crate) fn compile_effective(user: Option<ValidatedUserConfig>) -> EffectiveConfig {
    let (mode, diagnostics, mut rules) = match user {
        Some(user) => (user.mode, user.diagnostics, user.rules),
        None => (WindowTargetingMode::Rules, false, Vec::new()),
    };
    rules.extend(built_in_rules());
    EffectiveConfig {
        mode,
        diagnostics,
        rules,
    }
}

pub(crate) fn decide(config: &EffectiveConfig, candidates: &[WindowCandidate]) -> WindowDecision {
    let mut trace = Vec::new();

    for (candidate_index, candidate) in candidates.iter().enumerate() {
        let mut skipped = false;
        for rule in &config.rules {
            if !rule.matcher.matches(candidate) {
                continue;
            }

            if rule.action == WindowTargetAction::Skip {
                trace.push(DecisionStep {
                    candidate_index,
                    rule_id: rule.id.clone(),
                    action: WindowTargetAction::Skip,
                });
                skipped = true;
                break;
            }

            return WindowDecision {
                action: rule.action,
                candidate_index: Some(candidate_index),
                rule_id: rule.id.clone(),
                trace,
            };
        }

        if skipped {
            continue;
        }

        let action =
            if candidate.layer == 0 && candidate.activation_policy == ActivationPolicy::Regular {
                WindowTargetAction::Activate
            } else {
                WindowTargetAction::ForwardOnly
            };
        let rule_id = if action == WindowTargetAction::Activate {
            "default.regular-layer-zero"
        } else {
            "default.conservative"
        };
        return WindowDecision {
            action,
            candidate_index: Some(candidate_index),
            rule_id: rule_id.to_owned(),
            trace,
        };
    }

    WindowDecision {
        action: WindowTargetAction::ForwardOnly,
        candidate_index: None,
        rule_id: "default.no-target".to_owned(),
        trace,
    }
}

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
            "builtin.dock-transparent-cover",
            WindowTargetAction::Skip,
            RuleMatcher {
                bundle_id_prefixes: vec!["com.apple.dock".to_owned()],
                covers_display: Some(true),
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
                bundle_id_prefixes: vec!["com.apple.notificationcenterui".to_owned()],
                layer_min: Some(1),
                covers_display: Some(true),
                ..RuleMatcher::default()
            },
        ),
    ]
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
    fn dock_transparent_cover_skips_to_regular_window() {
        let config = compile_effective(None);
        let candidates = vec![
            candidate(
                64334,
                "com.apple.dock",
                20,
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
        assert_eq!(
            (
                decision.action,
                decision.candidate_index,
                decision.rule_id.as_str(),
            ),
            (
                WindowTargetAction::Activate,
                Some(1),
                "default.regular-layer-zero",
            )
        );
        assert_eq!(
            decision.trace,
            vec![DecisionStep {
                candidate_index: 0,
                rule_id: "builtin.dock-transparent-cover".to_owned(),
                action: WindowTargetAction::Skip,
            }]
        );
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

    #[test]
    fn matcher_requires_all_populated_fields() {
        let matcher = RuleMatcher {
            bundle_ids: vec!["com.example.target".to_owned()],
            activation_policies: vec![ActivationPolicy::Regular],
            ..RuleMatcher::default()
        };
        assert!(matcher.matches(&candidate(
            7,
            "com.example.target",
            0,
            ActivationPolicy::Regular,
            Some("AXWindow"),
            false,
        )));
        assert!(!matcher.matches(&candidate(
            7,
            "com.example.target",
            0,
            ActivationPolicy::Accessory,
            Some("AXWindow"),
            false,
        )));
    }

    #[test]
    fn matcher_allows_any_value_within_a_field() {
        let matcher = RuleMatcher {
            bundle_ids: vec![
                "com.example.first".to_owned(),
                "com.example.second".to_owned(),
            ],
            ..RuleMatcher::default()
        };
        assert!(matcher.matches(&candidate(
            7,
            "com.example.second",
            0,
            ActivationPolicy::Regular,
            Some("AXWindow"),
            false,
        )));
        assert!(!matcher.matches(&candidate(
            7,
            "com.example.other",
            0,
            ActivationPolicy::Regular,
            Some("AXWindow"),
            false,
        )));
    }
}
