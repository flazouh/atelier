use crate::usage_history::consts::{PER_MILLION, PRICES};
use crate::usage_history::structs::{Price, Tokens};

fn price_for(model: &str) -> Option<&'static Price> {
    let lower = model.to_ascii_lowercase();
    PRICES.iter().find(|(family, _)| lower.contains(family)).map(|(_, price)| price)
}

/// The estimated cost in USD of these tokens on this model; `None` when the model has no price. No tokens cost
/// nothing, whatever the model.
pub fn estimate_cost_usd(model: &str, tokens: &Tokens) -> Option<f64> {
    cost_usd(model, tokens)
}

pub(crate) fn cost_usd(model: &str, t: &Tokens) -> Option<f64> {
    if t.total() == 0 {
        return Some(0.0);
    }
    let p = price_for(model)?;
    let input = t.input as f64 * p.input
        + t.cache_read as f64 * p.input * p.cache_read_ratio
        + t.cache_write as f64 * p.input * p.cache_write_ratio;
    Some((input + t.output as f64 * p.output) / PER_MILLION)
}

/// The sum of costs; `None` as soon as one is unknown.
pub(crate) fn sum_cost(mut costs: impl Iterator<Item = Option<f64>>) -> Option<f64> {
    costs.try_fold(0.0, |acc, c| c.map(|c| acc + c))
}
