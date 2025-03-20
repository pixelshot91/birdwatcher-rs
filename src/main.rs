use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

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

#[tokio::main]
async fn main() {
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

    service_defintions
        .into_iter()
        .enumerate()
        .for_each(|(service_nb, service_def)| {
            // let service_def = service.def.clone();
            // let service_def = service_def;
            // let service_nb = service_nb;
            let services = services.clone();
            // let tx = tx.clone();
            tokio::spawn(async move {
                // let service_def = service_def;
                // let service_nb = service_nb;

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

                services.lock().unwrap()[service_nb].last_result = return_value;

                


                tokio::time::sleep(service_def.interval).await;
            });
        });
}
