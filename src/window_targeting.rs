#[cfg(target_os = "macos")]
mod config;
#[cfg(target_os = "macos")]
mod rules;

#[allow(unused_imports)]
pub(crate) use config::{
    ActivationPolicy, CompiledRule, ConfigError, RuleMatcher, ValidatedUserConfig,
    WindowTargetAction, WindowTargetingMode,
};
#[allow(unused_imports)]
pub(crate) use rules::{DecisionStep, EffectiveConfig, WindowCandidate, WindowDecision};
