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
/// An explicit HTTP launch target, not protocol detection. Scoped IPv6
/// addresses are copy-only because browser support for zone IDs varies.
pub fn http_url(mut address: SocketAddr) -> Option<String> {
    if address.port() == 0 { return None; }
    if let SocketAddr::V6(ipv6) = address { if ipv6.scope_id() != 0 { return None; } }
    if address.ip().is_unspecified() {
        address.set_ip(if address.is_ipv4() { std::net::Ipv4Addr::LOCALHOST.into() } else { std::net::Ipv6Addr::LOCALHOST.into() });
    }
    Some(format!("http://{address}/"))
}

pub struct Report { pub listeners: Vec<Listener>, pub truncated: bool, pub ownership_changed: bool }
#[derive(Clone, Copy, PartialEq, Eq)]
struct Identity { parent: Option<u32>, started: u64 }
fn process_snapshot() -> HashMap<u32, Identity> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
    // A fresh sampler excludes dead entries retained by filtered refreshes.
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, ProcessRefreshKind::new());
    system.processes().iter().map(|(pid, process)| (pid.as_u32(), Identity {
        parent: process.parent().map(|parent| parent.as_u32()), started: process.start_time(),
    })).collect()
}
fn stable_owned(root: u32, before: &HashMap<u32, Identity>, after: &HashMap<u32, Identity>) -> Result<HashSet<u32>> {
    let original = before.get(&root).ok_or_else(|| anyhow::anyhow!("Terminal process is no longer running"))?;
    ensure!(original.started != 0 && after.get(&root) == Some(original), "Terminal process changed during port scan");
    // A stable child is insufficient if one of its ancestors exited, was
    // reparented or reused. Only traverse the unchanged portion of the tree.
    descendants(root, before.iter().filter(|(pid, identity)| identity.started != 0 && after.get(pid) == Some(identity))
        .map(|(pid, identity)| (*pid, identity.parent)))
}

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

pub fn scan(root: u32, canceled: &AtomicBool) -> Result<Option<Report>> {
    use netstat2::{AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState};
    if canceled.load(Ordering::Acquire) { return Ok(None); }
    let before = process_snapshot();
    if canceled.load(Ordering::Acquire) { return Ok(None); }
    ensure!(before.contains_key(&root), "Terminal process is no longer running");
    let owned = descendants(root, before.iter().map(|(pid, identity)| (*pid, identity.parent)))?;
    let mut listeners = BTreeSet::new();
    let mut truncated = false;
    if canceled.load(Ordering::Acquire) { return Ok(None); }
    let mut sockets = netstat2::iterate_sockets_info(AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6, ProtocolFlags::TCP)?;
    let mut index = 0;
    loop {
        if canceled.load(Ordering::Acquire) { return Ok(None); }
        if index == 100_000 { truncated = true; break; }
        let Some(socket) = sockets.next() else { break; };
        let socket = socket?;
        index += 1;
        let ProtocolSocketInfo::Tcp(tcp) = socket.protocol_socket_info else { continue; };
        if tcp.state != TcpState::Listen { continue; }
        for pid in socket.associated_pids.into_iter().filter(|pid| owned.contains(pid)) {
            listeners.insert(Listener { address: SocketAddr::new(tcp.local_addr, tcp.local_port), pid });
            if listeners.len() > 128 { listeners.pop_last(); truncated = true; break; }
        }
        if truncated { break; }
    }
    if canceled.load(Ordering::Acquire) { return Ok(None); }
    let after = process_snapshot();
    if canceled.load(Ordering::Acquire) { return Ok(None); }
    let stable = stable_owned(root, &before, &after)?;
    let count = listeners.len();
    listeners.retain(|listener| stable.contains(&listener.pid));
    let ownership_changed = listeners.len() != count;
    Ok(Some(Report { listeners: listeners.into_iter().collect(), truncated, ownership_changed }))
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
    fn ownership_recheck_rejects_reused_reparented_and_vanished_ancestors() -> Result<()> {
        let before = HashMap::from([
            (10, Identity { parent: Some(1), started: 100 }),
            (20, Identity { parent: Some(10), started: 200 }),
            (30, Identity { parent: Some(20), started: 300 }),
            (40, Identity { parent: Some(10), started: 400 }),
        ]);
        assert_eq!(stable_owned(10, &before, &before)?, HashSet::from([10,20,30,40]));
        for changed in [Some(Identity { parent: Some(10), started: 201 }), Some(Identity { parent: Some(1), started: 200 }), None] {
            let mut after = before.clone();
            if let Some(identity) = changed { after.insert(20, identity); } else { after.remove(&20); }
            assert_eq!(stable_owned(10, &before, &after)?, HashSet::from([10,40]));
        }
        let mut after = before.clone();
        after.get_mut(&10).unwrap().started += 1;
        assert!(stable_owned(10, &before, &after).is_err());
        Ok(())
    }
    #[test]
    fn canceled_scan_does_not_require_a_live_process() -> Result<()> {
        assert!(scan(0, &AtomicBool::new(true))?.is_none());
        Ok(())
    }
    #[test]
    fn snapshot_finds_current_process_loopback_listener() -> Result<()> {
        let socket = std::net::TcpListener::bind("127.0.0.1:0")?;
        let address = socket.local_addr()?;
        let report = scan(std::process::id(), &AtomicBool::new(false))?.expect("active scan");
        assert!(report.listeners.iter().any(|row| row.address == address && row.pid == std::process::id()));
        Ok(())
    }
}

#[cfg(test)]
mod http_tests {
    use super::*;
    #[test]
    fn browser_targets_map_wildcards_and_preserve_explicit_addresses() {
        for (address, expected) in [
            ("0.0.0.0:3000", "http://127.0.0.1:3000/"),
            ("[::]:8080", "http://[::1]:8080/"),
            ("127.0.0.1:443", "http://127.0.0.1:443/"),
            ("192.168.1.2:9000", "http://192.168.1.2:9000/"),
            ("[::1]:4000", "http://[::1]:4000/"),
        ] { assert_eq!(http_url(address.parse().unwrap()).as_deref(), Some(expected)); }
        assert!(http_url("127.0.0.1:0".parse().unwrap()).is_none());
        assert!(http_url("[fe80::1%2]:8000".parse().unwrap()).is_none());
    }
}
