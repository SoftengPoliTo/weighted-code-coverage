use serde::Serialize;

use super::Metrics;

/// Function metrics.
#[derive(Debug, Clone, Serialize)]
pub struct FunctionMetrics {
    /// Function name.
    pub name: String,
    /// Function metrics.
    pub metrics: Metrics,
}
