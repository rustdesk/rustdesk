use docopt::Docopt;
use hbb_common::{
    env_logger::{init_from_env, Env, DEFAULT_FILTER_ENV},
    log, tokio,
};
use librustdesk::{ipc::Data, *};

const USAGE: &'static str = "
IPC test program.

Usage:
  ipc (-s | --server | -c | --client) [-p <str> | --postfix=<str>]
  ipc (-h | --help)

Options:
  -h --help             Show this screen.
  -s --server           Run as IPC server.
  -c --client           Run as IPC client.
  -p --postfix=<str>    IPC path postfix [default: ].
";

#[derive(Debug, serde::Deserialize)]
struct Args {
    flag_server: bool,
    flag_client: bool,
    flag_postfix: String,
}

#[tokio::main]
async fn main() {
    init_from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, "info"));

    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    if args.flag_server {
        if args.flag_postfix.is_empty() {
            log::info!("Starting IPC server...");
        } else {
            log::info!(
                "Starting IPC server with postfix: '{}'...",
                args.flag_postfix
            );
        }
        ipc_server(&args.flag_postfix).await;
    } else if args.flag_client {
        if args.flag_postfix.is_empty() {
            log::info!("Starting IPC client...");
        } else {
            log::info!(
                "Starting IPC client with postfix: '{}'...",
                args.flag_postfix
            );
        }
        ipc_client(&args.flag_postfix).await;
    }
}

async fn ipc_server(postfix: &str) {
    let postfix = postfix.to_string();
    let postfix2 = postfix.clone();
    std::thread::spawn(move || {
        if let Err(err) = crate::ipc::start(&postfix) {
            log::error!("Failed to start ipc: {}", err);
            std::process::exit(-1);
        }
    });
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    ipc_client(&postfix2).await;
}

async fn ipc_client(postfix: &str) {
    loop {
        match crate::ipc::connect(1000, postfix).await {
            Ok(mut conn) => match conn.send(&Data::Empty).await {
                Ok(_) => {
                    log::info!("send message to ipc server success");
                }
                Err(e) => {
                    log::error!("Failed to send message to ipc server: {}", e);
                }
            },
            Err(e) => {
                log::error!("Failed to connect to ipc server: {}", e);
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(6)).await;
    }
}
