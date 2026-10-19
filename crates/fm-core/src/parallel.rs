//! Work spread over the CPU cores, results in input order.
//!
//! A first scan reads, hashes and parses every file, then turns every face
//! into its index row. Each item is independent of the others, so the work
//! runs on one thread per core (E2.8: the 10 s first-scan budget). The
//! caller sees the same results, in the same order, as a plain loop.
//!
//! Reading a file mostly waits for the disk. [`map_ordered_io`] runs that
//! work on more threads than cores, so the disk always has requests queued
//! (E2.9: on the macOS bench runner, three threads, one per core, took 4 to
//! 20 s to read the reference library that Linux read in 2 s).

use std::num::NonZero;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;

/// Below this many items a plain loop on the calling thread is faster than
/// starting threads.
const MIN_PARALLEL: usize = 16;

/// Threads per core for work that waits for the disk.
const IO_THREADS_PER_CORE: usize = 4;

/// The fewest threads for work that waits for the disk, however few cores
/// there are: a disk serves several requests at a time.
const MIN_IO_THREADS: usize = 16;

/// The most threads for work that waits for the disk. Each one holds the
/// file it reads, so this also bounds the memory a scan holds at once.
const MAX_IO_THREADS: usize = 32;

/// One thread per core.
fn cpu_threads() -> usize {
    thread::available_parallelism().map_or(1, NonZero::get)
}

/// Threads for work that waits for the disk on `cores` cores.
fn io_threads(cores: usize) -> usize {
    (cores * IO_THREADS_PER_CORE).clamp(MIN_IO_THREADS, MAX_IO_THREADS)
}

/// `items.iter().map(work).collect()`, run on up to one thread per core.
///
/// `progress` runs on the calling thread with the number of items done
/// after each one, so a scan job can send its events from its own thread.
/// The order of the results is the order of `items`, whatever order the
/// threads finish in. An item no thread took (a thread could not start)
/// runs on the calling thread.
pub(crate) fn map_ordered<T, R, F>(items: &[T], work: F, progress: &mut dyn FnMut(usize)) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync,
{
    map_on(cpu_threads(), items, work, progress)
}

/// [`map_ordered`] for work that mostly waits for the disk, such as reading
/// a file: on [`io_threads`] threads, more than there are cores.
pub(crate) fn map_ordered_io<T, R, F>(
    items: &[T],
    work: F,
    progress: &mut dyn FnMut(usize),
) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync,
{
    map_on(io_threads(cpu_threads()), items, work, progress)
}

