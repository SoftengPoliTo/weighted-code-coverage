pub(crate) mod files;
pub(crate) mod functions;

use crossbeam::channel::{Receiver, Sender};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::error::*;

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
