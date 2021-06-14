//! The most simplest examples of how to use confy

extern crate confy;

#[macro_use]
extern crate serde_derive;

#[derive(Debug, Serialize, Deserialize)]
struct ConfyConfig {
    name: String,
    comfy: bool,
    foo: i64,
}

impl Default for ConfyConfig {
    fn default() -> Self {
        ConfyConfig {
            name: "Unknown".to_string(),
            comfy: true,
            foo: 42,
        }
    }
}

fn main() -> Result<(), confy::ConfyError> {
    let cfg: ConfyConfig = confy::load("confy_simple_app")?;
    println!("{:#?}", cfg);
    Ok(())
}
