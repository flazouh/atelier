/// A way to read how hard this machine works.
pub trait LoadProbe {
    /// The processor in use now, 0 to 1, over the time since the last call.
    fn cpu(&mut self) -> f32;

    /// Bytes of memory in use, and all there is.
    fn memory(&mut self) -> (u64, u64);

    /// Bytes this app holds, when the system says.
    fn app_memory(&mut self) -> Option<u64>;
}
