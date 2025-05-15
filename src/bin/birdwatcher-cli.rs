use birdwatcher_rs::service::Bundle;
use birdwatcher_rs::{rpc::common::InsightClient, tui};
use itertools::Itertools as _;
use std::iter::zip;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use std::{net::SocketAddr, str::FromStr, time::Duration};
use tarpc::{client, context, tokio_serde::formats::Json};

#[tokio::main(flavor = "current_thread")]
async fn main() -> color_eyre::Result<()> {
    let bundle = Arc::new(Mutex::<Option<Bundle>>::new(None));

    let bundle_for_tarp = bundle.clone();

    tokio::spawn(async {
        let bundle = bundle_for_tarp;
        let mut transport = tarpc::serde_transport::tcp::connect(
            SocketAddr::from_str("[::1]:50051").unwrap(),
            Json::default,
        );
        transport.config_mut().max_frame_length(usize::MAX);

        let client =
            InsightClient::new(client::Config::default(), transport.await.unwrap()).spawn();

        let mut interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            let res = client.get_data(context::current()).await;

            let received_bundle = res.unwrap();
            let services = zip(
                received_bundle.config.service_definitions.iter(),
                received_bundle.service_states.iter(),
            )
            .map(|(def, state)| format!("{}: {:?}", def.service_name, state))
            .join("\n");

            // println!("res = {}", services);
            {
                let mut bundle = bundle.lock().unwrap();
                // bundle.replace(received_bundle);
                *bundle = Some(received_bundle);
            }

            interval.tick().await;
        }
    });

    let res = tokio::spawn(async {
        color_eyre::install()?;
        let terminal = ratatui::init();
        let app_result = tui::table::App::new(bundle).run(terminal).await;
        ratatui::restore();
        app_result
    });

    res.await?
}
