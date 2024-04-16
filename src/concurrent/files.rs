use serde::Serialize;

use crate::metrics::MetricsThresholds;

use super::{functions::FunctionMetrics, Metrics, ProjectData};

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

impl FileMetrics {
    #[inline]
    pub(crate) fn new(
        name: String,
        project_data: ProjectData,
        metrics_thresholds: MetricsThresholds,
        functions: Option<Vec<FunctionMetrics>>,
    ) -> Self {
        Self {
            name,
            metrics: Metrics::file(project_data, metrics_thresholds),
            functions,
        }
    }
}
