//! Zero-boilerplate configuration management
//!
//! ## Why?
//!
//! There are a lot of different requirements when
//! selecting, loading and writing a config,
//! depending on the operating system and other
//! environment factors.
//!
//! In many applications this burden is left to you,
//! the developer of an application, to figure out
//! where to place the configuration files.
//!
//! This is where `confy` comes in.
//!
//! ## Idea
//!
//! `confy` takes care of figuring out operating system
//! specific and environment paths before reading and
//! writing a configuration.
//!
//! It gives you easy access to a configuration file
//! which is mirrored into a Rust `struct` via [serde].
//! This way you only need to worry about the layout of
//! your configuration, not where and how to store it.
//!
//! [serde]: https://docs.rs/crates/serde
//!
//! `confy` uses the [`Default`] trait in Rust to automatically
//! create a new configuration, if none is available to read
//! from yet.
//! This means that you can simply assume your application
//! to have a configuration, which will be created with
//! default values of your choosing, without requiring
//! any special logic to handle creation.
//!
//! [`Default`]: https://doc.rust-lang.org/std/default/trait.Default.html
//!
//! ```rust,no_run
//! use serde_derive::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize)]
//! struct MyConfig {
//!     version: u8,
//!     api_key: String,
//! }
//!
//! /// `MyConfig` implements `Default`
//! impl ::std::default::Default for MyConfig {
//!     fn default() -> Self { Self { version: 0, api_key: "".into() } }
//! }
//!
//! fn main() -> Result<(), confy::ConfyError> {
//!     let cfg = confy::load("my-app-name")?;
//!     Ok(())
//! }
//! ```
//!
//! Updating the configuration is then done via the [`store`] function.
//!
//! [`store`]: fn.store.html
//!

mod utils;
use utils::*;

use directories::ProjectDirs;
use serde::{de::DeserializeOwned, Serialize};
use std::error::Error;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind::NotFound, Write};
use std::path::{Path, PathBuf};

#[cfg(not(any(feature = "toml_conf", feature = "yaml_conf")))]
compile_error!("Exactly one config language feature must be enabled to use \
confy.  Please enable one of either the `toml_conf` or `yaml_conf` \
features.");

#[cfg(all(feature = "toml_conf", feature = "yaml_conf"))]
compile_error!("Exactly one config language feature must be enabled to compile \
confy.  Please disable one of either the `toml_conf` or `yaml_conf` features. \
NOTE: `toml_conf` is a default feature, so disabling it might mean switching off \
default features for confy in your Cargo.toml");

#[cfg(all(feature = "toml_conf", not(feature = "yaml_conf")))]
const EXTENSION: &str = "toml";

#[cfg(feature = "yaml_conf")]
const EXTENSION: &str = "yml";

/// Load an application configuration from disk
///
/// A new configuration file is created with default values if none
/// exists.
///
/// Errors that are returned from this function are I/O related,
/// for example if the writing of the new configuration fails
/// or `confy` encounters an operating system or environment
/// that it does not support.
///
/// **Note:** The type of configuration needs to be declared in some way
/// that is inferrable by the compiler. Also note that your
/// configuration needs to implement `Default`.
///
/// ```rust,no_run
/// # use confy::ConfyError;
/// # use serde_derive::{Serialize, Deserialize};
/// # fn main() -> Result<(), ConfyError> {
/// #[derive(Default, Serialize, Deserialize)]
/// struct MyConfig {}
///
/// let cfg: MyConfig = confy::load("my-app-name")?;
/// # Ok(())
/// # }
/// ```
pub fn load<T: Serialize + DeserializeOwned + Default>(name: &str) -> Result<T, ConfyError> {
    let project = ProjectDirs::from("rs", "", name).ok_or(ConfyError::BadConfigDirectoryStr)?;

    let config_dir_str = get_configuration_directory_str(&project)?;

    let path: PathBuf = [config_dir_str, &format!("{}.{}", name, EXTENSION)].iter().collect();

    load_path(path)
}

/// Load an application configuration from a specified path.
///
/// This is an alternate version of [`load`] that allows the specification of
/// an aritrary path instead of a system one.  For more information on errors
/// and behavior, see [`load`]'s documentation.
///
/// [`load`]: fn.load.html
pub fn load_path<T: Serialize + DeserializeOwned + Default>(path: impl AsRef<Path>) -> Result<T, ConfyError> {
    match File::open(&path) {
        Ok(mut cfg) => {
            let cfg_string = cfg
                .get_string()
                .map_err(ConfyError::ReadConfigurationFileError)?;

            #[cfg(feature = "toml_conf")] {
                let cfg_data = toml::from_str(&cfg_string);
                cfg_data.map_err(ConfyError::BadTomlData)
            }
            #[cfg(feature = "yaml_conf")] {
                let cfg_data = serde_yaml::from_str(&cfg_string);
                cfg_data.map_err(ConfyError::BadYamlData)
            }

        }
        Err(ref e) if e.kind() == NotFound => {
            if let Some(parent) = path.as_ref().parent() {
                fs::create_dir_all(parent)
                    .map_err(ConfyError::DirectoryCreationFailed)?;
            }
            store_path(path, T::default())?;
            Ok(T::default())
        }
        Err(e) => Err(ConfyError::GeneralLoadError(e)),
    }
}

