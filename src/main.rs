use std::{ffi::OsString, fs::File, io::Write, process::ExitCode, time::Duration};

use tokio::{process::Command, task::JoinSet, time::timeout};

#[derive(Clone)]
struct ServiceDefinition {
    function_name: String,
    command: String,
    args: Vec<String>,
    interval: Duration,
    command_timeout: Duration,
    /// Number of consecutive failure to consider the service unhealthy
    fall: u32,
    /// Number of consecutive failure to consider the service healthy
    rise: u32,
}

#[derive(Debug)]
enum ServiceState {
    /// In `Failure` state, count the number of success.
    /// When the number goes above `rise`, switch to `Success` state
    /// Any failure reset the counter to 0
    Failure { nb_of_success: u32 },
    /// In `Success` state, count the number of failure.
    /// When the number goes above `fall`, switch to `Failure` state
    /// Any failure reset the counter to 0
    Success { nb_of_failure: u32 },
}

impl ServiceState {
    fn update_with(
        &self,
        return_value: bool,
        service_def: &ServiceDefinition,
    ) -> (ServiceState, bool) {
        match self {
            ServiceState::Failure { nb_of_success } => {
                if return_value {
                    if nb_of_success + 1 >= service_def.rise {
                        // Switch to rise
                        (ServiceState::Success { nb_of_failure: 0 }, true)
                    } else {
                        // Another success, but not enough to rise
                        (
                            ServiceState::Failure {
                                nb_of_success: nb_of_success + 1,
                            },
                            false,
                        )
                    }
                } else
                /* A failure on failure */
                {
                    (ServiceState::Failure { nb_of_success: 0 }, false)
                }
            }
            ServiceState::Success { nb_of_failure } => {
                if return_value {
                    (ServiceState::Success { nb_of_failure: 0 }, false)
                } else
                /* A new failure. Should we switch to Failure? */
                {
                    // Yes, switch to Failure
                    if nb_of_failure + 1 >= service_def.fall {
                        (ServiceState::Failure { nb_of_success: 0 }, true)
                    } else {
                        // No. Another failure, but not enough to fall
                        (
                            ServiceState::Success {
                                nb_of_failure: nb_of_failure + 1,
                            },
                            false,
                        )
                    }
                }
            }
        }
    }
}

struct Service {
    def: ServiceDefinition,
    state: ServiceState,
}

#[derive(Clone)]
struct Config {
    generated_file_path: OsString,
    reload_command: String,
    reload_command_args: Vec<String>,
    reload_timeout: Duration,
    service_definitions: Vec<ServiceDefinition>,
}

struct ServiceCommandResult {
    service_id: usize,
    success: bool,
}

#[tokio::main]
async fn main() {
    let config = Config {
        generated_file_path: "birdwatcher_generated.conf".into(),
        reload_command: "birdc".to_string(),
        reload_command_args: vec!["configure".to_string()],
        reload_timeout: Duration::from_secs(2),
        service_definitions: vec![
            ServiceDefinition {
                function_name: "match_true".to_string(),
                command: "/bin/ls".to_string(),
                args: vec!["1".to_string()],
                interval: Duration::from_secs(1),
                command_timeout: Duration::from_secs(1),
                fall: 1,
                rise: 3,
            },
            ServiceDefinition {
                function_name: "match_false".to_string(),
                command: "/bin/ls".to_string(),
                args: vec!["2".to_string()],
                interval: Duration::from_secs(2),
                command_timeout: Duration::from_secs(1),
                fall: 2,
                rise: 2,
            },
        ],
    };

    let mut service_states: Vec<ServiceState> = config
        .service_definitions
        .iter()
        .map(|def| ServiceState::Failure {
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

    if let Some(t) = join_set.join_next().await {
        println!("Task failed with {}", t.err().unwrap())
    }
}

fn write_bird_function(config: &Config, services_states: &[ServiceState]) {
    use itertools::Itertools;
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
