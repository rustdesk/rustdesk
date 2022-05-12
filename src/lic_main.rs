mod license;
use hbb_common::{sodiumoxide::crypto::sign, ResultType};
use license::*;

fn gen_license(lic: &License) -> ResultType<String> {
    let tmp = serde_json::to_vec::<License>(lic)?;
    const SK: &[u8; 64] = &[
        139, 164, 88, 86, 6, 123, 221, 248, 96, 36, 106, 207, 99, 124, 27, 196, 5, 159, 58, 253,
        238, 94, 3, 184, 237, 236, 122, 59, 205, 95, 6, 189, 88, 168, 68, 104, 60, 5, 163, 198,
        165, 38, 12, 85, 114, 203, 96, 163, 70, 48, 0, 131, 57, 12, 46, 129, 83, 17, 84, 193, 119,
        197, 130, 103,
    ];
    let sk = sign::SecretKey(*SK);
    let tmp = base64::encode_config(sign::sign(&tmp, &sk), base64::URL_SAFE_NO_PAD);
    let tmp: String = tmp.chars().rev().collect();
    Ok(tmp)
}

fn main() {
    let mut args = Vec::new();
    let mut i = 0;
    for arg in std::env::args() {
        if i > 0 {
            args.push(arg);
        }
        i += 1;
    }
    let api = if args.len() < 3 {
        "".to_owned()
    } else {
        args[2].clone()
    };
    if args.len() == 3 {
        println!(
            "{:?}",
            gen_license(&License {
                key: args[0].clone(),
                host: args[1].clone(),
                api,
            })
        );
    }
    if args.len() == 1 {
        println!("{:?}", get_license_from_string(&args[0]));
    }
}
