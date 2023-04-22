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
    if s.contains("host=") {
        let strs: Vec<&str> = s.split("host=").collect();
        if strs.len() == 2 {
            let strs2: Vec<&str> = strs[1].split(",key=").collect();
            let host;
            let mut key = "";
            if strs2.len() == 2 {
                host = strs2[0];
                key = strs2[1];
            } else {
                host = strs[1];
            }
            return Ok(License {
                host: host.to_owned(),
                key: key.to_owned(),
                api: "".to_owned(),
            });
        }
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
