use serde::Serialize;

use crate::metrics::MetricsThresholds;

use super::{Metrics, SpaceData};

/// Function metrics.
#[derive(Debug, Clone, Serialize)]
pub struct FunctionMetrics {
    /// Function name.
    pub name: String,
    /// Function metrics.
    pub metrics: Metrics,
}

impl FunctionMetrics {
    #[inline]
    pub(crate) fn new(
        name: String,
        space_data: SpaceData,
        metrics_thresholds: MetricsThresholds,
    ) -> Self {
        Self {
            name,
            metrics: Metrics::function(space_data, metrics_thresholds),
        }
    }
}