/// The errors the confy crate can encounter.
#[derive(Debug)]
pub enum ConfyError {
    #[cfg(feature = "toml_conf")]
    BadTomlData(toml::de::Error),

    #[cfg(feature = "yaml_conf")]
    BadYamlData(serde_yaml::Error),

    DirectoryCreationFailed(std::io::Error),
    GeneralLoadError(std::io::Error),
    BadConfigDirectoryStr,

    #[cfg(feature = "toml_conf")]
    SerializeTomlError(toml::ser::Error),

    #[cfg(feature = "yaml_conf")]
    SerializeYamlError(serde_yaml::Error),

    WriteConfigurationFileError(std::io::Error),
    ReadConfigurationFileError(std::io::Error),
    OpenConfigurationFileError(std::io::Error),
}

impl fmt::Display for ConfyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {

            #[cfg(feature = "toml_conf")]
            ConfyError::BadTomlData(e) => write!(f, "Bad TOML data: {}", e),
            #[cfg(feature = "toml_conf")]
            ConfyError::SerializeTomlError(_) => write!(f, "Failed to serialize configuration data into TOML."),

            #[cfg(feature = "yaml_conf")]
            ConfyError::BadYamlData(e) => write!(f, "Bad YAML data: {}", e),
            #[cfg(feature = "yaml_conf")]
            ConfyError::SerializeYamlError(_) => write!(f, "Failed to serialize configuration data into YAML."),

            ConfyError::DirectoryCreationFailed(e) => write!(f, "Failed to create directory: {}", e),
            ConfyError::GeneralLoadError(_) => write!(f, "Failed to load configuration file."),
            ConfyError::BadConfigDirectoryStr => write!(f, "Failed to convert directory name to str."),
            ConfyError::WriteConfigurationFileError(_) => write!(f, "Failed to write configuration file."),
            ConfyError::ReadConfigurationFileError(_) => write!(f, "Failed to read configuration file."),
            ConfyError::OpenConfigurationFileError(_) => write!(f, "Failed to open configuration file."),
        }
    }
}

impl Error for ConfyError {}

/// Save changes made to a configuration object
///
/// This function will update a configuration,
/// with the provided values, and create a new one,
/// if none exists.
///
/// You can also use this function to create a new configuration
/// with different initial values than which are provided
/// by your `Default` trait implementation, or if your
/// configuration structure _can't_ implement `Default`.
///
/// ```rust,no_run
/// # use serde_derive::{Serialize, Deserialize};
/// # use confy::ConfyError;
/// # fn main() -> Result<(), ConfyError> {
/// #[derive(Serialize, Deserialize)]
/// struct MyConf {}
///
/// let my_cfg = MyConf {};
/// confy::store("my-app-name", my_cfg)?;
/// # Ok(())
/// # }
/// ```
///
/// Errors returned are I/O errors related to not being
/// able to write the configuration file or if `confy`
/// encounters an operating system or environment it does
/// not support.
pub fn store<T: Serialize>(name: &str, cfg: T) -> Result<(), ConfyError> {
    let project = ProjectDirs::from("rs", "", name).ok_or(ConfyError::BadConfigDirectoryStr)?;
    fs::create_dir_all(project.config_dir()).map_err(ConfyError::DirectoryCreationFailed)?;

    let config_dir_str = get_configuration_directory_str(&project)?;

    let path: PathBuf = [config_dir_str, &format!("{}.{}", name, EXTENSION)].iter().collect();

    store_path(path, cfg)
}

/// Save changes made to a configuration object at a specified path
///
/// This is an alternate version of [`store`] that allows the specification of
/// an aritrary path instead of a system one.  For more information on errors
/// and behavior, see [`store`]'s documentation.
///
/// [`store`]: fn.store.html
pub fn store_path<T: Serialize>(path: impl AsRef<Path>, cfg: T) -> Result<(), ConfyError> {
    let path = path.as_ref();
    let mut path_tmp = path.to_path_buf();
    use std::time::{SystemTime, UNIX_EPOCH};
    let mut i = 0;
    loop {
        i += 1;
        path_tmp.set_extension(SystemTime::now().duration_since(UNIX_EPOCH).map(|x| x.as_nanos()).unwrap_or(i).to_string());
        if !path_tmp.exists() {
            break;
        }
    }
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path_tmp)
        .map_err(ConfyError::OpenConfigurationFileError)?;

    let s;
    #[cfg(feature = "toml_conf")] {
        s = toml::to_string(&cfg).map_err(ConfyError::SerializeTomlError)?;
    }
   #[cfg(feature = "yaml_conf")] {
        s = serde_yaml::to_string(&cfg).map_err(ConfyError::SerializeYamlError)?;
    }

    f.write_all(s.as_bytes())
        .map_err(ConfyError::WriteConfigurationFileError)?;
    std::fs::rename(path_tmp, path)
        .map_err(ConfyError::WriteConfigurationFileError)?;
    Ok(())
}

fn get_configuration_directory_str(project: &ProjectDirs) -> Result<&str, ConfyError> {
    let config_dir_option = project.config_dir().to_str();

    match config_dir_option {
        Some(x) => Ok(x),
        None => Err(ConfyError::BadConfigDirectoryStr),
    }
}
