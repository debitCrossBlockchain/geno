use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct MetricConfig{
    pub metric_server_address: String,
    pub public_metric_server_address: String,
}

pub const DEFAULT_METRIC_SERVER_ADDRESS: &str = "0.0.0.0:19661";
pub const DEFAULT_PUBLIC_METRIC_SERVER_ADDRESS: &str = "0.0.0.0:19551";

impl Clone for MetricConfig{
    fn clone(&self) -> Self {
        Self{
            metric_server_address: self.metric_server_address.clone(),
            public_metric_server_address: self.public_metric_server_address.clone(),
        }
    }
}

impl Default for MetricConfig {
    fn default() -> MetricConfig {
        MetricConfig {
            metric_server_address: DEFAULT_METRIC_SERVER_ADDRESS.to_string(),
            public_metric_server_address: DEFAULT_PUBLIC_METRIC_SERVER_ADDRESS.to_string(),
        }
    }
}