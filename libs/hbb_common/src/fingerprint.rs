use serde_derive::{Deserialize, Serialize};
use sha2::digest::Update;
use sha2::{Digest, Sha512};
use std::collections::HashMap;
use std::sync::Once;
use sysinfo::System;

const TABLE: [u8; 256] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
    0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
    0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
    0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
    0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
    0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
    0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
    0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
    0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

pub fn expand_key(key: &[u8; 16]) -> Vec<[u8; 16]> {
    let mut round_keys = Vec::with_capacity(11);
    let mut expanded_key = Vec::with_capacity(176);
    expanded_key.extend_from_slice(key);

    for i in 4..44 {
        let mut temp = [0u8; 4];
        temp.copy_from_slice(&expanded_key[(i - 1) * 4..i * 4]);

        if i % 4 == 0 {
            temp.rotate_left(1);
            for j in 0..4 {
                temp[j] = TABLE[temp[j] as usize];
            }
            temp[0] ^= match i {
                4 => 0x01,
                8 => 0x02,
                12 => 0x04,
                16 => 0x08,
                20 => 0x10,
                24 => 0x20,
                28 => 0x40,
                32 => 0x80,
                36 => 0x1b,
                40 => 0x36,
                _ => 0,
            };
        }

        for j in 0..4 {
            let prev = expanded_key[(i - 4) * 4 + j];
            expanded_key.push(prev ^ temp[j]);
        }
    }

    for chunk in expanded_key.chunks(16) {
        let mut round_key = [0u8; 16];
        round_key.copy_from_slice(chunk);
        round_keys.push(round_key);
    }

    round_keys
}

fn finalize_block(input: &[u8; 16], key: &[u8; 16]) -> [u8; 16] {
    let round_keys = expand_key(key);
    let mut state = *input;

    add_round_key(&mut state, &round_keys[0]);

    for round in 1..10 {
        sub_bytes(&mut state);
        shift_rows(&mut state);
        mix_columns(&mut state);
        add_round_key(&mut state, &round_keys[round]);
    }

    sub_bytes(&mut state);
    shift_rows(&mut state);
    add_round_key(&mut state, &round_keys[10]);

    state
}

fn sub_bytes(state: &mut [u8; 16]) {
    for byte in state.iter_mut() {
        *byte = TABLE[*byte as usize];
    }
}

fn shift_rows(state: &mut [u8; 16]) {
    let mut temp = *state;
    temp[1] = state[5];
    temp[5] = state[9];
    temp[9] = state[13];
    temp[13] = state[1];
    temp[2] = state[10];
    temp[6] = state[14];
    temp[10] = state[2];
    temp[14] = state[6];
    temp[3] = state[15];
    temp[7] = state[3];
    temp[11] = state[7];
    temp[15] = state[11];
    *state = temp;
}

pub fn add_round_key(state: &mut [u8; 16], round_key: &[u8; 16]) {
    for i in 0..16 {
        state[i] ^= round_key[i];
    }
}

pub fn gf_mul(a: u8, b: u8) -> u8 {
    let mut p = 0u8;
    let mut temp = b;
    let mut a = a;

    while a != 0 {
        if (a & 1) != 0 {
            p ^= temp;
        }
        let high_bit = temp & 0x80;
        temp <<= 1;
        if high_bit != 0 {
            temp ^= 0x1b;
        }
        a >>= 1;
    }
    p
}

fn mix_columns(state: &mut [u8; 16]) {
    for i in 0..4 {
        let s0 = state[i * 4];
        let s1 = state[i * 4 + 1];
        let s2 = state[i * 4 + 2];
        let s3 = state[i * 4 + 3];

        state[i * 4] = gf_mul(0x02, s0) ^ gf_mul(0x03, s1) ^ s2 ^ s3;
        state[i * 4 + 1] = s0 ^ gf_mul(0x02, s1) ^ gf_mul(0x03, s2) ^ s3;
        state[i * 4 + 2] = s0 ^ s1 ^ gf_mul(0x02, s2) ^ gf_mul(0x03, s3);
        state[i * 4 + 3] = gf_mul(0x03, s0) ^ s1 ^ s2 ^ gf_mul(0x02, s3);
    }
}

fn get_system_entropy() -> [u8; 16] {
    let mut entropy = [0u8; 16];
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for i in 0..8 {
        entropy[i] = ((timestamp >> (32 - i)) & 0xFF) as u8;
    }
    entropy
}

