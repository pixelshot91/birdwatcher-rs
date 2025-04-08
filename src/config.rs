/// The Config as it is written in the birdwatcher.conf
/// It differ from the `elaborated` Config below which use more precise types
///  - Use f32 instead of Duration to avoid having to create a `secs` and `nanos` entry for each duration in the TOML file
///  - Checks that `command` fields have at least one element, the arg0
mod raw {
    use serde::Deserialize;

    use crate::deser::duration_deser_f32::DurationDeserF32;

    #[derive(Clone, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Config {
        pub generated_file: GeneratedFile,
        pub bird_reload: BirdReload,
        pub service_definitions: Vec<ServiceDefinition>,
    }

    #[derive(Clone, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct GeneratedFile {
        /// This file will be overriten by birdwatcher-rs each time a service change its state
        pub path: String,
        /// Add the return type of generated functions, which has been introduced in BIRD 2.14
        /// True by default
        /// Turn it off if you use Bird less than 2.14
        pub function_return_type: Option<bool>,
    }

    #[derive(Clone, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct BirdReload {
        pub command: Vec<String>,
        pub timeout_s: DurationDeserF32,
    }

    #[derive(Clone, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct ServiceDefinition {
        /// Informationnal string to describe the service
        pub service_name: String,
        /// This is the BIRD function that you should call in you bird.conf
        pub function_name: String,
        pub command: Vec<String>,
        pub interval_s: DurationDeserF32,
        pub command_timeout_s: DurationDeserF32,
        /// Number of consecutive failure to consider the service unhealthy
        pub fall: u32,
        /// Number of consecutive failure to consider the service healthy
        pub rise: u32,
    }
}

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};

use crate::service::ServiceDefinition;

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(PartialEq, Debug))]
pub struct GeneratedFile {
    pub path: String,
    pub function_return_type: bool,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Config {
    pub generated_file: GeneratedFile,
    pub reload_command: String,
    pub reload_command_args: Vec<String>,
    pub reload_timeout: Duration,
    pub service_definitions: Vec<ServiceDefinition>,
}

impl Config {
    pub fn load_from_file(filepath: &Path) -> Result<Config> {
        let config_file_content = fs_err::read_to_string(filepath)
            .with_context(|| format!("Cannot read file {:?}", filepath))?;
        Config::from_string(config_file_content)
    }

    fn from_string(str: String) -> Result<Config> {
        let raw_config: raw::Config = toml::from_str(&str)?;

        let (bird_reload_cmd, bird_reload_args) =
            raw_config.bird_reload.command.split_first().context("'bird_reload.command' should contain at least one element: the path to the executable to run")?;

        Ok(Config {
            generated_file: GeneratedFile { path: raw_config.generated_file.path, function_return_type: raw_config.generated_file.function_return_type.unwrap_or(true) },
            reload_command: bird_reload_cmd.to_owned(),
            reload_command_args: bird_reload_args.to_owned(),
            reload_timeout: raw_config.bird_reload.timeout_s.into(),
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
                            interval: raw.interval_s.into(),
                            command_timeout: raw.command_timeout_s.into(),
                            fall: raw.fall,
                            rise: raw.rise,
                        }
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use crate::{config::GeneratedFile, service::ServiceDefinition};

    use super::Config;
    use indoc::indoc;
    use pretty_assertions::assert_eq;

    #[test]
    fn empty_config_should_fail() {
        assert!(Config::from_string("".into()).is_err())
    }
    #[test]
    fn one_service() {
        let config = Config::from_string(
            r#"
[generated_file]
path = "birdwatcher_generated.conf"

[bird_reload]
command = ["birdc", "configure"]
timeout_s = 1

[[service_definitions]]
service_name = "first_service"
function_name = "match_true"
command = ["/bin/ls", "myfile.txt"]
command_timeout_s = 2
interval_s = 3
fall = 4
rise = 5
"#
            .to_owned(),
        )
        .unwrap();
        assert_eq!(
            config.generated_file,
            GeneratedFile {
                path: "birdwatcher_generated.conf".to_owned(),
                function_return_type: true
            }
        );
        assert_eq!(config.reload_command, "birdc");
        assert_eq!(config.reload_command_args, ["configure"]);
        assert_eq!(config.reload_timeout, Duration::from_secs(1));

        assert_eq!(config.service_definitions.len(), 1);
        assert_eq!(
            config.service_definitions,
            vec![ServiceDefinition {
                service_name: "first_service".to_owned(),
                function_name: "match_true".to_owned(),
                command: "/bin/ls".to_owned(),
                args: vec!["myfile.txt".to_owned()],
                command_timeout: Duration::from_secs(2),
                interval: Duration::from_secs(3),
                fall: 4,
                rise: 5,
            },]
        );
    }

    #[test]
    fn example_config_works() {
        let config =
            Config::load_from_file(std::path::Path::new("example/birdwatcher.conf")).unwrap();
        assert_eq!(config.service_definitions.len(), 2);
    }

    #[test]
    fn unknown_field_should_fail() {
        let config = Config::from_string(
            r#"
[generated_file]
path = "birdwatcher_generated.conf"

[bird_reload]
command = ["birdc", "configure"]
timeout_s = 2

[[service_definitions]]
service_name = "first_service"
function_name = "match_true"
command = ["/bin/ls", "1"]
command_timeout_s = 1
interval_s = 1.2
fall = 1
rise = 3
raise = 4
"#
            .to_owned(),
        );
        assert!(config.is_err());
        let e = config.err().unwrap();

        assert_eq!(
            e.to_string(),
            indoc! { r#"
            TOML parse error at line 17, column 1
               |
            17 | raise = 4
               | ^^^^^
            unknown field `raise`, expected one of `service_name`, `function_name`, `command`, `interval_s`, `command_timeout_s`, `fall`, `rise`
            "# }
        );
    }

    #[test]
    fn missing_field_should_fail() {
        let config = Config::from_string(
            r#"
[generated_file]
path = "birdwatcher_generated.conf"

[bird_reload]
command = ["birdc", "configure"]
timeout_s = 2

[[service_definitions]]
service_name = "first_service"
function_name = "match_true"
command = ["/bin/ls", "1"]
command_timeout_s = 1
interval_s = 1.2
fall = 1
"#
            .to_owned(),
        );
        assert!(config.is_err());
        let e = config.err().unwrap();

        assert_eq!(
            e.to_string(),
            indoc! { r#"
                TOML parse error at line 9, column 1
                  |
                9 | [[service_definitions]]
                  | ^^^^^^^^^^^^^^^^^^^^^^^
                missing field `rise`
             "# }
        );
    }
}
