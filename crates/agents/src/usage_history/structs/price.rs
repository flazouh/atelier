/// An estimated price: USD per million input and output tokens, and cache prices as a ratio of the input price.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Price {
    pub(crate) input: f64,
    pub(crate) output: f64,
    pub(crate) cache_read_ratio: f64,
    pub(crate) cache_write_ratio: f64,
}
