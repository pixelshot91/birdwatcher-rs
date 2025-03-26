/// The Config as it is written in the birdwatcher.conf
/// It differ from the `elaborated` Config below which use more precise types
///  - Use f32 instead of Duration to avoid having to create a `secs` and `nanos` entry for each duration in the TOML file
///  - Checks that `command` fields have at least one element, the arg0
mod raw {
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct Config {
        pub generated_file_path: String,
        pub bird_reload: BirdReload,
        pub service_definitions: Vec<ServiceDefinition>,
    }
    #[derive(Clone, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct BirdReload {
        pub command: Vec<String>,
        pub timeout_s: f32,
    }

    #[derive(Clone, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct ServiceDefinition {
        pub service_name: String,
        pub function_name: String,
        pub command: Vec<String>,
        pub interval_s: f32,
        pub command_timeout_s: f32,
        /// Number of consecutive failure to consider the service unhealthy
        pub fall: u32,
        /// Number of consecutive failure to consider the service healthy
        pub rise: u32,
    }
}

use anyhow::{Context, Result};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};

#[derive(Clone, Deserialize, Serialize)]
pub struct Config {
    pub generated_file_path: String,
    pub reload_command: String,
    pub reload_command_args: Vec<String>,
    pub reload_timeout: Duration,
    pub service_definitions: Vec<ServiceDefinition>,
}

impl Config {
    pub fn load_from_file(filepath: &Path) -> Result<Config> {
        let config_file_content = fs_err::read_to_string(filepath)
            .with_context(|| format!("Cannot read file {:?}", filepath))?;
        let raw_config: raw::Config = toml::from_str(&config_file_content)?;

        let (bird_reload_cmd, bird_reload_args) =
            raw_config.bird_reload.command.split_first().context("'bird_reload.command' should contain at least one element: the path to the executable to run")?;

        Ok(Config {
            generated_file_path: raw_config.generated_file_path,
            reload_command: bird_reload_cmd.to_owned(),
            reload_command_args: bird_reload_args.to_owned(),
            reload_timeout: Duration::from_secs_f32(raw_config.bird_reload.timeout_s),
            service_definitions: raw_config
                .service_definitions
                .into_iter()
                .map(|raw| {
                    raw.command.split_first().context(format!("'service_definitions.command' of service '{}' should contain at least one element: the path to the executable to run", raw.service_name))
                    .map(|(cmd, args)  | {
                        ServiceDefinition {
                            service_name: raw.service_name,
                            function_name: raw.function_name,
                            command: cmd.to_owned(),
                            args: args.to_owned(),
                            interval: Duration::from_secs_f32(raw.interval_s),
                            command_timeout: Duration::from_secs_f32(raw.command_timeout_s),
                            fall: raw.fall,
                            rise: raw.rise,
                        }
                    })

                    
                })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ServiceDefinition {
    /// Used for logs only
    pub service_name: String,
    /// The name of the generated `bird` function
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
    /// Handle the fall/rise mecanism where multiple success/failure must happen
    /// consecutivly to cause a state change
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
