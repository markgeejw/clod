use std::fs;

use assert_cmd::Command;
use tempfile::TempDir;

fn clod(home: &TempDir, claude_home: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("clod").unwrap();
    cmd.arg("--clod-home").arg(home.path());
    cmd.arg("--claude-home").arg(claude_home.path());
    // Ensure tests don't accidentally pick up a developer's CLOD_PROFILE.
    cmd.env_remove("CLOD_PROFILE");
    cmd.env_remove("CLOD_HOME");
    cmd
}

#[test]
fn end_to_end_new_switch_current_ls() {
    let clod_home = TempDir::new().unwrap();
    let claude_home = TempDir::new().unwrap();

    // seed shared dirs in fake ~/.claude so `new` has something to symlink
    fs::create_dir_all(claude_home.path().join("skills")).unwrap();
    fs::write(claude_home.path().join("CLAUDE.md"), "shared").unwrap();

    clod(&clod_home, &claude_home)
        .args(["new", "personal"])
        .assert()
        .success();
    clod(&clod_home, &claude_home)
        .args(["new", "work"])
        .assert()
        .success();

    clod(&clod_home, &claude_home)
        .args(["switch", "work"])
        .assert()
        .success();

    let current = clod(&clod_home, &claude_home)
        .arg("current")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&current.get_output().stdout).into_owned();
    assert!(
        stdout.starts_with("work"),
        "expected 'work...' got: {stdout}"
    );

    let ls = clod(&clod_home, &claude_home).arg("ls").assert().success();
    let stdout = String::from_utf8_lossy(&ls.get_output().stdout).into_owned();
    // active marker on `work`, not on `personal`
    assert!(stdout.contains("* work"), "ls output: {stdout}");
    assert!(stdout.contains("  personal"), "ls output: {stdout}");

    // shared symlinks exist in the new profile
    let prof = clod_home.path().join("profiles/personal");
    assert!(prof.join("skills").is_symlink());
    assert!(prof.join("CLAUDE.md").is_symlink());
}

#[test]
fn switch_unknown_profile_errors() {
    let clod_home = TempDir::new().unwrap();
    let claude_home = TempDir::new().unwrap();

    clod(&clod_home, &claude_home)
        .args(["switch", "ghost"])
        .assert()
        .failure();
}

#[test]
fn rm_active_profile_refuses() {
    let clod_home = TempDir::new().unwrap();
    let claude_home = TempDir::new().unwrap();

    clod(&clod_home, &claude_home)
        .args(["new", "p"])
        .assert()
        .success();
    clod(&clod_home, &claude_home)
        .args(["switch", "p"])
        .assert()
        .success();

    let out = clod(&clod_home, &claude_home)
        .args(["rm", "p", "-y"])
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).into_owned();
    assert!(stderr.contains("active profile"), "stderr: {stderr}");
}

#[test]
fn unknown_args_forwarded_to_claude() {
    let clod_home = TempDir::new().unwrap();
    let claude_home = TempDir::new().unwrap();

    // a fake `claude` that prints what it received
    let fake = claude_home.path().join("fakeclaude");
    fs::write(
        &fake,
        "#!/bin/sh\necho \"DIR=$CLAUDE_CONFIG_DIR\"\necho \"ARGS=$*\"\n",
    )
    .unwrap();
    let mut perms = fs::metadata(&fake).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    fs::set_permissions(&fake, perms).unwrap();

    clod(&clod_home, &claude_home)
        .args(["new", "personal"])
        .assert()
        .success();
    clod(&clod_home, &claude_home)
        .args(["switch", "personal"])
        .assert()
        .success();

    // No `run --` — args go straight to claude.
    let out = clod(&clod_home, &claude_home)
        .args(["--claude-bin", fake.to_str().unwrap(), "--print", "hi"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(
        stdout.contains(&format!(
            "DIR={}",
            clod_home.path().join("profiles/personal").display()
        )),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("ARGS=--print hi"), "stdout: {stdout}");
}

#[test]
fn profile_flag_overrides_active() {
    let clod_home = TempDir::new().unwrap();
    let claude_home = TempDir::new().unwrap();

    // a fake `claude` that prints what it received
    let fake = claude_home.path().join("fakeclaude");
    fs::write(
        &fake,
        "#!/bin/sh\necho \"DIR=$CLAUDE_CONFIG_DIR\"\necho \"ARGS=$*\"\n",
    )
    .unwrap();
    let mut perms = fs::metadata(&fake).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    fs::set_permissions(&fake, perms).unwrap();

    clod(&clod_home, &claude_home)
        .args(["new", "personal"])
        .assert()
        .success();
    clod(&clod_home, &claude_home)
        .args(["new", "work"])
        .assert()
        .success();
    clod(&clod_home, &claude_home)
        .args(["switch", "personal"])
        .assert()
        .success();

    // --profile work overrides the persisted personal active.
    let out = clod(&clod_home, &claude_home)
        .args([
            "--claude-bin",
            fake.to_str().unwrap(),
            "--profile",
            "work",
            "--print",
            "hi",
        ])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(
        stdout.contains(&format!(
            "DIR={}",
            clod_home.path().join("profiles/work").display()
        )),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("ARGS=--print hi"), "stdout: {stdout}");
}

#[test]
fn profile_flag_for_missing_profile_errors() {
    let clod_home = TempDir::new().unwrap();
    let claude_home = TempDir::new().unwrap();

    let out = clod(&clod_home, &claude_home)
        .args(["--profile", "ghost", "run"])
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).into_owned();
    assert!(stderr.contains("ghost"), "stderr: {stderr}");
}

#[test]
fn current_with_no_profile_errors_helpfully() {
    let clod_home = TempDir::new().unwrap();
    let claude_home = TempDir::new().unwrap();
    let out = clod(&clod_home, &claude_home)
        .arg("current")
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).into_owned();
    assert!(stderr.contains("no active profile"), "stderr: {stderr}");
}
