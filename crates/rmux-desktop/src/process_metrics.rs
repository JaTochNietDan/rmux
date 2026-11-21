//! Manual, PID-filtered process sampling. Never reads command lines or environment.
use std::{collections::HashMap, time::Instant};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

#[derive(Clone, Copy, serde::Serialize)]
pub struct Sample {
    pub resident_bytes: u64,
    pub cpu_percent: Option<f32>,
    pub start_time_seconds: u64,
}

#[derive(Clone, Copy, serde::Serialize)]
pub struct TreeSample {
    pub resident_bytes: u64,
    pub cpu_percent: Option<f32>,
    pub sampled_processes: usize,
    pub expected_processes: usize,
    pub complete: bool,
}

#[derive(Default)]
pub struct Sampler {
    system: System,
    pids: Vec<Pid>,
    baseline: Option<Instant>,
    identities: HashMap<Pid, u64>,
}

impl Sampler {
    #[cfg(test)]
    pub fn sample(&mut self, pids: impl IntoIterator<Item = u32>) -> HashMap<u32, Sample> {
        self.sample_bounded(pids, 513)
    }

    pub fn sample_with_trees(&mut self, pids: impl IntoIterator<Item = u32>, roots: impl IntoIterator<Item = u32>, canceled: &std::sync::atomic::AtomicBool) -> (HashMap<u32, Sample>, HashMap<u32, TreeSample>) {
        use crate::process_tree::{process_snapshot, TreeIndex, stable_index, ensure_stable_root};
        use std::collections::BTreeSet;
        let stopped = || canceled.load(std::sync::atomic::Ordering::Relaxed);
        if stopped() { return Default::default(); }
        let before = process_snapshot();
        let mut selected: BTreeSet<_> = pids.into_iter().take(513).collect();
        let mut trees = HashMap::new();
        let index = TreeIndex::new(before.iter().map(|(pid, identity)| (*pid, identity.parent)));
        for root in roots.into_iter().take(256) {
            if stopped() { return Default::default(); }
            let Ok(owned) = index.descendants(root) else { continue; };
            let ordered: BTreeSet<_> = owned.iter().copied().collect();
            for pid in ordered { if selected.len() == 4096 { break; } selected.insert(pid); }
            trees.insert(root, owned);
        }
        if stopped() { return Default::default(); }
        let samples = self.sample_bounded(selected, 4096);
        if stopped() { return Default::default(); }
        let after = process_snapshot();
        let stable_trees = stable_index(&before, &after);
        let current_trees = TreeIndex::new(after.iter().map(|(pid, identity)| (*pid, identity.parent)));
        let mut totals = HashMap::new();
        for (root, owned) in trees {
            if stopped() { return Default::default(); }
            if ensure_stable_root(root, &before, &after).is_err() { continue; }
            let Ok(stable) = stable_trees.descendants(root) else { continue; };
            let Ok(current) = current_trees.descendants(root) else { continue; };
            totals.insert(root, aggregate_owned_tree(&owned, &current, &stable, &samples, &before));
        }
        (samples, totals)
    }

