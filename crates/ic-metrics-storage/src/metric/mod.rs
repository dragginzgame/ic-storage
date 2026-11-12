//! Private adapters to the core's canonical restored-state admission.

use ic_metrics::{
    MeasurementAggregate, MeasurementCounter, MeasurementHistogram, MeasurementSummary,
};

use serde::Serialize;

use crate::{MAX_HISTOGRAM_BOUNDS, StoreError, codec::MetricKind};

mod continuity;
mod state;
use state::{AggregateState, HistogramState};

pub trait StoredMetric: Sized + Eq {
    type Configuration: ?Sized;
    fn configuration(&self) -> &Self::Configuration;
    const KIND: MetricKind;
    fn same_state(&self, previous: &Self) -> bool {
        self == previous
    }
    fn continues(&self, previous: &Self) -> bool;
    type State: Serialize + serde::de::DeserializeOwned;
    const ADMISSION: Result<(), StoreError> = Ok(());
    fn state(&self) -> Self::State;
    fn restore(configuration: &Self::Configuration, state: Self::State)
    -> Result<Self, StoreError>;
}

impl StoredMetric for MeasurementCounter {
    fn continues(&self, previous: &Self) -> bool {
        continuity::counter(self, previous)
    }

    const KIND: MetricKind = MetricKind::Counter;
    type Configuration = ();

    fn configuration(&self) -> &() {
        &()
    }

    type State = (u128, bool);

    fn state(&self) -> Self::State {
        (self.total(), self.total_overflowed())
    }

    fn restore((): &(), (total, overflowed): Self::State) -> Result<Self, StoreError> {
        Ok(ic_metrics::persistence::restore_counter(total, overflowed)?)
    }
}

impl StoredMetric for MeasurementAggregate {
    fn continues(&self, previous: &Self) -> bool {
        continuity::aggregate(self, previous)
    }

    const KIND: MetricKind = MetricKind::Aggregate;
    type Configuration = ();

    fn configuration(&self) -> &() {
        &()
    }

    type State = AggregateState;

    fn state(&self) -> Self::State {
        (*self).into()
    }

    fn restore((): &(), state: Self::State) -> Result<Self, StoreError> {
        state.restore()
    }
}

impl StoredMetric for MeasurementSummary {
    fn continues(&self, previous: &Self) -> bool {
        continuity::summary(self, previous)
    }

    const KIND: MetricKind = MetricKind::Summary;
    type Configuration = ();

    fn configuration(&self) -> &() {
        &()
    }

    type State = (AggregateState, Option<u64>);

    fn state(&self) -> Self::State {
        (self.aggregate().into(), self.latest())
    }

    fn restore((): &(), (aggregate, latest): Self::State) -> Result<Self, StoreError> {
        Ok(ic_metrics::persistence::restore_summary(
            aggregate.restore()?,
            latest,
        )?)
    }
}

impl<const N: usize> StoredMetric for MeasurementHistogram<N> {
    fn continues(&self, previous: &Self) -> bool {
        continuity::histogram(self, previous)
    }

    const KIND: MetricKind = MetricKind::Histogram;
    type Configuration = [u64; N];

    fn configuration(&self) -> &[u64; N] {
        self.upper_bounds()
    }

    fn same_state(&self, previous: &Self) -> bool {
        // Tiny arrays already reject cheaply. Larger matching arrays are costly
        // when recording changes a bucket but leaves overflow unchanged.
        if N < 4 {
            self == previous
        } else {
            self.overflow() == previous.overflow()
                && self.aggregate() == previous.aggregate()
                && self.overflow_count_overflowed() == previous.overflow_count_overflowed()
                && self.bucket_overflowed() == previous.bucket_overflowed()
                && equal_words(self.upper_bounds(), previous.upper_bounds())
                && equal_words(self.bucket_counts(), previous.bucket_counts())
        }
    }

    type State = HistogramState;
    const ADMISSION: Result<(), StoreError> = if N > MAX_HISTOGRAM_BOUNDS {
        Err(StoreError::TooManyBounds)
    } else {
        Ok(())
    };

    fn state(&self) -> Self::State {
        HistogramState {
            aggregate: self.aggregate().into(),
            buckets: ic_wire::List::new(
                self.upper_bounds()
                    .iter()
                    .copied()
                    .zip(self.bucket_counts().iter().copied())
                    .zip(self.bucket_overflowed().iter().copied())
                    .map(|((bound, count), flag)| (bound, count, flag))
                    .collect(),
            )
            .expect("static histogram shape was admitted before serialization"),
            overflow: self.overflow(),
            overflowed: self.overflow_count_overflowed(),
        }
    }

    fn restore(
        configuration: &Self::Configuration,
        state: Self::State,
    ) -> Result<Self, StoreError> {
        if state.buckets.as_slice().len() != N {
            return Err(StoreError::BoundsMismatch);
        }
        let mut bounds = [0; N];
        let mut counts = [0; N];
        let mut flags = [false; N];
        for (index, &(bound, count, flag)) in state.buckets.as_slice().iter().enumerate() {
            bounds[index] = bound;
            counts[index] = count;
            flags[index] = flag;
        }
        let value = ic_metrics::persistence::restore_histogram(
            bounds,
            counts,
            state.overflow,
            flags,
            state.overflowed,
            state.aggregate.restore()?,
        )?;
        if !equal_words(value.upper_bounds(), configuration) {
            return Err(StoreError::BoundsMismatch);
        }
        Ok(value)
    }
}

// Compare numeric fields directly: Wasm's byte-oriented array equality is
// expensive on these larger arrays. Keep full-width equality and length checks.
fn equal_words(left: &[u64], right: &[u64]) -> bool {
    left.len() == right.len() && left.iter().zip(right).all(|(left, right)| left == right)
}
