/// This machine, read with `sysinfo`.
pub struct SysinfoProbe {
    pub(in super::super) system: sysinfo::System,
    pub(in super::super) pid: Option<sysinfo::Pid>,
}