    fn sample_bounded(&mut self, pids: impl IntoIterator<Item = u32>, limit: usize) -> HashMap<u32, Sample> {
        // Direct diagnostics use 513 slots; tree diagnostics share 4096 slots.
        let mut pids: Vec<_> = pids.into_iter().take(limit).map(Pid::from_u32).collect();
        pids.sort_unstable();
        pids.dedup();
        // Reset on membership change so the process cache cannot grow as tabs
        // churn. This also resets CPU baselines instead of inventing a delta.
        if pids != self.pids {
            self.system = System::new();
            self.identities.clear();
            self.baseline = None;
            self.pids = pids;
        }
        if self.pids.is_empty() {
            return HashMap::new();
        }
        let now = Instant::now();
        let spaced = self
            .baseline
            .is_some_and(|then| now.duration_since(then) >= sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        let update_cpu = self.baseline.is_none() || spaced;
        let mut kind = ProcessRefreshKind::new().with_memory();
        if update_cpu {
            kind = kind.with_cpu();
        }
        // Enumerate the host process table once for the normal case. Repeating
        // that enumeration per shell is expensive on macOS and Windows.
        let complete = self.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&self.pids), kind,
        ) == self.pids.len();
        let mut samples = HashMap::new();
        for pid in &self.pids {
            // Filtered refresh retains dead entries. When the batch is partial,
            // probe each PID to exclude those entries. Do not refresh CPU twice:
            // doing so would replace its interval with a near-zero interval.
            if !complete && self
                .system
                .refresh_processes_specifics(ProcessesToUpdate::Some(&[*pid]), ProcessRefreshKind::new().with_memory())
                == 0
            {
                self.identities.remove(pid);
                continue;
            }
            let Some(process) = self.system.process(*pid) else {
                continue;
            };
            let start = process.start_time();
            let cpu = process.cpu_usage();
            let cpu_percent =
                (complete && spaced && self.identities.get(pid) == Some(&start) && cpu.is_finite())
                    .then_some(cpu);
            samples.insert(
                pid.as_u32(),
                Sample {
                    resident_bytes: process.memory(),
                    cpu_percent,
                    start_time_seconds: start,
                },
            );
            if update_cpu {
                self.identities.insert(*pid, start);
            }
        }
        if !complete {
            // Backends may advance their shared clock during the fallback.
            // Establish a fresh baseline before reporting another CPU delta.
            self.baseline = None;
        } else if update_cpu {
            self.baseline = Some(now);
        }
        samples
    }
}

fn aggregate_owned_tree(
    owned: &std::collections::HashSet<u32>,
    current: &std::collections::HashSet<u32>,
    stable: &std::collections::HashSet<u32>,
    samples: &HashMap<u32, Sample>,
    before: &HashMap<u32, crate::process_tree::Identity>,
) -> TreeSample {
    // Include arrivals as missing samples, not as silently omitted descendants.
    // Departures remain expected too; only unchanged ancestry contributes RSS.
    let expected = owned.len() + current.difference(owned).count();
    let valid = owned.iter().filter(|pid| stable.contains(pid)).filter_map(|pid| {
        samples.get(pid).filter(|sample| before.get(pid).is_some_and(|identity| identity.started == sample.start_time_seconds))
    });
    aggregate_tree(expected, valid)
}

