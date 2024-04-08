use serde::Serialize;

use super::{functions::FunctionMetrics, Metrics};

/// File metrics.
#[derive(Debug, Clone, Serialize)]
pub struct FileMetrics {
    /// File name.
    pub name: String,
    /// File metrics.
    pub metrics: Metrics,
    /// File functions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub functions: Option<Vec<FunctionMetrics>>,
}
