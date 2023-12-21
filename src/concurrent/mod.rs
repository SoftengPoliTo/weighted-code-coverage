pub(crate) mod files;
pub(crate) mod functions;

use core::cmp::Ordering;
use crossbeam::channel::{Receiver, Sender};
use rayon::prelude::*;
use rust_code_analysis::FuncSpace;
use serde::{Deserialize, Serialize};

use crate::{
    error::*,
    metrics::{
        crap::crap_function,
        get_covered_lines,
        skunk::skunk_nosmells_function,
        wcc::{wcc_plain_function, wcc_quantized_function},
    },
    Complexity,
};

// Defines a framework for a *producers-consumers-composer* pattern
// used to compute weighted code coverage.
pub(crate) trait Wcc {
    // Item sent from `producer` to `consumer`.
    type ProducerItem: Sync + Send;

    // Item sent from `consumer` to `composer`.
    type ConsumerItem: Sync + Send;

    // Output returned by the `composer`.
    type Output: Sync + Send;

    // Sends items to the `consumer`.
    //
    // * `sender` - `Sender` of the channel between `producer` and `consumer`.
    fn producer(&self, sender: Sender<Self::ProducerItem>) -> Result<()>;

    // Receivs items from the `producer`, processes them, and sends the results
    // to the `composer`.
    //
    // * `receiver` - `Receiver` of the channel between `producer` and `consumer`.
    // * `sender` - `Sender` of the channel between `consumer` and `composer`.
    fn consumer(
        &self,
        receiver: Receiver<Self::ProducerItem>,
        sender: Sender<Self::ConsumerItem>,
    ) -> Result<()>;

    // Receivs items from the `consumer`, computes an `Output`, and returns it.
    //
    // * `receiver` - `Receiver` of the channel between `consumer` and `composer`.
    fn composer(&self, receiver: Receiver<Self::ConsumerItem>) -> Result<Self::Output>;

