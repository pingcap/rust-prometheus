use crate::proto::LabelPair;
//use crate::timer;
use std::collections::HashMap;

// OpenMetrics require unix epoch timestamps
// https://github.com/OpenObservability/OpenMetrics/blob/main/specification/OpenMetrics.md#timestamps-2
fn epoch_timestamp() -> f64 {
    use std::time::SystemTime;
    let d = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
    let nanos = f64::from(d.subsec_nanos()) / 1e9;
    d.as_secs() as f64 + nanos
}

/// An OpenMetrics Exemplar
///
/// https://github.com/OpenObservability/OpenMetrics/blob/master/specification/OpenMetrics.md#exemplars
#[derive(Debug, Clone)]
pub struct Exemplar {
    pub(crate) value: f64,
    pub(crate) labels: Vec<LabelPair>,
    pub(crate) timestamp_epoch: f64,
}

impl Exemplar {
    /// Create an ['Exemplar'] with value
    pub fn new(val: f64) -> Self {
        println!("making exemplar of you {}", epoch_timestamp());
        Self {
            value: val,
            labels: vec![],
            timestamp_epoch: epoch_timestamp(),
        }
    }

    /// Create an ['Exemplar'] with value and labels
    pub fn new_with_labels(val: f64, exemplar_labels: HashMap<String, String>) -> Self {
        let mut label_pairs = Vec::with_capacity(exemplar_labels.len());
        // TODO: verify length of labelset + values as <= 128 UTF8 chars
        for (n, v) in exemplar_labels.iter() {
            let mut label_pair = LabelPair::default();
            label_pair.set_name(n.to_string());
            label_pair.set_value(v.to_string());
            label_pairs.push(label_pair);
        }

        println!("making exemplar of you2 {}", epoch_timestamp());
        Self {
            value: val,
            labels: label_pairs,
            timestamp_epoch: epoch_timestamp()
        }
    }
}
