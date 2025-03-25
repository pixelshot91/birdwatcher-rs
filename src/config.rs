// Use f32 instead of Duration to avoid having to create a `secs` and `nanos` entry for each duration in the TOML file
mod raw {
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct Config {
        pub generated_file_path: String,
        pub reload_command: String,
        pub reload_command_args: Vec<String>,
        pub reload_timeout: f32,
        pub service_definitions: Vec<ServiceDefinition>,
    }

    #[derive(Clone, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct ServiceDefinition {
        pub function_name: String,
        pub command: String,
        pub args: Vec<String>,
        pub interval: f32,
        pub command_timeout: f32,
        /// Number of consecutive failure to consider the service unhealthy
        pub fall: u32,
        /// Number of consecutive failure to consider the service healthy
        pub rise: u32,
    }
}

use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Deserialize, Serialize)]
pub struct Config {
    pub generated_file_path: String,
    pub reload_command: String,
    pub reload_command_args: Vec<String>,
    pub reload_timeout: Duration,
    pub service_definitions: Vec<ServiceDefinition>,
}

impl Config {
    pub fn load_from_file(filepath: &str) -> Config {
        let raw_config: Result<crate::config::raw::Config, _> =
            toml::from_str(&std::fs::read_to_string(filepath).unwrap());
        match raw_config {
            Err(e) => {
                println!("{}", e);
                panic!();
            }

            Ok(raw_config) => Config {
                generated_file_path: raw_config.generated_file_path,
                reload_command: raw_config.reload_command,
                reload_command_args: raw_config.reload_command_args,
                reload_timeout: Duration::from_secs_f32(raw_config.reload_timeout),
                service_definitions: raw_config
                    .service_definitions
                    .into_iter()
                    .map(|raw| ServiceDefinition {
                        function_name: raw.function_name,
                        command: raw.command,
                        args: raw.args,
                        interval: Duration::from_secs_f32(raw.interval),
                        command_timeout: Duration::from_secs_f32(raw.command_timeout),
                        fall: raw.fall,
                        rise: raw.rise,
                    })
                    .collect_vec(),
            },
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ServiceDefinition {
    pub function_name: String,
    pub command: String,
    pub args: Vec<String>,
    pub interval: Duration,
    pub command_timeout: Duration,
    /// Number of consecutive failure to consider the service unhealthy
    pub fall: u32,
    /// Number of consecutive failure to consider the service healthy
    pub rise: u32,
}

#[derive(Debug)]
pub enum ServiceState {
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
    pub fn update_with(
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
