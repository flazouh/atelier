/// A small deterministic generator. Not random: the same seed gives the same run.
pub(super) struct Lcg(pub(super) u64);

impl Lcg {
    pub(super) fn next(&mut self, below: usize) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % below
    }
}
