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
}

struct Service {
    def: ServiceDefinition,
    last_result: bool,
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
        },
        ServiceDefinition {
            function_name: "match_false".to_string(),
            command: "/bin/false".to_string(),
            interval: Duration::from_secs(2),
        },
    ];

    let services: Vec<Service> = service_defintions
        .iter()
        .map(|def| Service {
            def: def.clone(),
            last_result: false,
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
                        services_lock[service_nb].last_result = return_value;
                        write_bird_function(&generated_file_path, services_lock.as_slice());
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
            let return_value = if service.last_result { "true" } else { "false" };
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
