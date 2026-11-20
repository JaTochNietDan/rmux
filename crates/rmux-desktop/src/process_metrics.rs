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
        use crate::process_tree::{process_snapshot, descendants, stable_owned};
        use std::collections::BTreeSet;
        let stopped = || canceled.load(std::sync::atomic::Ordering::Relaxed);
        if stopped() { return Default::default(); }
        let before = process_snapshot();
        let mut selected: BTreeSet<_> = pids.into_iter().take(513).collect();
        let mut trees = HashMap::new();
        for root in roots.into_iter().take(256) {
            if stopped() { return Default::default(); }
            let Ok(owned) = descendants(root, before.iter().map(|(pid, identity)| (*pid, identity.parent))) else { continue; };
            let ordered: BTreeSet<_> = owned.iter().copied().collect();
            for pid in ordered { if selected.len() == 4096 { break; } selected.insert(pid); }
            trees.insert(root, owned);
        }
        if stopped() { return Default::default(); }
        let samples = self.sample_bounded(selected, 4096);
        if stopped() { return Default::default(); }
        let after = process_snapshot();
        let mut totals = HashMap::new();
        for (root, owned) in trees {
            if stopped() { return Default::default(); }
            let Ok(stable) = stable_owned(root, &before, &after) else { continue; };
            let valid: Vec<_> = owned.iter().filter(|pid| stable.contains(pid)).filter_map(|pid| {
                samples.get(pid).filter(|sample| before.get(pid).is_some_and(|identity| identity.started == sample.start_time_seconds))
            }).collect();
            totals.insert(root, aggregate_tree(owned.len(), valid.into_iter()));
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
    fn sampling_scaling() {
        struct Shells(Vec<std::process::Child>);
        impl Drop for Shells {
            fn drop(&mut self) {
                for child in &mut self.0 {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }
        for count in [1, 8, 32] {
            let mut shells = Shells(Vec::new());
            for _ in 0..count {
                shells.0.push(std::process::Command::new("/bin/sh")
                    .args(["-c", "read line"])
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn().unwrap());
            }
            let pids: Vec<_> = std::iter::once(std::process::id())
                .chain(shells.0.iter().map(|child| child.id())).collect();
            let mut sampler = Sampler::default();
            let started = Instant::now();
            let first = sampler.sample(pids.iter().copied());
            let cold_us = started.elapsed().as_micros();
            assert_eq!(first.len(), count + 1);
            assert!(first.values().all(|sample| sample.cpu_percent.is_none()));
            let mut times = Vec::new();
            for _ in 0..5 {
                std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL + std::time::Duration::from_millis(5));
                let started = Instant::now();
                let samples = sampler.sample(pids.iter().copied());
                times.push(started.elapsed().as_micros());
                assert_eq!(samples.len(), count + 1);
                assert!(samples.values().all(|sample| sample.cpu_percent.is_some_and(|cpu| cpu.is_finite() && cpu >= 0.)));
            }
            times.sort_unstable();
            println!("{{\"shells\":{count},\"application_processes\":1,\"cold_us\":{cold_us},\"warm_samples\":5,\"warm_p50_us\":{},\"warm_max_us\":{}}}", times[2], times[4]);
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