fn aggregate_tree<'a>(expected: usize, samples: impl Iterator<Item = &'a Sample>) -> TreeSample {
    let mut result = TreeSample { resident_bytes: 0, cpu_percent: Some(0.), sampled_processes: 0, expected_processes: expected, complete: false };
    for sample in samples {
        result.sampled_processes += 1;
        result.resident_bytes = result.resident_bytes.saturating_add(sample.resident_bytes);
        result.cpu_percent = result.cpu_percent.zip(sample.cpu_percent).map(|(sum, cpu)| sum + cpu).filter(|cpu| cpu.is_finite());
    }
    result.complete = result.sampled_processes == expected;
    if !result.complete { result.cpu_percent = None; }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changing_tree_membership_withholds_complete_totals() {
        use crate::process_tree::{Identity, stable_index};
        let root = Identity { parent: None, started: 1 };
        let child = Identity { parent: Some(10), started: 2 };
        let before = HashMap::from([(10, root), (20, child)]);
        let owned = [10,20].into_iter().collect();
        let samples = HashMap::from([
            (10, Sample { resident_bytes: 100, cpu_percent: Some(1.), start_time_seconds: 1 }),
            (20, Sample { resident_bytes: 200, cpu_percent: Some(2.), start_time_seconds: 2 }),
        ]);
        let check = |after: HashMap<u32, Identity>| {
            let current = crate::process_tree::TreeIndex::new(after.iter().map(|(pid, value)| (*pid, value.parent))).descendants(10).unwrap();
            let stable = stable_index(&before, &after).descendants(10).unwrap();
            aggregate_owned_tree(&owned, &current, &stable, &samples, &before)
        };
        let mut after = before.clone();
        after.insert(30, Identity { parent: Some(20), started: 3 });
        let arrival = check(after);
        assert!(!arrival.complete);
        assert_eq!((arrival.sampled_processes, arrival.expected_processes, arrival.resident_bytes), (2,3,300));
        assert_eq!(arrival.cpu_percent, None);
        let departure = check(HashMap::from([(10, root)]));
        assert!(!departure.complete);
        assert_eq!((departure.sampled_processes, departure.expected_processes, departure.resident_bytes), (1,2,100));
        let replaced = check(HashMap::from([(10,root), (20, Identity { started: 4, ..child })]));
        assert!(!replaced.complete);
        assert_eq!(replaced.resident_bytes, 100);
        assert_eq!(replaced.cpu_percent, None);
        let unchanged = check(before.clone());
        assert!(unchanged.complete);
        assert_eq!(unchanged.cpu_percent, Some(3.));
    }

    #[test]
    fn tree_totals_require_complete_cpu_baselines() {
        let first = Sample { resident_bytes: 10, cpu_percent: Some(12.), start_time_seconds: 1 };
        let second = Sample { resident_bytes: 20, cpu_percent: Some(34.), start_time_seconds: 2 };
        let total = aggregate_tree(2, [&first, &second].into_iter());
        assert!(total.complete);
        assert_eq!(total.resident_bytes, 30);
        assert_eq!(total.cpu_percent, Some(46.));
        let partial = aggregate_tree(3, [&first, &second].into_iter());
        assert!(!partial.complete);
        assert_eq!(partial.sampled_processes, 2);
        assert_eq!(partial.cpu_percent, None);
        let unready = Sample { cpu_percent: None, ..second };
        assert_eq!(aggregate_tree(2, [&first, &unready].into_iter()).cpu_percent, None);
    }
    #[cfg(unix)]
    #[test]
    fn shell_tree_samples_live_child_and_honors_cancellation() {
        use std::io::BufRead;
        struct Child(std::process::Child);
        impl Drop for Child {
            fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); }
        }
        let canceled = std::sync::atomic::AtomicBool::new(true);
        assert!(Sampler::default().sample_with_trees([0], [0], &canceled).0.is_empty());
        // A foreground child blocks on a pipe until the test closes stdin.
        let mut shell = Child(std::process::Command::new("/bin/sh")
            .args(["-c", "sh -c 'echo ready; read line'; :"])
            .stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap());
        let mut ready = String::new();
        std::io::BufReader::new(shell.0.stdout.take().unwrap()).read_line(&mut ready).unwrap();
        assert_eq!(ready.trim(), "ready");
        let (_, trees) = Sampler::default().sample_with_trees([shell.0.id()], [shell.0.id()], &std::sync::atomic::AtomicBool::new(false));
        drop(shell.0.stdin.take());
        shell.0.wait().unwrap();
        let tree = trees.get(&shell.0.id()).expect("live shell tree");
        assert!(tree.complete);
        assert!(tree.sampled_processes >= 2);
        assert!(tree.resident_bytes > 0);
        assert!(tree.cpu_percent.is_none());
    }
    /// Explicit diagnostic benchmark, excluded from ordinary test runs.
    #[cfg(unix)]
    #[test]
    #[ignore = "manual process sampling benchmark"]
    fn sampling_scaling() { sampling_baseline(false, 0); }

    #[cfg(unix)]
    #[test]
    #[ignore = "manual process tree sampling benchmark"]
    fn tree_sampling_scaling() { sampling_baseline(true, 0); }

    #[cfg(unix)]
    #[test]
    #[ignore = "manual process tree benchmark with child processes"]
    fn tree_children_sampling_scaling() { sampling_baseline(true, 4); }

    #[cfg(unix)]
    fn sampling_baseline(trees: bool, children: usize) {
        use std::io::Read;
        struct Shells(Vec<std::process::Child>);
        impl Drop for Shells {
            fn drop(&mut self) {
                for child in &mut self.0 {
                    // Closing stdin lets the fixture's trap reap its own children.
                    drop(child.stdin.take());
                    let _ = child.wait();
                }
            }
        }
        for count in [1, 8, 32] {
            let mut shells = Shells(Vec::new());
            for _ in 0..count {
                // Child PID list is private to this fixture. The EXIT trap both
                // terminates and reaps them, including on assertion unwind.
                let script = format!("kids=''; trap 'for pid in $kids; do kill $pid 2>/dev/null; done; wait' EXIT; i=0; while [ $i -lt {children} ]; do sleep 60 & kids=\"$kids $!\"; i=$((i+1)); done; printf R; read line");
                let child = std::process::Command::new("/bin/sh")
                    .args(["-c", &script])
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::null())
                    .spawn().unwrap();
                shells.0.push(child);
                let mut ready = [0];
                shells.0.last_mut().unwrap().stdout.as_mut().unwrap().read_exact(&mut ready).unwrap();
                assert_eq!(ready, [b'R']);
            }
            let pids: Vec<_> = std::iter::once(std::process::id())
                .chain(shells.0.iter().map(|child| child.id())).collect();
            let mut sampler = Sampler::default();
            let started = Instant::now();
            let canceled = std::sync::atomic::AtomicBool::new(false);
            let sample = |sampler: &mut Sampler| {
                if trees {
                    let (samples, totals) = sampler.sample_with_trees(pids.iter().copied(), pids.iter().copied().skip(1), &canceled);
                    assert_eq!(totals.len(), count);
                    assert!(totals.values().all(|tree| tree.complete && tree.sampled_processes == children + 1));
                    samples
                } else { sampler.sample(pids.iter().copied()) }
            };
            let first = sample(&mut sampler);
            let cold_us = started.elapsed().as_micros();
            assert_eq!(first.len(), count * (children + 1) + 1);
            assert!(first.values().all(|sample| sample.cpu_percent.is_none()));
            let mut times = Vec::new();
            for _ in 0..5 {
                std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL + std::time::Duration::from_millis(5));
                let started = Instant::now();
                let samples = sample(&mut sampler);
                times.push(started.elapsed().as_micros());
                assert_eq!(samples.len(), count * (children + 1) + 1);
                assert!(samples.values().all(|sample| sample.cpu_percent.is_some_and(|cpu| cpu.is_finite() && cpu >= 0.)));
            }
            times.sort_unstable();
            println!("{{\"trees\":{trees},\"shells\":{count},\"children_per_shell\":{children},\"application_processes\":1,\"cold_us\":{cold_us},\"warm_samples\":5,\"warm_p50_us\":{},\"warm_max_us\":{}}}", times[2], times[4]);
        }
    }

    #[cfg(unix)]
    #[test]
    fn exited_process_is_not_reported_from_cache() {
        let mut child = std::process::Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let pid = child.id();
        let own_pid = std::process::id();
        let mut sampler = Sampler::default();
        let live = sampler.sample([pid, own_pid]);
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(live.contains_key(&pid));
        let remaining = sampler.sample([pid, own_pid]);
        assert_eq!(remaining.len(), 1);
        assert!(remaining[&own_pid].resident_bytes > 0);
        assert!(remaining[&own_pid].cpu_percent.is_none());
        assert!(sampler.baseline.is_none());
        assert!(!sampler.identities.contains_key(&Pid::from_u32(pid)));
    }
    #[test]
    fn own_process_has_memory_without_fabricating_first_cpu_sample() {
        if !sysinfo::IS_SUPPORTED_SYSTEM {
            return;
        }
        let pid = std::process::id();
        let mut sampler = Sampler::default();
        let first = sampler.sample([pid]);
        assert!(first[&pid].resident_bytes > 0);
        assert!(first[&pid].cpu_percent.is_none());
        assert!(sampler.sample([]).is_empty());
        assert!(sampler.pids.is_empty() && sampler.identities.is_empty());
    }
}
