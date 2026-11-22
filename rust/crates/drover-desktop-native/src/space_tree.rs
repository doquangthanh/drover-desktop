//! Worktree spaces under their repository's space in the sidebar, as Herdr shows them. A task worktree lives at
//! `<repo>-wt/<task>` next to the repository (the daemon's `git::create_worktree`), and opening it makes a space of
//! its own; here that space is put under the space whose folder is `<repo>`, on the same machine.

use drover_proto::wire::Space;

/// A folder written the same way on every platform: `/` separators, no trailing `/`, lower case on Windows.
fn norm(path: &str) -> String {
    let p = path.replace('\\', "/").trim_end_matches('/').to_string();
    if cfg!(windows) { p.to_lowercase() } else { p }
}

/// The repository folder a task worktree folder belongs to: `<dir>/<repo>-wt/<task>` -> `<dir>/<repo>`.
pub fn repo_of_worktree(cwd: &str) -> Option<String> {
    let cwd = norm(cwd);
    let (wt_dir, task) = cwd.rsplit_once('/')?;
    let repo = wt_dir.strip_suffix("-wt")?;
    (!task.is_empty() && !repo.is_empty() && !repo.ends_with('/')).then(|| repo.to_string())
}

/// The space `s` sits under: the open space of its repository on the same machine, if any.
pub fn parent_of<'a>(s: &Space, spaces: &[&'a Space]) -> Option<&'a Space> {
    let repo = repo_of_worktree(&s.cwd)?;
    spaces.iter().copied().find(|p| p.id != s.id && p.machine == s.machine && norm(&p.cwd) == repo)
}

/// Spaces in sidebar order: each top-level space followed by its worktree spaces (true = nested).
pub fn ordered<'a>(spaces: &[&'a Space]) -> Vec<(&'a Space, bool)> {
    let mut out = vec![];
    for s in spaces.iter().copied().filter(|s| parent_of(s, spaces).is_none()) {
        out.push((s, false));
        out.extend(spaces.iter().copied().filter(|c| parent_of(c, spaces).is_some_and(|p| p.id == s.id)).map(|c| (c, true)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn space(id: &str, cwd: &str) -> Space {
        Space { id: id.into(), name: id.into(), cwd: cwd.into(), ..Default::default() }
    }

    #[test]
    fn a_task_worktree_folder_points_at_its_repository() {
        assert_eq!(repo_of_worktree("/home/u/ecam-wt/ecam-5690").as_deref(), Some("/home/u/ecam"));
        assert_eq!(repo_of_worktree("/home/u/ecam-wt/ecam-5690/").as_deref(), Some("/home/u/ecam"));
        assert_eq!(repo_of_worktree("/home/u/ecam"), None);
        assert_eq!(repo_of_worktree("/-wt/x"), None);
    }

    #[test]
    fn worktree_spaces_follow_their_repository_and_orphans_stay_on_top() {
        let (repo, other) = (space("s1", "/p/shop"), space("s2", "/p/blog"));
        let (wt, orphan) = (space("s3", "/p/shop-wt/fix-cart"), space("s4", "/p/gone-wt/task"));
        let all = [&wt, &repo, &orphan, &other];
        let got: Vec<(&str, bool)> = ordered(&all).iter().map(|(s, nested)| (s.id.as_str(), *nested)).collect();
        assert_eq!(got, [("s1", false), ("s3", true), ("s4", false), ("s2", false)]);
    }

    #[test]
    fn a_worktree_on_another_machine_is_not_nested() {
        let repo = space("s1", "/p/shop");
        let wt = Space { machine: Some("box".into()), ..space("box:s3", "/p/shop-wt/x") };
        assert!(parent_of(&wt, &[&repo, &wt]).is_none());
    }
}
