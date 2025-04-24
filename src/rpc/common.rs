#[tarpc::service]
pub trait Insight {
    /// Returns a greeting for name.
    async fn hello(name: String) -> String;
    // async fn get_time() -> String;
}