/// [`map_ordered`] on up to `threads` threads.
fn map_on<T, R, F>(threads: usize, items: &[T], work: F, progress: &mut dyn FnMut(usize)) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync,
{
    let threads = threads.min(items.len());
    if threads <= 1 || items.len() < MIN_PARALLEL {
        return items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let r = work(item);
                progress(i + 1);
                r
            })
            .collect();
    }

    let mut results: Vec<Option<R>> = std::iter::repeat_with(|| None).take(items.len()).collect();
    let next = AtomicUsize::new(0);
    let mut done = 0;
    thread::scope(|scope| {
        let (tx, rx) = mpsc::channel();
        for _ in 0..threads {
            let tx = tx.clone();
            let (next, work) = (&next, &work);
            let started = thread::Builder::new()
                .name("fm-scan-worker".into())
                .spawn_scoped(scope, move || {
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(i) else { break };
                        if tx.send((i, work(item))).is_err() {
                            break;
                        }
                    }
                });
            if started.is_err() {
                break;
            }
        }
        drop(tx);
        for (i, r) in rx {
            results[i] = Some(r);
            done += 1;
            progress(done);
        }
    });

    items
        .iter()
        .zip(results)
        .map(|(item, r)| {
            r.unwrap_or_else(|| {
                let r = work(item);
                done += 1;
                progress(done);
                r
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn results_keep_the_input_order_whatever_order_threads_finish_in() {
        let items: Vec<u64> = (0..200).collect();
        // Early items take longest, so threads finish out of order.
        let out = map_ordered(
            &items,
            |&n| {
                thread::sleep(Duration::from_micros((200 - n) * 20));
                n * n
            },
            &mut |_| {},
        );
        assert_eq!(out, items.iter().map(|n| n * n).collect::<Vec<_>>());
    }

    #[test]
    fn progress_counts_up_once_per_item_on_the_calling_thread() {
        let items: Vec<u32> = (0..100).collect();
        let caller = thread::current().id();
        let mut seen = Vec::new();
        map_ordered(&items, |&n| n + 1, &mut |done| {
            assert_eq!(thread::current().id(), caller);
            seen.push(done);
        });
        assert_eq!(seen, (1..=100).collect::<Vec<_>>());
    }

    #[test]
    fn small_and_empty_inputs_run_inline() {
        let caller = thread::current().id();
        let out = map_ordered(&[1, 2, 3], |&n| (n, thread::current().id()), &mut |_| {});
        assert!(out.iter().all(|(_, id)| *id == caller));
        assert_eq!(out.iter().map(|(n, _)| *n).collect::<Vec<_>>(), [1, 2, 3]);
        let empty: Vec<u8> = map_ordered(&[] as &[u8], |&n| n, &mut |_| panic!("no items"));
        assert!(empty.is_empty());
    }

    #[test]
    fn large_inputs_use_more_than_one_thread() {
        if thread::available_parallelism().map_or(1, NonZero::get) < 2 {
            eprintln!("skipped: one core");
            return;
        }
        let items: Vec<u32> = (0..64).collect();
        let out = map_ordered(
            &items,
            |_| {
                thread::sleep(Duration::from_millis(2));
                thread::current().id()
            },
            &mut |_| {},
        );
        let mut ids = out.clone();
        ids.sort_by_key(|id| format!("{id:?}"));
        ids.dedup();
        assert!(ids.len() > 1, "{ids:?}");
    }

    /// A map under test: `map_ordered` or `map_ordered_io` on `usize` items.
    type Map = fn(&[usize], &(dyn Fn(&usize) -> usize + Sync), &mut dyn FnMut(usize)) -> Vec<usize>;

    /// The most items `map` ran at the same time, each waiting 5 ms as a read
    /// waits for the disk; checks the results on the way.
    fn peak_at_once(items: usize, map: Map) -> usize {
        let items: Vec<usize> = (0..items).collect();
        let (now, peak) = (AtomicUsize::new(0), AtomicUsize::new(0));
        let out = map(
            &items,
            &|&n| {
                let running = now.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(running, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(5));
                now.fetch_sub(1, Ordering::SeqCst);
                n * 2
            },
            &mut |_| {},
        );
        assert_eq!(out, items.iter().map(|n| n * 2).collect::<Vec<_>>());
        peak.into_inner()
    }

    #[test]
    fn disk_work_gets_more_threads_than_cores_within_bounds() {
        assert_eq!(io_threads(1), MIN_IO_THREADS);
        assert_eq!(io_threads(3), MIN_IO_THREADS);
        assert_eq!(io_threads(6), 6 * IO_THREADS_PER_CORE);
        assert_eq!(io_threads(64), MAX_IO_THREADS);
        assert!((1..MAX_IO_THREADS).all(|cores| io_threads(cores) > cores));
    }

    #[test]
    fn disk_work_overlaps_more_items_than_there_are_cores() {
        let cores = cpu_threads();
        let threads = io_threads(cores);
        let peak = peak_at_once(threads * 4, |i, w, p| map_ordered_io(i, w, p));
        assert!(peak > cores, "{peak} at once on {cores} cores");
        assert!(peak <= threads, "{peak} at once, {threads} threads");
    }

    #[test]
    fn cpu_work_stays_at_one_thread_per_core() {
        let cores = cpu_threads();
        let peak = peak_at_once(io_threads(cores) * 4, |i, w, p| map_ordered(i, w, p));
        assert!(peak <= cores, "{peak} at once on {cores} cores");
    }

    #[test]
    fn disk_work_keeps_the_order_and_counts_progress_on_the_caller() {
        let items: Vec<u64> = (0..300).collect();
        let caller = thread::current().id();
        let mut seen = 0;
        let out = map_ordered_io(
            &items,
            |&n| {
                thread::sleep(Duration::from_micros((300 - n) * 10));
                n + 7
            },
            &mut |done| {
                assert_eq!(thread::current().id(), caller);
                seen += 1;
                assert_eq!(done, seen);
            },
        );
        assert_eq!(seen, items.len());
        assert_eq!(out, items.iter().map(|n| n + 7).collect::<Vec<_>>());
    }
}
