use hbb_common::config;

mod generated {
    include!("white_label_generated.rs");
}

pub fn apply() {
    if !generated::APP_NAME.is_empty() {
        *config::APP_NAME.write().unwrap() = generated::APP_NAME.to_owned();
    }
    config::DEFAULT_SETTINGS
        .write()
        .unwrap()
        .extend(to_owned_settings(generated::DEFAULT_SETTINGS));
    config::OVERWRITE_SETTINGS
        .write()
        .unwrap()
        .extend(to_owned_settings(generated::OVERRIDE_SETTINGS));
}

fn to_owned_settings<'a>(
    settings: &'a [(&'a str, &'a str)],
) -> impl Iterator<Item = (String, String)> + 'a {
    settings
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
}

pub fn update_check_url() -> &'static str {
    generated::UPDATE_CHECK_URL
}

pub fn windows_download_url_template() -> &'static str {
    generated::WINDOWS_DOWNLOAD_URL_TEMPLATE
}
