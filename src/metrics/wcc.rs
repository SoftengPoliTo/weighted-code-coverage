use super::round_sd;

pub(crate) const WCC_COMPLEXITY_THRESHOLD: f64 = 15.;

#[inline]
pub(crate) fn wcc_function(complexity: f64, wcc_coverage: f64, ploc: f64) -> f64 {
    if complexity > WCC_COMPLEXITY_THRESHOLD {
        return 0.0;
    }

    wcc(wcc_coverage, ploc)
}

#[inline]
pub(crate) fn wcc(wcc_coverage: f64, ploc: f64) -> f64 {
    round_sd((wcc_coverage / ploc) * 100.0)
}
