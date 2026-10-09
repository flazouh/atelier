use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use crate::usage_history::consts::MAX_THREADS;

/// `f` on each item, on up to [`MAX_THREADS`] threads; the results keep the order of the items.
pub(crate) fn parallel_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let n = items.len();
    let threads = thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(MAX_THREADS).min(n);
    if threads <= 1 {
        return items.iter().map(f).collect();
    }
    let next = AtomicUsize::new(0);
    let mut indexed: Vec<(usize, R)> = thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut local = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(i) else { return local };
                        local.push((i, f(item)));
                    }
                })
            })
            .collect();
        workers.into_iter().flat_map(|w| w.join().unwrap_or_default()).collect()
    });
    indexed.sort_by_key(|(i, _)| *i);
    indexed.into_iter().map(|(_, r)| r).collect()
}
