// RustDesk 授权校验模块
// 接入 auth.weail.icu 卡密验证系统
// 将此代码集成到 src/common.rs 或 src/main.rs

use std::time::Duration;
use reqwest::blocking::Client;
use serde_json::Value;

const AUTH_URL: &str = "https://auth.weail.icu/api/SoftwareApi/checkSoftware";
const APP_ID: &str = "1";
const PUBLIC_KEY_B64: &str = "MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAw2pZUg5gyFGwgutdUUO7/DZ1WMT1DdNU1/ZeGX0QpWqFYq517ay13N1oPFTNuaR1DV1BN+xO6QUMCu0VBuhP1QcO5ELgur8QouhRVrbN+T6Wnu0Oj/Q2B76zdiQxKYnZPpybzS/uF79pgQjgk0ZVvlWo7bne29VkS7ui5kUzKxAHycyLjmQAFbTccuIqfVOM2vn9Vg3zXMmHgX//ZSdgBlG7XVHBSGwVfLlliZLEh4SQzoLVPjt8PMkgbTJVndwJA9ecHcZY/3nkg4FYTkgMrXP9lrR/yYQMMKNTRqDFFKNtc9oEcCX6b7b8g2VaAHQFi+FuKKOcZs5c/ZpKnZ3UnQIDAQAB";

// 从本地文件读取授权信息
fn read_license_key() -> Option<String> {
    // 尝试从多个位置读取 license.key
    let paths = [
        "license.key",
        "/etc/rustdesk/license.key",
        &format!("{}/license.key", std::env::var("HOME").unwrap_or_default()),
    ];
    for path in paths.iter() {
        if let Ok(content) = std::fs::read_to_string(path) {
            let key = content.trim().to_string();
            if !key.is_empty() {
                return Some(key);
            }
        }
    }
    None
}

// 获取设备唯一标识（使用 RustDesk 自带的 ID）
fn get_device_id() -> String {
    // 使用 hbb_common::config 里的设备ID
    // 如果无法获取，则使用 hostname + mac 组合
    use std::process::Command;
    if let Ok(output) = Command::new("hostname").arg("-f").output() {
        if let Ok(hostname) = String::from_utf8(output.stdout) {
            return hostname.trim().to_string();
        }
    }
    "unknown_device".to_string()
}

