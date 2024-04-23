mod custom_server;
use hbb_common::{ResultType, base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _}};
use custom_server::*;

fn gen_name(lic: &CustomServer) -> ResultType<String> {
    let tmp = URL_SAFE_NO_PAD.encode(&serde_json::to_vec(lic)?);
    Ok(tmp.chars().rev().collect())
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let api = args.get(2).cloned().unwrap_or_default();
    let relay = args.get(3).cloned().unwrap_or_default();
    if args.len() >= 2 {
        match gen_name(&CustomServer {
            key: args[0].clone(),
            host: args[1].clone(),
            api,
            relay,
        }) {
            Ok(name) => println!("rustdesk-custom_serverd-{}.exe", name),
            Err(e) => println!("{:?}", e),
        }
    }
    if args.len() == 1 {
        println!("{:?}", get_custom_server_from_string(&args[0]));
    }
}
