use hbb_common::{config::Config, log};
use std::ops::Deref;

mod cn;
mod en;
mod fr;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn translate(name: String) -> String {
    let locale = sys_locale::get_locale().unwrap_or_default().to_lowercase();
    log::debug!("The current locale is {}", locale);
    translate_locale(name, &locale)
}

pub fn translate_locale(name: String, locale: &str) -> String {
    let mut lang = Config::get_option("lang");
    if lang.is_empty() {
        lang = locale
            .split("-")
            .last()
            .map(|x| x.split("_").last().unwrap_or_default())
            .unwrap_or_default()
            .to_owned();
    }
    let m = match lang.to_lowercase().as_str() {
        "fr" => fr::T.deref(),
        "cn" => cn::T.deref(),
        _ => en::T.deref(),
    };
    if let Some(v) = m.get(&name as &str) {
        v.to_string()
    } else {
        name
    }
}
