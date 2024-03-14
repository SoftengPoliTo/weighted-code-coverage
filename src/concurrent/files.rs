use serde::Serialize;

use super::{functions::FunctionMetrics, Metrics};

/// Metrics of a file.
#[derive(Debug, Clone, Serialize)]
pub struct FileMetrics {
    pub name: String,
    pub metrics: Metrics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub functions: Option<Vec<FunctionMetrics>>,
}

impl FileMetrics {
    pub(crate) fn new(name: String, metrics: Metrics, functions: Option<Vec<FunctionMetrics>>) -> Self {
        Self {
            name,
            metrics,
            functions,
        }
    }
}