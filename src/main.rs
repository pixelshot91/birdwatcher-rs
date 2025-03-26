#![feature(never_type)]

mod config;
mod deser;

use std::{fs::File, io::Write, path::PathBuf};

use tokio::{process::Command, task::JoinSet, time::timeout};

use config::{Config, ServiceState};

use clap::Parser;

use anyhow::{anyhow, Context, Result};

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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let config: Config = Config::load_from_file(&cli.config)
        .context(format!("Failed to load config file {:?}", cli.config))?;

    // Contains the only mutable state: a counter for each service
    let mut service_states: Vec<ServiceState> = config
        .service_definitions
        .iter()
        .map(|def|
            // Start with all services disabled, but only one success is enough to switch to `Success`
            ServiceState::Failure {
                nb_of_success: def.rise - 1,
        })
        .collect();

    write_bird_function(&config, &service_states);
    launch_reload_function(&config).await;

    let (tx, rx) = tokio::sync::mpsc::channel(1);

    let mut join_set = JoinSet::new();

    config
        .service_definitions
        .iter()
        .enumerate()
        .for_each(|(service_nb, service_def)| {
            println!("Starting {}", service_def.function_name);
            let service_def = service_def.clone();

            let tx = tx.clone();

            join_set.spawn(async move {
                loop {
                    println!(
                        "Regen function {}, Launching command {}",
                        service_def.function_name, service_def.command
                    );
                    let command = tokio::process::Command::new(service_def.command.clone())
                        .args(&service_def.args)
                        .output();
                    let result = timeout(service_def.command_timeout, command).await;
                    let return_value = match result {
                        Err(..) => {
                            println!("Command timed out");
                            false
                        }
                        Ok(Ok(o)) => {
                            if o.status.success() {
                                true
                            } else {
                                false
                            }
                        }
                        Ok(Err(e)) => {
                            println!(
                                "Could not launch command \'{}\'. e = {}",
                                service_def.command, e
                            );
                            false
                        }
                    };
                    println!(
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

    println!("All services launched");

    // Main task. Listen for new result from all the tasks spawned above
    join_set.spawn(async move {
        // Move rx inside this task
        let mut rx = rx;

        loop {
            let service_command_result = rx.recv().await.unwrap();
            let (new_state, should_reload) = service_states[service_command_result.service_id]
                .update_with(
                    service_command_result.success,
                    &config.service_definitions[service_command_result.service_id],
                );
            service_states[service_command_result.service_id] = new_state;

            if should_reload {
                write_bird_function(&config, &service_states);
                launch_reload_function(&config).await;
            }
        }
    });

    // No tasks should terminate (neither a service task or the main task).
    // If one does exit, this is an error
    let terminated_task: Result<!, tokio::task::JoinError> = join_set
        .join_next()
        .await
        .ok_or(anyhow!("No tasks in the JoinSet ??"))?;
    let err = terminated_task.unwrap_err();
    Err(anyhow!("A task failed: {}", err))
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
            format!(
                "
function {function_name}() -> bool
{{
    return {return_value};
}}
",
            )
        })
        .join("\n");

    let mut f = File::create(&config.generated_file_path).unwrap();
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
                println!("Reload successful");
            } else {
                println!(
                    "Reload failure. stdout = {}, stderr = {}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                );
            }
        }
        Ok(Err(e)) => {
            println!(
                "Could not launch reload command \'{}\'. e = {}",
                config.reload_command, e
            );
        }
        Err(_) => {
            println!("Reload command timed out");
        }
    };
}
