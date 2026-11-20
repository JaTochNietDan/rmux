//! Manual, PID-filtered process sampling. Never reads command lines or environment.
use std::{collections::HashMap, time::Instant};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

#[derive(Clone, Copy, serde::Serialize)]
pub struct Sample {
    pub resident_bytes: u64,
    pub cpu_percent: Option<f32>,
    pub start_time_seconds: u64,
}

#[derive(Default)]
pub struct Sampler {
    system: System,
    pids: Vec<Pid>,
    baseline: Option<Instant>,
    identities: HashMap<Pid, u64>,
}

impl Sampler {
    pub fn sample(&mut self, pids: impl IntoIterator<Item = u32>) -> HashMap<u32, Sample> {
        // One application process plus at most 256 terminal shell processes.
        let mut pids: Vec<_> = pids.into_iter().take(257).map(Pid::from_u32).collect();
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

#[cfg(test)]
mod tests {
    use super::*;
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
