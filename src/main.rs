use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    fs::File,
    io::Write,
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::task::JoinSet;

// An IP range
#[derive(Clone)]
struct CIDR {}

#[derive(Clone)]
struct ServiceDefinition {
    function_name: String,
    command: String,
    prefixes: Option<Vec<CIDR>>,
    interval: Duration,
}

struct Service {
    def: ServiceDefinition,
    if_true: String,
    if_false: String,
    last_result: bool,
}

// struct ServiceResult {
//     function_name: String,
//     if_true: String,
//     if_false: String,
//     bird_return_value: bool,
// }

struct Config {
    generated_file_path: OsString,
}

#[tokio::main]
async fn main() {
    let config = Config {
        generated_file_path: "birdwatcher_generated.conf".into(),
    };

    let service_defintions = [ServiceDefinition {
        function_name: "match_true".to_string(),
        command: "/bin/true".to_string(),
        prefixes: None,
        interval: Duration::from_secs(1),
    }];

    let mut services: Vec<Service> = service_defintions
        .iter()
        .map(|def| Service {
            def: def.clone(),
            if_true: "true".to_string(),
            if_false: "false".to_string(),
            last_result: false,
        })
        .collect();

    let mut services: Arc<Mutex<Vec<Service>>> = Arc::new(Mutex::new(services));

    // let mut functions: HashMap<String, bool> = HashMap::new();
    // services.iter().for_each(|service| {
    //     functions.insert(service.function_name.clone(), false);
    // });

    // let (tx, mut rx) = tokio::sync::mpsc::channel(1);

    let mut join_set = JoinSet::new();

    let join_handles =
        service_defintions
            .into_iter()
            .enumerate()
            .for_each(|(service_nb, service_def)| {
                // let service_def = service.def.clone();
                // let service_def = service_def;
                // let service_nb = service_nb;
                let services = services.clone();
                let generated_file_path = config.generated_file_path.clone();
                join_set.spawn(async move {
                    loop {
                        std::print!(
                            "Regen function {}, Launching command {}",
                            service_def.function_name,
                            service_def.command
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
                                eprint!(
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

    print!("All services launched");
    if let Some(t) = join_set.join_next().await {
        eprint!("Task failed with {}", t.err().unwrap())
    }
}

fn write_bird_function(generated_file_path: &OsStr, services: &[Service]) {
    use itertools::Itertools;
    let content = services
        .iter()
        .map(|service| {
            let function_name = &service.def.function_name;
            let return_value = if service.last_result {
                &service.if_true
            } else {
                &service.if_false
            };
            format!("function {function_name} {{ return {return_value}; }}",)
        })
        .join("\n");

    let mut f = File::create(generated_file_path).unwrap();
    f.write_all(content.as_bytes()).unwrap();
}
