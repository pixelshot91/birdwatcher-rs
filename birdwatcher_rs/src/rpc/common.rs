use crate::service::Bundle;

/// Trait that defines the RPC service. `birdwatcher-daemon` start the server and `birdwatcher-cli` interract with it.
/// 
/// Currently, it only has one method, `get_data`, that return the current state of the services.
/// It is expected to be extended in the future with more methods, for example to trigger a manual check of a service, to reset the hysteresis state, or to change the configuration on the fly.
#[tarpc::service]
pub trait Insight {
    async fn get_data() -> Bundle;
}
