//! Small, bounded Git HEAD discovery, called only on a background executor.
use std::{fs::File, io::Read, path::Path};
fn small_text(path: &Path) -> Option<String> {
    let mut text = String::new();
    File::open(path).ok()?.take(4097).read_to_string(&mut text).ok()?;
    (text.len() <= 4096).then_some(text)
}
pub fn git_directory(directory: &Path) -> Option<std::path::PathBuf> {
    for ancestor in directory.ancestors().take(64) {
        let marker = ancestor.join(".git");
        let git_dir = if marker.is_dir() { marker } else if marker.is_file() {
            let text = small_text(&marker)?;
            let target = text.trim().strip_prefix("gitdir: ")?;
            ancestor.join(target)
        } else { continue; };
        return git_dir.canonicalize().ok();
    }
    None
}
pub fn branch_at(git_dir: &Path) -> Option<String> {
    let head = small_text(&git_dir.join("HEAD"))?;
    let head = head.trim();
    if let Some(name) = head.strip_prefix("ref: refs/heads/") {
        return (!name.is_empty() && name.len() <= 1024 && !name.chars().any(char::is_control)).then(|| name.to_owned());
    }
    if matches!(head.len(), 40 | 64) && head.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Some(format!("detached {}", &head[..8]));
    }
    None
}
#[cfg(test)]
fn branch(directory: &Path) -> Option<String> { branch_at(&git_directory(directory)?) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn branch_nested_worktree_detached_and_bounds() {
        let root = tempfile::tempdir().unwrap();
        let repo = root.path().join("repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir_all(repo.join("src/nested")).unwrap();
        std::fs::write(repo.join(".git/HEAD"), "ref: refs/heads/feature/世界\n").unwrap();
        assert_eq!(branch(&repo.join("src/nested")).as_deref(), Some("feature/世界"));
        let tree = root.path().join("worktree");
        std::fs::create_dir(&tree).unwrap();
        std::fs::write(tree.join(".git"), "gitdir: ../repo/.git\n").unwrap();
        assert_eq!(branch(&tree).as_deref(), Some("feature/世界"));
        std::fs::write(repo.join(".git/HEAD"), "a".repeat(40)).unwrap();
        assert_eq!(branch(&tree).as_deref(), Some("detached aaaaaaaa"));
        std::fs::write(repo.join(".git/HEAD"), "a".repeat(4097)).unwrap();
        assert_eq!(branch(&tree), None);
        assert_eq!(branch(root.path()), None);
    }
}
