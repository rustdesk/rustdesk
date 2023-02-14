use serde_json::{json, value::Value};
use std::ops::Deref;

mod cn;
mod cs;
mod da;
mod de;
mod en;
mod eo;
mod es;
mod fr;
mod hu;
mod id;
mod it;
mod ja;
mod ko;
mod nl;
mod pl;
mod ptbr;
mod ro;
mod ru;
mod sk;
mod tr;
mod tw;
mod vn;
mod kz;
mod ua;
mod fa;
mod ca;
mod gr;
mod sv;
mod sq;
mod sr;
mod th;
mod sl;

lazy_static::lazy_static! {
    pub static ref LANGS: Value =
        json!(vec![
            ("en", "English"),
            ("it", "Italiano"),
            ("fr", "Français"),
            ("de", "Deutsch"),
            ("nl", "Nederlands"),
            ("cn", "简体中文"),
            ("tw", "繁體中文"),
            ("pt", "Português"),
            ("es", "Español"),
            ("hu", "Magyar"),
            ("ru", "Русский"),
            ("sk", "Slovenčina"),
            ("id", "Indonesia"),
            ("cs", "Čeština"),
            ("da", "Dansk"),
            ("eo", "Esperanto"),
            ("tr", "Türkçe"),
            ("vn", "Tiếng Việt"),
            ("pl", "Polski"),
            ("ja", "日本語"),
            ("ko", "한국어"),
            ("kz", "Қазақ"),
            ("ua", "Українська"),
            ("fa", "فارسی"),
            ("ca", "Català"),
            ("gr", "Ελληνικά"),
            ("sv", "Svenska"),
            ("sq", "Shqip"),
            ("sr", "Srpski"),
            ("th", "ภาษาไทย"),
            ("sl", "Slovenščina"),
            ("ro", "Română"),
        ]);
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn translate(name: String) -> String {
    let locale = sys_locale::get_locale().unwrap_or_default().to_lowercase();
    translate_locale(name, &locale)
}

pub fn translate_locale(name: String, locale: &str) -> String {
    let mut lang = hbb_common::config::LocalConfig::get_option("lang").to_lowercase();
    if lang.is_empty() {
        // zh_CN on Linux, zh-Hans-CN on mac, zh_CN_#Hans on Android
        if locale.starts_with("zh") {
            lang = (if locale.contains("TW") { "tw" } else { "cn" }).to_owned();
        }
    }
    if lang.is_empty() {
        lang = locale
            .split("-")
            .next()
            .map(|x| x.split("_").next().unwrap_or_default())
            .unwrap_or_default()
            .to_owned();
    }
    let lang = lang.to_lowercase();
    let m = match lang.as_str() {
        "fr" => fr::T.deref(),
        "cn" => cn::T.deref(),
        "it" => it::T.deref(),
        "tw" => tw::T.deref(),
        "de" => de::T.deref(),
        "nl" => nl::T.deref(),
        "es" => es::T.deref(),
        "hu" => hu::T.deref(),
        "ru" => ru::T.deref(),
        "eo" => eo::T.deref(),
        "id" => id::T.deref(),
        "ptbr" => ptbr::T.deref(),
        "br" => ptbr::T.deref(),
        "pt" => ptbr::T.deref(),
        "tr" => tr::T.deref(),
        "cs" => cs::T.deref(),
        "da" => da::T.deref(),
        "sk" => sk::T.deref(),
        "vn" => vn::T.deref(),
        "pl" => pl::T.deref(),
        "ja" => ja::T.deref(),
        "ko" => ko::T.deref(),
        "kz" => kz::T.deref(),
        "ua" => ua::T.deref(),
        "fa" => fa::T.deref(),
        "ca" => ca::T.deref(),
        "gr" => gr::T.deref(),
        "sv" => sv::T.deref(),
        "sq" => sq::T.deref(),
        "sr" => sr::T.deref(),
        "th" => th::T.deref(),
        "sl" => sl::T.deref(),
        "ro" => ro::T.deref(),
        _ => en::T.deref(),
    };
    if let Some(v) = m.get(&name as &str) {
        if v.is_empty() {
            if lang != "en" {
                if let Some(v) = en::T.get(&name as &str) {
                    return v.to_string();
                }
            }
        } else {
            return v.to_string();
        }
    }
    name
}
