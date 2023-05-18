use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hbb_common::{bail, sodiumoxide::crypto::sign, ResultType};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Default, Serialize, Deserialize, Clone)]
pub struct License {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub api: String,
}

fn get_license_from_string_(s: &str) -> ResultType<License> {
    let tmp: String = s.chars().rev().collect();
    const PK: &[u8; 32] = &[
        88, 168, 68, 104, 60, 5, 163, 198, 165, 38, 12, 85, 114, 203, 96, 163, 70, 48, 0, 131, 57,
        12, 46, 129, 83, 17, 84, 193, 119, 197, 130, 103,
    ];
    let pk = sign::PublicKey(*PK);
    let data = URL_SAFE_NO_PAD.decode(tmp)?;
    if let Ok(lic) = serde_json::from_slice::<License>(&data) {
        return Ok(lic);
    }
    if let Ok(data) = sign::verify(&data, &pk) {
        Ok(serde_json::from_slice::<License>(&data)?)
    } else {
        bail!("sign:verify failed");
    }
}

pub fn get_license_from_string(s: &str) -> ResultType<License> {
    let s = if s.to_lowercase().ends_with(".exe") {
        &s[0..s.len() - 4]
    } else {
        s
    };
    /*
     * The following code tokenizes the file name based on commas and
     * extracts relevant parts sequentially.
     *
     * host= is expected to be the first part.
     *
     * Since Windows renames files adding (1), (2) etc. before the .exe
     * in case of duplicates, which causes the host or key values to be
     * garbled.
     *
     * This allows using a ',' (comma) symbol as a final delimiter.
     */
    if s.contains("host=") {
        let stripped = &s[s.find("host=").unwrap_or(0)..s.len()];
        let strs: Vec<&str> = stripped.split(",").collect();
        let mut host = "";
        let mut key = "";
        let strs_iter = strs.iter();
        for el in strs_iter {
            if el.starts_with("host=") {
                host = &el[5..el.len()];
            }

            if el.starts_with("key=") {
                key = &el[4..el.len()];
            }
        }
        return Ok(License {
            host: host.to_owned(),
            key: key.to_owned(),
            api: "".to_owned(),
        });
    } else {
        let strs = if s.contains("-licensed-") {
            s.split("-licensed-")
        } else {
            s.split("--")
        };
        for s in strs {
            if let Ok(lic) = get_license_from_string_(s) {
                return Ok(lic);
            }
        }
    }
    bail!("Failed to parse");
}

#[cfg(test)]
#[cfg(target_os = "windows")]
mod test {
    use super::*;

    #[test]
    fn test_filename_license_string() {
        assert!(get_license_from_string("rustdesk.exe").is_err());
        assert!(get_license_from_string("rustdesk").is_err());
        assert_eq!(
            get_license_from_string("rustdesk-host=server.example.net.exe").unwrap(),
            License {
                host: "server.example.net".to_owned(),
                key: "".to_owned(),
                api: "".to_owned(),
            }
        );
        assert_eq!(
            get_license_from_string("rustdesk-host=server.example.net,.exe").unwrap(),
            License {
                host: "server.example.net".to_owned(),
                key: "".to_owned(),
                api: "".to_owned(),
            }
        );
        // key in these tests is "foobar.,2" base64 encoded
        assert_eq!(
            get_license_from_string("rustdesk-host=server.example.net,key=Zm9vYmFyLiwyCg==.exe")
                .unwrap(),
            License {
                host: "server.example.net".to_owned(),
                key: "Zm9vYmFyLiwyCg==".to_owned(),
                api: "".to_owned(),
            }
        );
        assert_eq!(
            get_license_from_string("rustdesk-host=server.example.net,key=Zm9vYmFyLiwyCg==,.exe")
                .unwrap(),
            License {
                host: "server.example.net".to_owned(),
                key: "Zm9vYmFyLiwyCg==".to_owned(),
                api: "".to_owned(),
            }
        );
    }
}
