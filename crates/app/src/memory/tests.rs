use super::*;

/// This process's resident memory, in kB, from /proc.
fn rss_kb() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let line = status.lines().find(|l| l.starts_with("VmRSS:")).unwrap();
    line.split_whitespace().nth(1).unwrap().parse().unwrap()
}

/// Memory a thread freed stays in its arena until it is given back. A 200-file review kept about 14 MB
/// after each close this way.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
#[test]
fn freed_memory_goes_back_to_the_system() {
    // 160 MB in small blocks, the way texts and hunks are, made on a thread of their own. One block in 64
    // stays, as a few long-lived things do, so the freed pages sit between kept ones and the heap cannot
    // shrink from its top.
    let kept_blocks = std::thread::spawn(|| {
        let blocks: Vec<Vec<u8>> = (0..40_000).map(|i| vec![i as u8; 4096]).collect();
        blocks.into_iter().step_by(64).collect::<Vec<_>>()
    })
    .join()
    .unwrap();
    let kept = rss_kb();
    trim_now();
    let after = rss_kb();
    std::hint::black_box(&kept_blocks);
    assert!(after + 50_000 < kept, "RSS {kept} kB before the trim and {after} kB after");
}
