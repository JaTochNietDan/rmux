//! Explicit local socket snapshots. No subprocess, background polling or shell input.
use anyhow::{Result, ensure};
use std::{collections::{BTreeSet, HashMap, HashSet}, net::SocketAddr, sync::atomic::{AtomicBool, Ordering}};
static SCANNING: AtomicBool = AtomicBool::new(false);
pub struct Permit;
impl Permit {
    pub fn acquire() -> Result<Self> {
        ensure!(SCANNING.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_ok(), "Another port scan is running; retry shortly");
        Ok(Self)
    }
}
impl Drop for Permit { fn drop(&mut self) { SCANNING.store(false, Ordering::Release); } }
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Listener { pub address: SocketAddr, pub pid: u32 }
pub struct Report { pub listeners: Vec<Listener>, pub truncated: bool }

fn descendants(root: u32, processes: impl IntoIterator<Item = (u32, Option<u32>)>) -> Result<HashSet<u32>> {
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for (pid, parent) in processes { if let Some(parent) = parent { children.entry(parent).or_default().push(pid); } }
    let mut owned = HashSet::new();
    let mut pending = vec![root];
    while let Some(pid) = pending.pop() {
        if owned.insert(pid) {
            ensure!(owned.len() <= 4096, "Terminal process tree exceeds 4096 processes");
            if let Some(children) = children.get(&pid) { pending.extend(children); }
        }
    }
    Ok(owned)
}

pub fn scan(root: u32) -> Result<Report> {
    use netstat2::{AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState};
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, ProcessRefreshKind::new());
    let root_pid = Pid::from_u32(root);
    let Some(process) = system.process(root_pid) else { anyhow::bail!("Terminal process is no longer running"); };
    let started = process.start_time();
    let owned = descendants(root, system.processes().iter().map(|(pid, process)| (pid.as_u32(), process.parent().map(|parent| parent.as_u32()))))?;
    let mut listeners = BTreeSet::new();
    let mut truncated = false;
    let sockets = netstat2::iterate_sockets_info(AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6, ProtocolFlags::TCP)?;
    for (index, socket) in sockets.enumerate() {
        if index == 100_000 { truncated = true; break; }
        let socket = socket?;
        let ProtocolSocketInfo::Tcp(tcp) = socket.protocol_socket_info else { continue; };
        if tcp.state != TcpState::Listen { continue; }
        for pid in socket.associated_pids.into_iter().filter(|pid| owned.contains(pid)) {
            listeners.insert(Listener { address: SocketAddr::new(tcp.local_addr, tcp.local_port), pid });
            if listeners.len() > 128 { listeners.pop_last(); truncated = true; break; }
        }
        if truncated { break; }
    }
    // Exclude a vanished/reused root. Process/socket observations are still not
    // atomic; child ownership can change during the scan.
    ensure!(system.refresh_processes_specifics(ProcessesToUpdate::Some(&[root_pid]), ProcessRefreshKind::new()) == 1
        && system.process(root_pid).is_some_and(|process| process.start_time() == started), "Terminal process changed during port scan");
    Ok(Report { listeners: listeners.into_iter().collect(), truncated })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn process_tree_includes_descendants_and_handles_cycles() -> Result<()> {
        let owned = descendants(10, [(10, Some(30)), (20, Some(10)), (30, Some(20)), (99, Some(1))])?;
        assert_eq!(owned, HashSet::from([10,20,30]));
        Ok(())
    }
    #[test]
    fn snapshot_finds_current_process_loopback_listener() -> Result<()> {
        let socket = std::net::TcpListener::bind("127.0.0.1:0")?;
        let address = socket.local_addr()?;
        let report = scan(std::process::id())?;
        assert!(report.listeners.iter().any(|row| row.address == address && row.pid == std::process::id()));
        Ok(())
    }
}
