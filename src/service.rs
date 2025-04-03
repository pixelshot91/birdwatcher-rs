use serde::{Deserialize, Serialize};
use std::time::Duration;

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
