use std::{
    ffi::{OsStr, OsString},
    fs::File,
    io::Write,
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::task::JoinSet;

#[derive(Clone)]
struct ServiceDefinition {
    function_name: String,
    command: String,
    interval: Duration,
    /// Number of consecutive failure to consider the service unhealthy
    fall: u32,
    /// Number of consecutive failure to consider the service healthy
    rise: u32,
}

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
/*
let (new_state, should_reload) = match old_state {
    ServiceState::Failure {
        nb_of_consecutive_success,
    } => {
        if return_value {
            if nb_of_consecutive_success + 1 == service_def.rise {
                (
                    ServiceState::Success {
                        nb_of_consecutive_failure: 0,
                    },
                    true,
                )
            } else {
                (
                    ServiceState::Failure {
                        nb_of_consecutive_success: nb_of_consecutive_success
                            + 1,
                    },
                    false,
                )
            }
        } else {
            // Still another failure
            (
                ServiceState::Failure {
                    nb_of_consecutive_success: 0,
                },
                false,
            )
        }
    }
    ServiceState::Success {
        nb_of_consecutive_failure,
    } => todo!(),
}; */

struct Service {
    def: ServiceDefinition,
    state: ServiceState,
}

struct Config {
    generated_file_path: OsString,
}

#[tokio::main]
async fn main() {
    let config = Config {
        generated_file_path: "birdwatcher_generated.conf".into(),
    };

    let service_defintions = [
        ServiceDefinition {
            function_name: "match_true".to_string(),
            command: "/bin/true".to_string(),
            interval: Duration::from_secs(1),
            fall: 1,
            rise: 3,
        },
        ServiceDefinition {
            function_name: "match_false".to_string(),
            command: "/bin/false".to_string(),
            interval: Duration::from_secs(2),
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

    let services: Arc<Mutex<Vec<Service>>> = Arc::new(Mutex::new(services));

    let mut join_set = JoinSet::new();

    service_defintions
        .into_iter()
        .enumerate()
        .for_each(|(service_nb, service_def)| {
            println!("Staring {}", service_def.function_name);
            let services = services.clone();
            let generated_file_path = config.generated_file_path.clone();
            join_set.spawn(async move {
                loop {
                    println!(
                        "Regen function {}, Launching command {}",
                        service_def.function_name, service_def.command
                    );
                    let result = tokio::process::Command::new(service_def.command.clone())
                        .output()
                        .await;
                    let return_value = match result {
                        Ok(o) => {
                            if o.status.success() {
                                true
                            } else {
                                false
                            }
                        }
                        Err(e) => {
                            eprintln!(
                                "Could not launch command \'{}\'. e = {}",
                                service_def.command, e
                            );
                            false
                        }
                    };
                    {
                        let mut services_lock = services.lock().unwrap();
                        let old_state = &services_lock[service_nb].state;

                        let (new_state, should_reload) =
                            old_state.update_with(return_value, &service_def);
                        services_lock[service_nb].state = new_state;

                        // let state_changed = state.update_with(return_value);

                        /* let old_state = &services_lock[service_nb].state;
                        let (new_state, should_reload) = match old_state {
                            ServiceState::Failure { nb_of_success } => {
                                if return_value {
                                    if nb_of_consecutive_success + 1 == service_def.rise {
                                        (ServiceState::Success { nb_of_failure: 0 }, true)
                                    } else {
                                        (
                                            ServiceState::Failure {
                                                nb_of_success: nb_of_consecutive_success + 1,
                                            },
                                            false,
                                        )
                                    }
                                } else {
                                    // Still another failure
                                    (ServiceState::Failure { nb_of_success: 0 }, false)
                                }
                            }
                            ServiceState::Success { nb_of_failure } => todo!(),
                        }; */
                        if should_reload {
                            write_bird_function(&generated_file_path, services_lock.as_slice());
                            // TODO: bird configure
                        }
                    }

                    tokio::time::sleep(service_def.interval).await;
                }
            });
        });

    println!("All services launched");
    if let Some(t) = join_set.join_next().await {
        eprintln!("Task failed with {}", t.err().unwrap())
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
