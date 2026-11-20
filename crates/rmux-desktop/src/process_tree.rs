//! Process ownership snapshots shared by manual diagnostics.
use anyhow::{Result, ensure};
use std::collections::{HashMap, HashSet};
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Identity { pub parent: Option<u32>, pub started: u64 }
pub fn process_snapshot() -> HashMap<u32, Identity> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
    // A fresh sampler excludes dead entries retained by filtered refreshes.
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, ProcessRefreshKind::new());
    system.processes().iter().map(|(pid, process)| (pid.as_u32(), Identity {
        parent: process.parent().map(|parent| parent.as_u32()), started: process.start_time(),
    })).collect()
}
pub fn ensure_stable_root(root: u32, before: &HashMap<u32, Identity>, after: &HashMap<u32, Identity>) -> Result<()> {
    let original = before.get(&root).ok_or_else(|| anyhow::anyhow!("Terminal process is no longer running"))?;
    ensure!(original.started != 0 && after.get(&root) == Some(original), "Terminal process changed during sampling");
    Ok(())
}
pub fn stable_index(before: &HashMap<u32, Identity>, after: &HashMap<u32, Identity>) -> TreeIndex {
    // Only traverse unchanged ancestry, not just unchanged leaf processes.
    TreeIndex::new(before.iter().filter(|(pid, identity)| identity.started != 0 && after.get(pid) == Some(identity))
        .map(|(pid, identity)| (*pid, identity.parent)))
}
pub fn stable_owned(root: u32, before: &HashMap<u32, Identity>, after: &HashMap<u32, Identity>) -> Result<HashSet<u32>> {
    ensure_stable_root(root, before, after)?;
    stable_index(before, after).descendants(root)
}

pub struct TreeIndex { children: HashMap<u32, Vec<u32>> }
impl TreeIndex {
    pub fn new(processes: impl IntoIterator<Item = (u32, Option<u32>)>) -> Self {
        let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
        for (pid, parent) in processes { if let Some(parent) = parent { children.entry(parent).or_default().push(pid); } }
        Self { children }
    }
    pub fn descendants(&self, root: u32) -> Result<HashSet<u32>> {
        let mut owned = HashSet::new();
        let mut pending = vec![root];
        while let Some(pid) = pending.pop() {
            if owned.insert(pid) {
                ensure!(owned.len() <= 4096, "Terminal process tree exceeds 4096 processes");
                if let Some(children) = self.children.get(&pid) { pending.extend(children); }
            }
        }
        Ok(owned)
    }
}
pub fn descendants(root: u32, processes: impl IntoIterator<Item = (u32, Option<u32>)>) -> Result<HashSet<u32>> {
    TreeIndex::new(processes).descendants(root)
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
    fn oversized_process_tree_is_rejected() {
        assert_eq!(descendants(1, (2..=4096).map(|pid| (pid, Some(1)))).unwrap().len(), 4096);
        assert!(descendants(1, (2..=4097).map(|pid| (pid, Some(1)))).is_err());
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
}
