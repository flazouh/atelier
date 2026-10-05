use sysinfo::{MemoryRefreshKind, Pid, ProcessRefreshKind, ProcessesToUpdate, System};

use super::super::{structs::SysinfoProbe, traits::LoadProbe};

impl SysinfoProbe {
    pub fn new() -> Self {
        let mut system = System::new();
        // The first reading of the processor is of nothing; the one after it is of the second between.
        system.refresh_cpu_usage();
        Self { system, pid: sysinfo::get_current_pid().ok() }
    }
}

impl Default for SysinfoProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl LoadProbe for SysinfoProbe {
    fn cpu(&mut self) -> f32 {
        self.system.refresh_cpu_usage();
        self.system.global_cpu_usage() / 100.
    }

    fn memory(&mut self) -> (u64, u64) {
        self.system.refresh_memory_specifics(MemoryRefreshKind::new().with_ram());
        (self.system.used_memory(), self.system.total_memory())
    }

    fn app_memory(&mut self) -> Option<u64> {
        let pid: Pid = self.pid?;
        self.system.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), ProcessRefreshKind::new().with_memory());
        self.system.process(pid).map(sysinfo::Process::memory)
    }
}
