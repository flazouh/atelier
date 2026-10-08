use crate::usage_history::consts::BUCKET_SECS;
use crate::usage_history::structs::{Buckets, Rec, Tokens};

impl Buckets {
    pub(crate) fn add(&mut self, ts_secs: i64, model: &str, tokens: Tokens) {
        let bucket = ts_secs.div_euclid(BUCKET_SECS);
        if !self.by_model.contains_key(model) {
            self.by_model.insert(model.to_owned(), Default::default());
        }
        if let Some(buckets) = self.by_model.get_mut(model) {
            *buckets.entry(bucket).or_default() += tokens;
        }
    }

    /// The records, sorted by bucket then model.
    pub(crate) fn into_records(self) -> Vec<Rec> {
        let mut out: Vec<Rec> = self
            .by_model
            .into_iter()
            .flat_map(|(model, buckets)| {
                buckets.into_iter().map(move |(bucket, tokens)| Rec { bucket, model: model.clone(), tokens })
            })
            .collect();
        out.sort_by(|a, b| a.bucket.cmp(&b.bucket).then_with(|| a.model.cmp(&b.model)));
        out
    }
}
