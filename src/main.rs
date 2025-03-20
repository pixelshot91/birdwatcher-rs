use std::{
    ffi::{OsStr, OsString},
    fs::File,
    io::Write,
    sync::{Arc, Mutex},
    time::Duration,
    vec,
};

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
}

#[tokio::main]
async fn main() {
    let config = Config {
        generated_file_path: "birdwatcher_generated.conf".into(),
        reload_command: "birdc".to_string(),
        reload_command_args: vec!["configure".to_string()],
        reload_timeout: Duration::from_secs(2),
    };

    let service_defintions = [
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
            command: "/bin/sleep".to_string(),
            args: vec!["2".to_string()],
            interval: Duration::from_secs(2),
            command_timeout: Duration::from_secs(1),
            fall: 2,
            rise: 2,
        },
    ];

    let services: Vec<Service> = service_defintions
        .iter()
        .map(|def| Service {
            def: def.clone(),
            state: ServiceState::Failure {
                nb_of_success: def.rise - 1,
            },
        })
        .collect();

    write_bird_function(&config.generated_file_path, &services);
    launch_reload_function(&config).await;

    let services: Arc<Mutex<Vec<Service>>> = Arc::new(Mutex::new(services));

    let mut join_set = JoinSet::new();

    service_defintions
        .into_iter()
        .enumerate()
        .for_each(|(service_nb, service_def)| {
            println!("Staring {}", service_def.function_name);
            let services = services.clone();
            let config = config.clone();
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
                    println!("return value {return_value}");
                    let should_reload_bird = {
                        let mut services_lock = services.lock().unwrap();
                        let old_state = &services_lock[service_nb].state;

                        let (new_state, should_reload) =
                            old_state.update_with(return_value, &service_def);
                        println!("{:?}", new_state);
                        services_lock[service_nb].state = new_state;

                        if should_reload {
                            write_bird_function(
                                &config.generated_file_path,
                                services_lock.as_slice(),
                            );
                            // We cannot call the reload command here because we still hold a lock over `services`.
                            // So first get out of this scope
                        }
                        should_reload
                    };
                    if should_reload_bird {
                        println!("Need to reload Bird");
                        launch_reload_function(&config).await;
                    }

                    tokio::time::sleep(service_def.interval).await;
                }
            });
        });

    println!("All services launched");
    if let Some(t) = join_set.join_next().await {
        println!("Task failed with {}", t.err().unwrap())
    }
}

fn write_bird_function(generated_file_path: &OsStr, services: &[Service]) {
    use itertools::Itertools;
    let content = services
        .iter()
        .map(|service| {
            let function_name = &service.def.function_name;
            let return_value = match service.state {
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

    let mut f = File::create(generated_file_path).unwrap();
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
