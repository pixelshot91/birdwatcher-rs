use crate::service::Bundle;

#[tarpc::service]
pub trait Insight {
    async fn get_data() -> Bundle;
}
