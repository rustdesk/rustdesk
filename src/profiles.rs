use base::config::keys::{
    OPTION_API_SERVER, OPTION_CUSTOM_RENDEZVOUS_SERVER, OPTION_KEY, OPTION_RELAY_SERVER,
};
use hbb_common::{anyhow::anyhow, config::Config, ResultType};
use std::{collections::BTreeMap, fs};

const PROFILE_KEYS: [&str; 4] = [
    OPTION_CUSTOM_RENDEZVOUS_SERVER,
    OPTION_RELAY_SERVER,
    OPTION_API_SERVER,
    OPTION_KEY,
];

type Profiles = BTreeMap<String, BTreeMap<String, String>>;

fn load() -> Profiles {
    fs::read_to_string(Config::path("profiles.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn store(profiles: &Profiles) -> ResultType<()> {
    let path = Config::path("profiles.json");
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, serde_json::to_string_pretty(profiles)?)?;
    Ok(())
}

fn snapshot(options: &std::collections::HashMap<String, String>) -> BTreeMap<String, String> {
    PROFILE_KEYS
        .iter()
        .filter_map(|k| options.get(*k).map(|v| (k.to_string(), v.clone())))
        .collect()
}

// Keys missing from the profile are removed, so switching to a profile without a
// relay server really clears the relay server.
fn apply_to(
    options: &mut std::collections::HashMap<String, String>,
    profile: &BTreeMap<String, String>,
) {
    for k in PROFILE_KEYS {
        match profile.get(k).filter(|v| !v.is_empty()) {
            Some(v) => {
                options.insert(k.to_owned(), v.clone());
            }
            None => {
                options.remove(k);
            }
        }
    }
}

fn run(args: &[String]) -> ResultType<String> {
    let mut profiles = load();
    match (args.first().map(String::as_str), args.get(1)) {
        (Some("list"), _) => Ok(profiles.keys().cloned().collect::<Vec<_>>().join("\n")),
        (Some("show"), Some(name)) => {
            let p = profiles
                .get(name)
                .ok_or_else(|| anyhow!("No such profile: {name}"))?;
            Ok(serde_json::to_string_pretty(p)?)
        }
        (Some("save"), Some(name)) => {
            profiles.insert(name.clone(), snapshot(&crate::ipc::get_options()));
            store(&profiles)?;
            Ok(format!("Saved profile {name}"))
        }
        (Some("use"), Some(name)) => {
            let p = profiles
                .get(name)
                .ok_or_else(|| anyhow!("No such profile: {name}"))?;
            let mut options = crate::ipc::get_options();
            apply_to(&mut options, p);
            crate::ipc::set_options(options)?;
            Ok(format!("Switched to profile {name}"))
        }
        (Some("delete"), Some(name)) => {
            if profiles.remove(name).is_none() {
                return Err(anyhow!("No such profile: {name}"));
            }
            store(&profiles)?;
            Ok(format!("Deleted profile {name}"))
        }
        _ => Ok("Usage: --profile list | show <name> | save <name> | use <name> | delete <name>"
            .to_owned()),
    }
}

pub fn cli(args: &[String]) {
    match run(args) {
        Ok(msg) => println!("{msg}"),
        Err(err) => println!("{err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn switching_profile_clears_keys_it_does_not_set() {
        let mut options = HashMap::from([
            (OPTION_RELAY_SERVER.to_owned(), "relay.a".to_owned()),
            (OPTION_CUSTOM_RENDEZVOUS_SERVER.to_owned(), "id.a".to_owned()),
            ("theme".to_owned(), "dark".to_owned()),
        ]);
        let profile = BTreeMap::from([(OPTION_CUSTOM_RENDEZVOUS_SERVER.to_owned(), "id.b".to_owned())]);
        apply_to(&mut options, &profile);
        assert_eq!(options.get(OPTION_CUSTOM_RENDEZVOUS_SERVER).map(String::as_str), Some("id.b"));
        assert!(!options.contains_key(OPTION_RELAY_SERVER));
        assert_eq!(options.get("theme").map(String::as_str), Some("dark"));
    }
}
