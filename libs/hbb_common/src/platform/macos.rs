use osascript;
use serde_derive;

#[derive(Serialize)]
struct AlertParams {
    title: String,
    message: String,
    alert_type: String,
    buttons: Vec<String>,
}

#[derive(Deserialize)]
struct AlertResult {
    #[serde(rename = "buttonReturned")]
    button: String,
}

/// Alert dialog, return the clicked button value.
///
/// # Arguments
///
/// * `app` - The app to execute the script.
/// * `alert_type` - Alert type. critical
/// * `title` - The alert title.
/// * `message` - The alert message.
/// * `buttons` - The buttons to show.
pub fn alert(
    app: &str,
    alert_type: &str,
    title: &str,
    message: String,
    buttons: Vec<String>,
) -> ResultType<String> {
    let script = osascript::JavaScript::new(format!(
        "
    var App = Application('{}');
    App.includeStandardAdditions = true;
    return App.displayAlert($params.title, {
        message: $params.message,
        'as': $params.alert_type,
        buttons: $params.buttons,
    });
    ",
        app
    ));

    script
        .execute_with_params(AlertParams {
            title,
            message,
            alert_type,
            buttons,
        })?
        .button
}
