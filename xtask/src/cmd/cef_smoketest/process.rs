use std::collections::{HashMap, HashSet};
use std::process::{Command, Stdio};

pub trait ProcessEnumerator: Send + Sync {
    fn descendants(&self, root: u32) -> Vec<u32>;
}

#[derive(Clone, Debug, Default)]
pub struct FakeProcessEnumerator {
    children: HashMap<u32, Vec<u32>>,
}

impl FakeProcessEnumerator {
    pub fn new(children: HashMap<u32, Vec<u32>>) -> Self {
        Self { children }
    }
}

impl ProcessEnumerator for FakeProcessEnumerator {
    fn descendants(&self, root: u32) -> Vec<u32> {
        let mut out = Vec::new();
        collect_descendants(root, &self.children, &mut HashSet::new(), &mut out);
        out
    }
}

fn collect_descendants(
    parent: u32,
    children: &HashMap<u32, Vec<u32>>,
    seen: &mut HashSet<u32>,
    out: &mut Vec<u32>,
) {
    if let Some(kids) = children.get(&parent) {
        for &kid in kids {
            if seen.insert(kid) {
                out.push(kid);
                collect_descendants(kid, children, seen, out);
            }
        }
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Default)]
pub struct Pgrep;

#[cfg(target_os = "macos")]
impl ProcessEnumerator for Pgrep {
    fn descendants(&self, root: u32) -> Vec<u32> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        self.collect(root, &mut seen, &mut out);
        out
    }
}

#[cfg(target_os = "macos")]
impl Pgrep {
    fn collect(&self, parent: u32, seen: &mut HashSet<u32>, out: &mut Vec<u32>) {
        for child in direct_children(parent) {
            if seen.insert(child) {
                out.push(child);
                self.collect(child, seen, out);
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn direct_children(parent: u32) -> Vec<u32> {
    let output = Command::new("pgrep")
        .args(["-P", &parent.to_string()])
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };

    if !output.status.success() {
        return Vec::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .filter_map(|pid| pid.parse::<u32>().ok())
        .collect()
}

pub fn filter_live_pids(pids: &[u32]) -> Vec<u32> {
    pids.iter()
        .copied()
        .filter(|pid| pid_is_live(*pid))
        .collect()
}

pub fn pid_is_live(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

pub fn process_tree(root: u32, enumerator: &dyn ProcessEnumerator) -> Vec<u32> {
    let mut pids = enumerator.descendants(root);
    pids.push(root);
    pids.sort_unstable();
    pids.dedup();
    pids
}

pub fn kill_pids(pids: &[u32]) {
    let mut pids = pids.to_vec();
    pids.sort_unstable_by(|a, b| b.cmp(a));

    for pid in pids {
        let _ = Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

pub fn kill_tree(root: u32, enumerator: &dyn ProcessEnumerator) {
    let pids = process_tree(root, enumerator);
    kill_pids(&pids);
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{FakeProcessEnumerator, ProcessEnumerator};

    #[test]
    fn fake_process_enumerator_walks_transitive_tree() {
        let enumerator = FakeProcessEnumerator::new(HashMap::from([
            (10, vec![20]),
            (20, vec![30]),
            (30, vec![40]),
        ]));

        let descendants = enumerator.descendants(10);
        assert_eq!(descendants, vec![20, 30, 40]);
    }
}
