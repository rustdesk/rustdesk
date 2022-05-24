use std::ops::Deref;

mod cn;
mod en;
mod fr;
mod it;
mod tw;
mod de;
mod ru;
mod eo;
mod ptbr;
mod id;
mod tr;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn translate(name: String) -> String {
    let locale = sys_locale::get_locale().unwrap_or_default().to_lowercase();
    translate_locale(name, &locale)
}

pub fn translate_locale(name: String, locale: &str) -> String {
    let mut lang = hbb_common::config::LocalConfig::get_option("lang").to_lowercase();
    if lang.is_empty() {
        // zh_CN on Linux, zh-Hans-CN on mac, zh_CN_#Hans on Android
        if locale.starts_with("zh") && (locale.ends_with("CN") || locale.ends_with("SG") || locale.ends_with("Hans")) {
            lang = "cn".to_owned();
        }
    }
    if lang.is_empty() {
        lang = locale
            .split("-")
            .last()
            .map(|x| x.split("_").last().unwrap_or_default())
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
        "ru" => ru::T.deref(),
        "eo" => eo::T.deref(),
        "id" => id::T.deref(),
        "ptbr" => ptbr::T.deref(),
        "br" => ptbr::T.deref(),
        "pt" => ptbr::T.deref(),
        "tr" => tr::T.deref(),
        _ => en::T.deref(),
    };
    if let Some(v) = m.get(&name as &str) {
        v.to_string()
    } else {
        if lang != "en" {
            if let Some(v) = en::T.get(&name as &str) {
                return v.to_string();
            }
        }
        name
    }
}
