/// An estimated cost in dollars; a dash when the model has no price.
pub fn format_cost(cost: Option<f64>) -> String {
    match cost {
        None => "–".to_string(),
        Some(c) if c > 0. && c < 0.01 => "<$0.01".to_string(),
        Some(c) => format!("${c:.2}"),
    }
}
