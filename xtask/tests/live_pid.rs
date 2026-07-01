use std::process::Command;

use zbag_xtask::cmd::cef_smoketest::process::filter_live_pids;

#[test]
fn filter_live_pids_all_live() {
    let current = std::process::id();
    assert_eq!(filter_live_pids(&[current]), vec![current]);
}

#[test]
fn filter_live_pids_mixed() {
    let current = std::process::id();
    let dead = spawn_dead_pid();
    assert_eq!(filter_live_pids(&[current, dead]), vec![current]);
}

#[test]
fn filter_live_pids_all_dead() {
    let dead = spawn_dead_pid();
    assert!(filter_live_pids(&[dead]).is_empty());
}

fn spawn_dead_pid() -> u32 {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(":")
        .spawn()
        .expect("spawn short-lived process");
    let pid = child.id();
    let _ = child.wait();
    pid
}
