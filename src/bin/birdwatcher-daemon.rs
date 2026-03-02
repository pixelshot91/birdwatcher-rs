#![feature(never_type)]

use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use fs_err::PathExt;
use opentelemetry::KeyValue;
use tokio::{net::UnixListener, process::Command, task::JoinSet, time::timeout};

use birdwatcher_rs::{
    config::Config, rpc::common::Insight, rpc::server::InsightServer, service::ServiceState,
};

use clap::Parser;

use color_eyre::{
    eyre::{eyre, Context as _},
    Result,
};

use futures::prelude::*;

use tarpc::{
    server::{BaseChannel, Channel},
    tokio_serde::formats::Bincode,
    tokio_util::codec::LengthDelimitedCodec,
};
use tracing::{debug, error, field, info, warn, Instrument as _};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Sets a custom config file
    #[arg(short, long, value_name = "FILE")]
    config: PathBuf,
}

/// A message send by a Service task to the main task
struct ServiceCommandResult {
    service_id: usize,
    success: bool,
}

// opentelemetry metric provider need multi_thread runtime
#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let config: Config = Config::load_from_file(&cli.config)
        .wrap_err(format!("Failed to load config file {:?}", cli.config))?;

    let (meter_provider, logger_provider, tracer_provider) =
        birdwatcher_rs::telemetry::init_telemetry()?;

    birdwatcher_rs::telemetry::send_dummy_telemetry(
        &meter_provider,
        &logger_provider,
        &tracer_provider,
    )
    .unwrap();

    // Contains the only mutable state: a counter for each service
    let service_states: Vec<ServiceState> = config
        .service_definitions
        .iter()
        .map(|def|
            // Start with all services disabled, but only one success is enough to switch to `Success`
            ServiceState::Failure {
                nb_of_success: def.rise - 1,
        })
        .collect();
    let service_states = Arc::new(std::sync::Mutex::new(service_states));
    let config = Arc::new(config);

    let pid_path = "/tmp/birdwatcher.pid";
    match fs_err::read_to_string(pid_path) {
        Ok(stored_pid) => {
            let stored_pid = stored_pid.trim();
            let another_bw_is_running =
                std::path::Path::new(&format!("/proc/{stored_pid}")).fs_err_try_exists()?;
            if another_bw_is_running {
                error!("Another birdwatcher with PID {stored_pid} is already running");
                std::process::exit(1);
            } else {
                fs_err::write(pid_path, format!("{}\n", std::process::id()))?;
            }
        }
        Err(e) => {
            if let std::io::ErrorKind::NotFound = e.kind() {
                fs_err::write(pid_path, format!("{}\n", std::process::id()))?;
            } else {
                return Err(e).context(format!(
                    "Trying to know if another birdwatcher is running by looking at PID file `{pid_path}`",
                ));
            }
        }
    };

    let socket_path = "/tmp/birdwatcher.sock";
    match std::fs::remove_file(socket_path) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e).context(format!("Cannot remove file {socket_path}")),
    }

    let listener = UnixListener::bind(&Path::new(socket_path)).unwrap();

    async fn spawn(fut: impl Future<Output = ()> + Send + 'static) {
        debug!("spawning");
        tokio::spawn(fut);
    }
    let services_states_for_server = service_states.clone();
    let config_for_server = config.clone();

    let codec_builder = LengthDelimitedCodec::builder();

    tokio::spawn(async move {
        loop {
            let (conn, _addr) = listener.accept().await.unwrap();
            let framed = codec_builder.new_framed(conn);
            let transport = tarpc::serde_transport::new(framed, Bincode::default());

            let server = InsightServer {
                service_states: services_states_for_server.clone(),
                config: config_for_server.clone(),
            };
            let fut = BaseChannel::with_defaults(transport)
                .execute(server.serve())
                .for_each(spawn);
            tokio::spawn(fut);
        }
    });

    write_bird_function(&config, &service_states.lock().unwrap());
    launch_reload_function(&config).await;

    let meter = opentelemetry::global::meter("my_test_meter");
    let service_up = meter
        .u64_gauge("birdwatcher_service_up")
        .with_description("0 = The service is down. 1 = The service is up")
        .build();
    let service_hysteresis_state = meter
        .f64_gauge("birdwatcher_service_hysteresis_state")
        .with_description("Like service_up, but more detailed. It aggregates the result the last function_return value.
        It can take intermediate values between 0 and 1 for a failed service raising, or a successful service failing")
        .build();
    let function_return_value = meter
        .u64_gauge("birdwatcher_function_return_value")
        .with_description("Return value of a function.")
        .build();

    let (tx, rx) = tokio::sync::mpsc::channel(1);

    let mut join_set = JoinSet::new();

    config
        .service_definitions
        .iter()
        .enumerate()
        .for_each(|(service_nb, service_def)| {
            info!("Starting {}", service_def.function_name);
            let service_def = service_def.clone();

            let tx = tx.clone();

            let function_return_value = function_return_value.clone();

            join_set.spawn(async move {
                loop {
                    debug!(
                        "Regen function {}, Launching command {}",
                        service_def.function_name, service_def.command
                    );
                    let command = tokio::process::Command::new(service_def.command.clone())
                        .args(&service_def.args)
                        .output();

                    let command_execution_span = tracing::info_span!(
                        "function_execution",
                        service_def.command,
                        result = field::Empty
                    );
                    let result = timeout(service_def.command_timeout, command)
                        .instrument(command_execution_span.clone())
                        .await;

                    let return_value = match result {
                        Err(..) => {
                            info!(service_name = service_def.service_name, "Command timed out");
                            command_execution_span.record("result", "timeout");

                            false
                        }
                        Ok(Ok(o)) => {
                            let span_result = if o.status.success() {
                                "success"
                            } else {
                                "non-zero status"
                            };
                            command_execution_span
                                .record("result", format!("returned {}", span_result));
                            o.status.success()
                        }
                        Ok(Err(e)) => {
                            warn!(
                                service_name = service_def.service_name,
                                "Could not launch command \'{}\'. e = {}", service_def.command, e
                            );
                            command_execution_span.record("result", "error launching command");

                            false
                        }
                    };
                    let return_value_u64 = if return_value { 1 } else { 0 };
                    function_return_value.record(
                        return_value_u64,
                        &[KeyValue::new("service", service_def.service_name.clone())],
                    );
                    debug!(
                        "function name {}, return value {return_value}",
                        service_def.function_name
                    );

                    tx.send(ServiceCommandResult {
                        service_id: service_nb,
                        success: return_value,
                    })
                    .await
                    .unwrap();

                    tokio::time::sleep(service_def.interval).await;
                }
            });
        });

    info!("All services launched");

    // Main task. Listen for new result from all the tasks spawned above
    join_set.spawn(async move {
        // Move rx inside this task
        let mut rx = rx;

        loop {
            let service_command_result = rx.recv().await.unwrap();

            let (service_states_copy, should_reload) = {
                let mut service_states = service_states.lock().unwrap();
                let (new_state, should_reload) = service_states[service_command_result.service_id]
                    .update_with(
                        service_command_result.success,
                        &config.service_definitions[service_command_result.service_id],
                    );
                let (service_up_value, service_hysteresis_state_value) = match &new_state {
                    ServiceState::Failure { nb_of_success } => (
                        0,
                        *nb_of_success as f64
                            / config.service_definitions[service_command_result.service_id].rise
                                as f64,
                    ),
                    ServiceState::Success { nb_of_failure } => (
                        1,
                        1.0 - (*nb_of_failure as f64
                            / config.service_definitions[service_command_result.service_id].fall
                                as f64),
                    ),
                };
                service_states[service_command_result.service_id] = new_state;

                service_up.record(
                    service_up_value,
                    &[KeyValue::new(
                        "service",
                        config.service_definitions[service_command_result.service_id]
                            .service_name
                            .clone(),
                    )],
                );
                service_hysteresis_state.record(
                    service_hysteresis_state_value,
                    &[KeyValue::new(
                        "service",
                        config.service_definitions[service_command_result.service_id]
                            .service_name
                            .clone(),
                    )],
                );

                (service_states.clone(), should_reload)
            };

            if should_reload {
                write_bird_function(&config, &service_states_copy);
                launch_reload_function(&config).await;
            }
        }
    });

    // No tasks should terminate (neither a service task or the main task).
    // If one does exit, this is an error
    let terminated_task: Result<!, tokio::task::JoinError> = join_set
        .join_next()
        .await
        .ok_or(eyre!("No tasks in the JoinSet ??"))?;
    let err = terminated_task.unwrap_err();
    Err(eyre!("A task failed: {}", err))
}