    // Executes the *producers-consumers-composer* pattern.
    fn run(&self, n_threads: usize) -> Result<Self::Output>
    where
        Self: Sync,
    {
        let (producer_to_consumer_sender, producer_to_consumer_receiver) =
            crossbeam::channel::bounded(n_threads);
        let (consumer_to_composer_sender, consumer_to_composer_receiver) =
            crossbeam::channel::bounded(n_threads);

        match crossbeam::thread::scope(|scope| {
            // Producer
            scope.spawn(|_| self.producer(producer_to_consumer_sender));

            // Composer
            let composer = scope.spawn(|_| self.composer(consumer_to_composer_receiver));

            // Consumer
            (0..n_threads).into_par_iter().try_for_each(move |_| {
                self.consumer(
                    producer_to_consumer_receiver.clone(),
                    consumer_to_composer_sender.clone(),
                )
            })?;

            composer.join()?
        }) {
            Ok(output) => output,
            Err(e) => Err(e.into()),
        }
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct ConsumerOutputWcc {
    pub(crate) covered_lines: f64,
    pub(crate) total_lines: f64,
    pub(crate) wcc_plain_sum: f64,
    pub(crate) wcc_quantized_sum: f64,
    pub(crate) ploc_sum: f64,
    pub(crate) comp_sum: f64,
}

impl ConsumerOutputWcc {
    fn update(&mut self, update: Self) {
        self.covered_lines += update.covered_lines;
        self.total_lines += update.total_lines;
        self.wcc_plain_sum += update.wcc_plain_sum;
        self.wcc_quantized_sum += update.wcc_quantized_sum;
        self.ploc_sum += update.ploc_sum;
        self.comp_sum += update.comp_sum;
    }
}

// Struct containing all the metrics
#[derive(Clone, Default, Debug, Serialize, Deserialize, Copy, PartialEq)]
pub(crate) struct Metrics {
    pub(crate) wcc_plain: f64,
    pub(crate) wcc_quantized: f64,
    pub(crate) crap: f64,
    pub(crate) skunk: f64,
    pub(crate) is_complex: bool,
    pub(crate) coverage: f64,
}

impl Metrics {
    pub(crate) fn new(
        wcc_plain: f64,
        wcc_quantized: f64,
        crap: f64,
        skunk: f64,
        is_complex: bool,
        coverage: f64,
    ) -> Self {
        Self {
            wcc_plain,
            wcc_quantized,
            crap,
            skunk,
            is_complex,
            coverage,
        }
    }

    pub(crate) fn min() -> Self {
        Self {
            wcc_plain: f64::MAX,
            wcc_quantized: f64::MAX,
            crap: f64::MAX,
            skunk: f64::MAX,
            is_complex: false,
            coverage: 100.0,
        }
    }

    pub(crate) fn wcc_plain(mut self, wcc_plain: f64) -> Self {
        self.wcc_plain = wcc_plain;
        self
    }

    pub(crate) fn wcc_quantized(mut self, wcc_quantized: f64) -> Self {
        self.wcc_quantized = wcc_quantized;
        self
    }

    pub(crate) fn crap(mut self, crap: f64) -> Self {
        self.crap = crap;
        self
    }

    pub(crate) fn skunk(mut self, skunk: f64) -> Self {
        self.skunk = skunk;
        self
    }

    pub(crate) fn coverage(mut self, coverage: f64) -> Self {
        self.coverage = coverage;
        self
    }
}

const COMPLEXITY_FACTOR: f64 = 25.0;

pub(crate) trait Visit {
    fn get_metrics_from_space(
        space: &FuncSpace,
        covs: &[Option<i32>],
        metric: Complexity,
        coverage: Option<f64>,
        thresholds: &[f64],
    ) -> Result<(Metrics, (f64, f64))>;
}

pub(crate) struct Tree;

impl Visit for Tree {
    fn get_metrics_from_space(
        space: &FuncSpace,
        covs: &[Option<i32>],
        metric: Complexity,
        coverage: Option<f64>,
        thresholds: &[f64],
    ) -> Result<(Metrics, (f64, f64))> {
        let (wcc_plain, sp_sum) = wcc_plain_function(space, covs, metric)?;
        let (wcc_quantized, sq_sum) = wcc_quantized_function(space, covs, metric)?;
        let crap = crap_function(space, covs, metric, coverage)?;
        let skunk = skunk_nosmells_function(space, covs, metric, coverage)?;
        let is_complex = check_complexity(wcc_plain, wcc_quantized, crap, skunk, thresholds);
        let coverage = if let Some(coverage) = coverage {
            coverage
        } else {
            let (covl, tl) = get_covered_lines(covs, space.start_line, space.end_line)?;
            if tl == 0.0 {
                0.0
            } else {
                (covl / tl) * 100.0
            }
        };
        let m = Metrics::new(
            wcc_plain,
            wcc_quantized,
            crap,
            skunk,
            is_complex,
            f64::round(coverage * 100.0) / 100.0,
        );
        Ok((m, (sp_sum, sq_sum)))
    }
}

#[inline(always)]
#[allow(dead_code)]
pub(crate) fn compare_float(a: f64, b: f64) -> bool {
    a.total_cmp(&b) == Ordering::Equal
}

// Check complexity of a metric
// Return true if al least one metric exceed a threshold , false otherwise
#[inline(always)]
fn check_complexity(
    wcc_plain: f64,
    wcc_quantized: f64,
    crap: f64,
    skunk: f64,
    thresholds: &[f64],
) -> bool {
    wcc_plain > thresholds[0]
        || wcc_quantized > thresholds[1]
        || crap > thresholds[2]
        || skunk > thresholds[3]
}

// GET average, maximum and minimum given all the metrics
pub(crate) fn get_cumulative_values(metrics: &Vec<Metrics>) -> (Metrics, Metrics, Metrics) {
    let mut min = Metrics::min();
    let mut max = Metrics::default();
    let (wcc, wccq, crap, skunk, cov) = metrics.iter().fold((0.0, 0.0, 0.0, 0.0, 0.0), |acc, m| {
        max.wcc_plain = max.wcc_plain.max(m.wcc_plain);
        max.wcc_quantized = max.wcc_quantized.max(m.wcc_quantized);
        max.crap = max.crap.max(m.crap);
        max.skunk = max.skunk.max(m.skunk);
        min.wcc_plain = min.wcc_plain.min(m.wcc_plain);
        min.wcc_quantized = min.wcc_quantized.min(m.wcc_quantized);
        min.crap = min.crap.min(m.crap);
        min.skunk = min.skunk.min(m.skunk);
        (
            acc.0 + m.wcc_plain,
            acc.1 + m.wcc_quantized,
            acc.2 + m.crap,
            acc.3 + m.skunk,
            acc.4 + m.coverage,
        )
    });
    let l = metrics.len() as f64;
    let avg = Metrics::new(wcc / l, wccq / l, crap / l, skunk / l, false, cov);
    (avg, max, min)
}

// Calculate WCC PLAIN , WCC QUANTIZED, CRA and SKUNKSCORE for the entire project
// Using the sum values computed before
pub(crate) fn get_project_metrics(
    values: ConsumerOutputWcc,
    project_coverage: Option<f64>,
) -> Result<Metrics> {
    let project_coverage = if let Some(cov) = project_coverage {
        cov
    } else if values.total_lines != 0.0 {
        (values.covered_lines / values.total_lines) * 100.0
    } else {
        0.0
    };
    let mut m = Metrics::default();
    m = m.wcc_plain(values.wcc_plain_sum / values.ploc_sum);
    m = m.wcc_quantized(values.wcc_quantized_sum / values.ploc_sum);
    m = m.crap(
        ((values.comp_sum.powf(2.)) * ((1.0 - project_coverage / 100.).powf(3.))) + values.comp_sum,
    );
    m = m.skunk((values.comp_sum / COMPLEXITY_FACTOR) * (100. - (project_coverage)));
    m = m.coverage(project_coverage);
    Ok(m)
}