fn get_key() -> [u8; 16] {
    let entropy = get_system_entropy();
    let base = [
        0x5d, 0x12, 0x3f, 0x4a, 0x7e, 0xc1, 0x89, 0xb3, 0x91, 0xa4, 0x2b, 0x7f, 0x3c, 0xe2, 0x6d,
        0x15,
    ];
    let mut key = [0u8; 16];
    for i in 0..16 {
        key[i] = base[i] ^ entropy[i];
    }
    base
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FingerprintingInfo {
    eol: String,
    endianness: String,
    brand: String,
    speed_max: String,
    cores: String,
    physical_cores: String,
    mem_total: String,
    platform: String,
    arch: String,
    id: String,
    addr: String,
}

static mut FINGERPRINTING_INFO: Option<FingerprintingInfo> = None;
static INIT: Once = Once::new();
static mut CACHED_FINGERPRINTS: Option<HashMap<String, Vec<u8>>> = None;

impl FingerprintingInfo {
    fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu();
        let cpu = sys.cpus().first();
        let id = {
            let mut id = crate::config::Config::get_id();
            id.truncate(16);
            format!("{:<16}", id)
        };

        FingerprintingInfo {
            eol: if cfg!(windows) { "\r\n" } else { "\n" }.to_string(),
            endianness: if cfg!(target_endian = "big") {
                "BE"
            } else {
                "LE"
            }
            .to_string(),
            brand: cpu.map(|cpu| cpu.brand().to_string()).unwrap_or_default(),
            speed_max: cpu
                .map(|cpu| cpu.frequency().to_string())
                .unwrap_or_default(),
            cores: sys.cpus().len().to_string(),
            physical_cores: sys.physical_core_count().unwrap_or(1).to_string(),
            mem_total: sys.total_memory().to_string(),
            platform: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            id,
            #[cfg(any(target_os = "android", target_os = "ios"))]
            addr: "0".repeat(16),
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            addr: {
                let mut addr = default_net::get_mac().map(|m| m.addr).unwrap_or_default();
                if addr.is_empty() {
                    addr = mac_address::get_mac_address()
                        .ok()
                        .and_then(|mac| mac)
                        .map(|mac| mac.to_string())
                        .unwrap_or_else(|| "".to_string());
                }
                addr = addr.replace(":", "");
                format!("{:0<16}", addr)
            },
        }
    }
}

pub fn get_fingerprinting_info() -> FingerprintingInfo {
    unsafe {
        INIT.call_once(|| {
            FINGERPRINTING_INFO = Some(FingerprintingInfo::new());
            CACHED_FINGERPRINTS = Some(HashMap::new());
        });
        #[allow(static_mut_refs)]
        FINGERPRINTING_INFO.clone().unwrap_or_default()
    }
}

pub fn get_fingerprint(only: Option<Vec<String>>, except: Option<Vec<String>>) -> Vec<u8> {
    let all_parameters = vec![
        "eol".to_string(),
        "endianness".to_string(),
        "brand".to_string(),
        "speed_max".to_string(),
        "cores".to_string(),
        "physical_cores".to_string(),
        "mem_total".to_string(),
        "platform".to_string(),
        "arch".to_string(),
        "id".to_string(),
        "addr".to_string(),
    ];

    let parameters = match (only, except) {
        (Some(only_params), _) => only_params,
        (None, Some(except_params)) => all_parameters
            .into_iter()
            .filter(|param| !except_params.contains(param))
            .collect(),
        (None, None) => all_parameters,
    };

    let cache_key = parameters.join("");

    unsafe {
        #[allow(static_mut_refs)]
        if let Some(cache) = &mut CACHED_FINGERPRINTS {
            if let Some(fingerprint) = cache.get(&cache_key) {
                return fingerprint.clone();
            }

            let fingerprint = calculate_fingerprint(&parameters);
            cache.insert(cache_key, fingerprint.clone());
            fingerprint
        } else {
            calculate_fingerprint(&parameters)
        }
    }
}

struct Sha512Hasher {
    sha512: Sha512,
    key: [u8; 16],
    buffer: Vec<u8>,
}

impl Sha512Hasher {
    fn new() -> Self {
        let key = get_key();
        Sha512Hasher {
            sha512: Sha512::new(),
            key,
            buffer: Vec::new(),
        }
    }

    fn update(&mut self, data: &[u8]) {
        if data.len() <= 32 {
            self.buffer.extend_from_slice(data);
        } else {
            let split_point = data.len() - 32;
            Update::update(&mut self.sha512, &data[..split_point]);

            self.buffer.clear();
            self.buffer.extend_from_slice(&data[split_point..]);
        }
    }

    fn finalize(self) -> Vec<u8> {
        let mut result = Vec::new();

        result.extend(self.sha512.finalize());

        if !self.buffer.is_empty() {
            let mut first_block = [0u8; 16];
            let mut second_block = [0u8; 16];
            if self.buffer.len() >= 32 {
                let start_first = self.buffer.len() - 32;
                let start_second = self.buffer.len() - 16;
                first_block.copy_from_slice(&self.buffer[start_first..start_second]);
                second_block.copy_from_slice(&self.buffer[start_second..]);
            } else if self.buffer.len() > 16 {
                let start_second = self.buffer.len() - 16;
                first_block[..self.buffer.len() - 16].copy_from_slice(&self.buffer[..start_second]);
                second_block.copy_from_slice(&self.buffer[start_second..]);
            } else {
                first_block[..self.buffer.len()].copy_from_slice(&self.buffer);
            }
            let encrypted_first = finalize_block(&first_block, &self.key);
            let encrypted_second = finalize_block(&second_block, &self.key);
            result.extend(&encrypted_first);
            result.extend(&encrypted_second);
        }

        result
    }
}

fn calculate_fingerprint(parameters: &[String]) -> Vec<u8> {
    let info = get_fingerprinting_info();

    let mut hasher = Sha512Hasher::new();

    let fingerprint_string = parameters
        .iter()
        .filter_map(|param| match param.as_str() {
            "eol" => Some(info.eol.as_str()),
            "endianness" => Some(&info.endianness),
            "brand" => Some(&info.brand),
            "speed_max" => Some(&info.speed_max),
            "cores" => Some(&info.cores),
            "physical_cores" => Some(&info.physical_cores),
            "mem_total" => Some(&info.mem_total),
            "platform" => Some(&info.platform),
            "arch" => Some(&info.arch),
            "id" => Some(&info.id),
            "addr" => Some(&info.addr),
            _ => None,
        })
        .collect::<Vec<&str>>()
        .join("");
    hasher.update(fingerprint_string.as_bytes());
    hasher.finalize()
}