fn write_bird_function(config: &Config, services_states: &[ServiceState]) {
    use itertools::Itertools;
    // Combines the services static definition and their mutable state
    let services = config.service_definitions.iter().zip(services_states);
    let content = services
        .map(|(service_def, service_state)| {
            let function_name = &service_def.function_name;
            let return_value = match service_state {
                ServiceState::Failure { .. } => "false",
                ServiceState::Success { .. } => "true",
            };
            let return_type = match config.generated_file.function_return_type {
                true => "-> bool",
                false => "",
            };
            format!(
                "
function {function_name}() {return_type}
{{
    return {return_value};
}}
",
            )
        })
        .join("\n");

    let mut f = fs_err::File::create(&config.generated_file.path).unwrap();
    f.write_all(content.as_bytes()).unwrap();
}

async fn launch_reload_function(config: &Config) {
    let reload_command = Command::new(&config.reload_command)
        .args(&config.reload_command_args)
        .output();
    let reload_return_value = timeout(config.reload_timeout, reload_command).await;
    match reload_return_value {
        Ok(Ok(o)) => {
            if o.status.success() {
                info!("Reload successful");
            } else {
                error!(
                    "Reload failure. stdout = {}, stderr = {}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                );
            }
        }
        Ok(Err(e)) => {
            error!(
                "Could not launch reload command \'{}\'. e = {}",
                config.reload_command, e
            );
        }
        Err(_) => {
            error!("Reload command timed out");
        }
    };
}
