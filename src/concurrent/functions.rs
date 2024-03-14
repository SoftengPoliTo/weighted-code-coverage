use serde::Serialize;

use super::Metrics;

/// Metrics of a function.
#[derive(Debug, Clone, Serialize)]
pub struct FunctionMetrics {
    pub name: String,
    pub metrics: Metrics,
}

impl FunctionMetrics {
    pub(crate) fn new(name: String, metrics: Metrics) -> Self {
        Self { name, metrics }
    }
}