// 读取私钥（Base64格式）
fn get_private_key_base64() -> String {
    // 私钥直接内嵌，避免文件依赖
    // 也可以从本地文件读取，这里提供两种方式
    let private_key_b64 = "MIIFDjBABgkqhkiG9w0BBQ0wMzAbBgkqhkiG9w0BBQwwDgQIPzAi43k+OqMCAggAMBQGCCqGSIb3DQMHBAgvYmjldnnDKASCBMjBpmHj15pd376H4qQDH0zE2aVWqFt8sZmUlQUy+zuZ1t0Frfl2Iq1aukvDXLMDwMClbqhnR+Yvt0sSk7kU5E5XFouH2T+djIID7X6TGGvgtYAAsMQl3Tu6jonYeyZC4XCe4aZQ3wSI+U2fKuSl6nuDhOhNr0/oqg8jOfDFsl3jeDKeWVtML4A4JEbyW2S4mSp+Ugx+reUa/8OUpgfXdrg1uBFovejq4AkkWwjj6jOUCcVZS426DknYgKgMzENhwP3hqF8iXjTI1AvDMP4JSe3cGhHPPKHoNGNbd2crWS5xxXqur2F2LEcJ1y4qJGlJB3coIIssYOcWk/3AHqiXu+4Gh2MQ31s1vo4atUC+UXIMpo1Vk3CDcQs4RX3+qzwwLK12Q+/fnvrQy0W2v1izA8AxoutldEqIADn5zpBjjIngKlmZ+xwATcKHxZlwI+L1Oba/DnTrqKoabwOdZowpz75HBhOW1sc9JZ9CqCg+3pA70+vzzvbAfMAwrKCSlC/sRh9Nhh01MUBSHoSRCxi1pLss5w+t3hfZYPS6H3uhQZ5tlO6FMODhni3fwSacnYk3HNNPAeTDuTaSYL2MHI/XGx1iJpUuQOftRcWxPaZke47w1SPo2bB+itgihhP2vFZ1j9bR1iqHBoo7iUeTCsBS+EtBX1DznNuwSU1ztw0xbmGNR0T2UpYuHgPaGuyfLvjok70Iq4ce0c2oTfRMoFTeznU0XetzsVDMnPQi66HWlHW8HVVEcJNq92fpc3dazrxYU7i0qXYhJrt7+JpMjf7gPL91/y5anCFpj/8W03yrgF42V0H3lt+p+vGJGTfuFGpcSSPoEfb6TyipM997HreKLgRtbSt1oJ2m0+GBj3/cC7wX6v2c5ArhfWGqfvpbaDyJyaKsqHyAC8BYbg+hD7J/k5hsfFlm9bw2yalrPtfpD3qI4ZRXiqQ91bNA7hFseVtAxP7D8LPgJK0xFkNQhMPz1RIW2MuUfqSsPnd4TQJ7AJB8eZX5btZh8vrnED7aybejZUYOqwdrNaN/zyud4BtVdcqBWpTUwiCFLT6LCxHFiqnTbCFzVqVdMNfbk9LJwVaQLGnWgF9LoBcPmnmR9gWSnZMESdh13YpAtEZpew8Fz7TpWLtIMMzhnjiUg8IEKUpL2CTWHMRd9ol7rf2E40Z08jOqaw/wdJkOQS2rPSbCfJzQgjTJgOPNXH/cQxrb8Z2SvoL9eWyAiZ0ZqGvEaMACcXDnBwg0P5ZWIx9XCeStTEaFc49QJWc/PhLh/2G1Ghq3xiMVA+0ISBIASNIGDphse0WoUZlWomg1HVAKUydTjUFVyANZWdsqRPvcWzKSw9kg5In9yBzyK1IH7hUFuhLMPYOquMJ0J4tGEulQ83Kkag5thYQSlmU08GlJ42XiIbIB4YUEOrFxH4Oblk8s1M6pT28ElVfCGHf4hKrlxtBRJjhMqguYazZ0UcsPT82fG8i4AL7T79w9uLRZzQnVit1quJ2aymlYNZR49Qk9r1NIQWlnK5gAsZSkyOuoCm8ZMUIPlgdIa9tFdIxo4BofOtxF/dkf094HR02DoAkqOJZaeO+Kt7/96wZX+PC6azzJaED+dxc1D9Rb0+IdFU8K/hnG0pBet+mqprXPG/4=";
    private_key_b64.to_string()
}

// 验证授权
pub fn verify_license() -> bool {
    // 1. 读取本地卡密
    let license_key = match read_license_key() {
        Some(key) => key,
        None => {
            eprintln!("未找到授权文件 license.key");
            return false;
        }
    };

    // 2. 获取设备ID
    let device_id = get_device_id();

    // 3. 发送验证请求
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .danger_accept_invalid_certs(true)  // 如果是自签证书需要这个
        .build();

    let client = match client {
        Ok(c) => c,
        Err(e) => {
            eprintln!("HTTP客户端创建失败: {}", e);
            return false;
        }
    };

    let params = [
        ("appid", APP_ID),
        ("key", &license_key),
        ("verify_data", &device_id),
        ("private_key", &get_private_key_base64()),
        ("public_key", PUBLIC_KEY_B64),
    ];

    match client.post(AUTH_URL).form(&params).send() {
        Ok(resp) => {
            let text = resp.text().unwrap_or_default();
            // 解析返回JSON
            if let Ok(json) = serde_json::from_str::<Value>(&text) {
                // 假设 code="1" 或 code=1 表示成功
                let code = json.get("code").map(|v| {
                    if v.is_string() {
                        v.as_str().unwrap_or("").to_string()
                    } else if v.is_number() {
                        v.to_string()
                    } else {
                        "".to_string()
                    }
                }).unwrap_or_default();

                if code == "1" || code == "1" {
                    println!("授权验证通过");
                    return true;
                } else {
                    let msg = json.get("msg").and_then(|v| v.as_str()).unwrap_or("未知错误");
                    eprintln!("授权验证失败: {}", msg);
                }
            } else {
                eprintln!("授权接口返回格式错误: {}", text);
            }
        }
        Err(e) => {
            eprintln!("授权请求失败: {}", e);
            // 可以选择离线模式：如果网络不通，检查本地缓存
        }
    }

    false
}

// 获取授权信息（供UI显示）
pub fn get_license_info() -> Option<(String, String)> {
    let key = read_license_key()?;
    let device = get_device_id();
    Some((key, device))
}
