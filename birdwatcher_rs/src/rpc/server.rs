use crate::{
    config::Config,
    rpc::common::Insight,
    service::{Bundle, ServiceState},
};

use std::{ops::Deref, sync::Arc};
use tarpc::context;

#[derive(Clone)]
pub struct InsightServer {
    pub service_states: Arc<std::sync::Mutex<Vec<ServiceState>>>,
    pub config: Arc<Config>,
}

impl Insight for InsightServer {
    async fn get_data(self, _: context::Context) -> Bundle {
        Bundle {
            config: self.config.deref().clone(),
            service_states: self.service_states.lock().unwrap().clone(),
        }
    }
}
