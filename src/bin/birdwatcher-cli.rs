use birdwatcher_rs::{rpc::common::InsightClient, service::Bundle, tui};
use clap::{command, Parser, Subcommand};
use std::sync::{Arc, Mutex};
use std::{net::SocketAddr, str::FromStr, time::Duration};
use tarpc::tokio_serde::formats::Bincode;
use tarpc::{client, context, tokio_serde::formats::Json};
use tokio::net::UnixStream;
use tokio::task::JoinSet;

#[derive(Parser, Debug)] // requires `derive` feature
struct CliArg {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Output a snaphot of the services state
    Json {},
    /// Show a live view of the services state
    Tui {},
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> color_eyre::Result<()> {
    let args = CliArg::parse();
    if let Commands::Json {} = args.command {
        let conn = UnixStream::connect("/tmp/birdwatcher.sock").await?;

        let codec_builder = tarpc::tokio_util::codec::LengthDelimitedCodec::builder();
        let transport =
            tarpc::serde_transport::new(codec_builder.new_framed(conn), Bincode::default());
        let client = InsightClient::new(client::Config::default(), transport).spawn();

        let res = client.get_data(context::current()).await?;

        dbg!(res);

        return Ok(());
    }

    let bundle = Arc::new(Mutex::<Option<Bundle>>::new(None));

    let bundle_for_tarp = bundle.clone();

    let mut set = JoinSet::new();

    set.spawn(async {
        let bundle = bundle_for_tarp;

        let mut interval = tokio::time::interval(Duration::from_secs(1));

        'connection: loop {
            let mut transport = tarpc::serde_transport::tcp::connect(
                SocketAddr::from_str("[::1]:50051").unwrap(),
                Json::default,
            );
            transport.config_mut().max_frame_length(usize::MAX);
            match transport.await {
                Err(_) => {
                    interval.tick().await;
                    continue 'connection;
                }

                Ok(t) => {
                    let client = InsightClient::new(client::Config::default(), t).spawn();

                    loop {
                        let res = client.get_data(context::current()).await;

                        let received_bundle = res.ok();
                        let should_reset_connection = received_bundle.is_none();
                        /* let services = zip(
                            received_bundle.config.service_definitions.iter(),
                            received_bundle.service_states.iter(),
                        )
                        .map(|(def, state)| format!("{}: {:?}", def.service_name, state))
                        .join("\n"); */

                        // println!("res = {}", services);
                        {
                            let mut bundle = bundle.lock().unwrap();
                            *bundle = received_bundle;
                        }

                        if should_reset_connection {
                            continue 'connection;
                        }

                        interval.tick().await;
                    }
                }
            };
        }
    });

    set.spawn(async {
        color_eyre::install()?;
        let terminal = ratatui::init();
        let app_result = tui::table::App::new(bundle).run(terminal).await;
        ratatui::restore();
        app_result
    });

    set.join_next().await.unwrap().unwrap()
}
