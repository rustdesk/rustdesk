import os
import re

def main():
    cfg_path = os.path.join("libs", "hbb_common", "src", "config.rs")
    if not os.path.exists(cfg_path):
        print(f"[CI] Warning: {cfg_path} not found")
        return

    with open(cfg_path, "r", encoding="utf-8") as f:
        text = f.read()

    # App name
    text = re.sub(
        r'pub static ref APP_NAME: RwLock<String> = RwLock::new\(".*?"\.to_owned\(\)\);',
        'pub static ref APP_NAME: RwLock<String> = RwLock::new("YuleLink".to_owned());',
        text
    )

    # Server
    text = re.sub(
        r'pub const RENDEZVOUS_SERVERS: &\[&str\] = &\[".*?"\];',
        'pub const RENDEZVOUS_SERVERS: &[&str] = &["x9ragj8s4ydxv89f7m-rustdesk.yulelink.com"];',
        text
    )

    # Key
    text = re.sub(
        r'pub const RS_PUB_KEY: &str = ".*?";',
        'pub const RS_PUB_KEY: &str = "s2J6JDCuo1yTevpB2unpJwYdhIYnd9Qq79dEogH7cGQ=";',
        text
    )

    # Relay server
    if 'pub const RELAY_SERVER:' in text:
        text = re.sub(
            r'pub const RELAY_SERVER: &str = ".*?";',
            'pub const RELAY_SERVER: &str = "";',
            text
        )
    else:
        text = text.replace(
            'pub const RS_PUB_KEY:',
            'pub const RELAY_SERVER: &str = "";\npub const RS_PUB_KEY:'
        )

    # API server
    if 'pub const API_SERVER:' in text:
        text = re.sub(
            r'pub const API_SERVER: &str = ".*?";',
            'pub const API_SERVER: &str = "";',
            text
        )
    else:
        text = text.replace(
            'pub const RS_PUB_KEY:',
            'pub const API_SERVER: &str = "";\npub const RS_PUB_KEY:'
        )

    # Ensure get_options and get_option have fallback logic
    if '!res.contains_key("custom-rendezvous-server")' not in text:
        old_get_opts = """    pub fn get_options() -> HashMap<String, String> {
        let mut res = DEFAULT_SETTINGS.read().unwrap().clone();
        res.extend(CONFIG2.read().unwrap().options.clone());
        res.extend(OVERWRITE_SETTINGS.read().unwrap().clone());
        res
    }"""
        new_get_opts = """    pub fn get_options() -> HashMap<String, String> {
        let mut res = DEFAULT_SETTINGS.read().unwrap().clone();
        res.extend(CONFIG2.read().unwrap().options.clone());
        res.extend(OVERWRITE_SETTINGS.read().unwrap().clone());
        if !res.contains_key("custom-rendezvous-server") || res["custom-rendezvous-server"].is_empty() {
            if let Some(first) = RENDEZVOUS_SERVERS.first() {
                if !first.is_empty() {
                    res.insert("custom-rendezvous-server".to_string(), first.to_string());
                }
            }
        }
        if !res.contains_key("key") || res["key"].is_empty() {
            if !RS_PUB_KEY.is_empty() {
                res.insert("key".to_string(), RS_PUB_KEY.to_string());
            }
        }
        if !res.contains_key("relay-server") || res["relay-server"].is_empty() {
            if !RELAY_SERVER.is_empty() {
                res.insert("relay-server".to_string(), RELAY_SERVER.to_string());
            }
        }
        if !res.contains_key("api-server") || res["api-server"].is_empty() {
            if !API_SERVER.is_empty() {
                res.insert("api-server".to_string(), API_SERVER.to_string());
            }
        }
        res
    }"""
        text = text.replace(old_get_opts, new_get_opts)

    if 'match k {' not in text:
        old_get_opt = """    pub fn get_option(k: &str) -> String {
        get_or(
            &OVERWRITE_SETTINGS,
            &CONFIG2.read().unwrap().options,
            &DEFAULT_SETTINGS,
            k,
        )
        .unwrap_or_default()
    }"""
        new_get_opt = """    pub fn get_option(k: &str) -> String {
        let v = get_or(
            &OVERWRITE_SETTINGS,
            &CONFIG2.read().unwrap().options,
            &DEFAULT_SETTINGS,
            k,
        )
        .unwrap_or_default();
        if v.is_empty() {
            match k {
                "custom-rendezvous-server" => {
                    RENDEZVOUS_SERVERS.first().map(|s| s.to_string()).unwrap_or_default()
                }
                "key" => RS_PUB_KEY.to_string(),
                "relay-server" => RELAY_SERVER.to_string(),
                "api-server" => API_SERVER.to_string(),
                _ => v,
            }
        } else {
            v
        }
    }"""
        text = text.replace(old_get_opt, new_get_opt)

    with open(cfg_path, "w", encoding="utf-8") as f:
        f.write(text)
    print("[CI] Custom server and branding successfully injected!")

if __name__ == "__main__":
    main()
