use super::super::{Host, Message, Task, Usage, Worker, probe::Detector, probe::HostPath};
use super::*;
use crate::registry::all as providers;
use herdr_client::ConnectTarget;
use std::{collections::HashSet, sync::mpsc};

impl Usage {
    /// Replaces the worker with one whose next installation check the
    /// returned closure answers with the given provider ids.
    pub(crate) fn detection_peer_for_test(&mut self) -> impl FnMut(&[&str]) + use<> {
        let (requests, incoming) = mpsc::sync_channel(1);
        let (outgoing, results) = mpsc::sync_channel(1);
        self.worker = Some(Worker { requests, results });
        self.detecting = false;
        move |ids| {
            let Ok(Task::Detect(host)) = incoming.try_recv() else {
                panic!("expected an installation check");
            };
            outgoing
                .send(Message::Installed(host, Ok(ids_of(ids))))
                .unwrap();
        }
    }
}

fn ids_of(ids: &[&str]) -> HashSet<Provider> {
    providers()
        .filter(|provider| ids.contains(&provider.id()))
        .collect()
}

fn installed(usage: &Usage, target: &ConnectTarget) -> HashSet<Provider> {
    usage
        .for_target(target)
        .map(|entry| entry.installed.clone())
        .unwrap_or_default()
}

fn poll_with_usage_off(usage: &mut Usage) -> bool {
    usage.poll(
        None,
        &UsageConfig::default(),
        &Default::default(),
        0,
        false,
        Instant::now(),
    )
}

#[test]
fn one_check_runs_at_a_time_and_each_answer_stays_with_its_host_while_usage_is_off() {
    let (requests, incoming) = mpsc::sync_channel(1);
    let (outgoing, results) = mpsc::sync_channel(1);
    let mut usage = Usage {
        worker: Some(Worker { requests, results }),
        ..Default::default()
    };
    let local = ConnectTarget::Local;
    let remote = ConnectTarget::Ssh {
        target: "test-host".into(),
        session: "test".into(),
    };
    usage.request_detection(&local);
    let Ok(Task::Detect(first)) = incoming.try_recv() else {
        panic!("Settings must check at once");
    };
    usage.request_detection(&remote);
    assert!(usage.detection_target_changed(&local));
    assert!(
        incoming.try_recv().is_err(),
        "only one worker job may run at once"
    );
    outgoing
        .send(Message::Installed(first, Ok(ids_of(&["claude"]))))
        .unwrap();
    assert!(poll_with_usage_off(&mut usage));
    assert_eq!(installed(&usage, &local), ids_of(&["claude"]));
    assert_eq!(installed(&usage, &remote), ids_of(&[]));
    let Ok(Task::Detect(current)) = incoming.try_recv() else {
        panic!("the newest target is checked next");
    };
    assert!(matches!(&current, Host::Ssh(host) if host == "test-host"));
    outgoing
        .send(Message::Installed(current, Ok(ids_of(&["codex"]))))
        .unwrap();
    assert!(poll_with_usage_off(&mut usage));
    assert_eq!(installed(&usage, &remote), ids_of(&["codex"]));
    assert_eq!(installed(&usage, &local), ids_of(&["claude"]));
    assert!(!poll_with_usage_off(&mut usage));
}

#[test]
fn reads_wait_for_a_running_check() {
    let mut usage = Usage::default();
    let mut reply = usage.detection_peer_for_test();
    usage.request_detection(&ConnectTarget::Local);
    let now = Instant::now();
    let config = UsageConfig::default();
    usage.poll(
        Some(Host::Local),
        &config,
        &Default::default(),
        0,
        true,
        now,
    );
    assert!(!usage.busy(), "a read cannot queue behind the check");
    reply(&["codex"]);
    usage.poll(
        Some(Host::Local),
        &config,
        &Default::default(),
        0,
        true,
        now,
    );
    assert!(usage.busy());
    assert_eq!(
        usage.current().unwrap().installed,
        ids_of(&["codex"]),
        "a read keeps what the check found"
    );
}

#[test]
fn a_failed_check_keeps_earlier_evidence_and_a_dead_worker_is_not_respawned() {
    let (requests, incoming) = mpsc::sync_channel(1);
    let (outgoing, results) = mpsc::sync_channel(1);
    let mut usage = Usage {
        worker: Some(Worker { requests, results }),
        ..Default::default()
    };
    let target = ConnectTarget::Local;
    usage.request_detection(&target);
    let Ok(Task::Detect(host)) = incoming.try_recv() else {
        panic!("expected an installation check");
    };
    outgoing
        .send(Message::Installed(host.clone(), Ok(ids_of(&["claude"]))))
        .unwrap();
    poll_with_usage_off(&mut usage);
    usage.request_detection(&target);
    incoming.try_recv().unwrap();
    outgoing
        .send(Message::Installed(host, Err(Error::Unreachable)))
        .unwrap();
    poll_with_usage_off(&mut usage);
    assert_eq!(installed(&usage, &target), ids_of(&["claude"]));
    drop(incoming);
    usage.request_detection(&target);
    assert!(usage.worker.is_none());
    poll_with_usage_off(&mut usage);
    assert!(
        usage.worker.is_none(),
        "a failed request cannot respawn on every poll"
    );
}

#[test]
fn local_detector_checks_file_metadata_not_contents() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("invalid credentials.json");
    std::fs::write(&file, "not JSON, not a usable sign-in").unwrap();
    let mut exec = Exec::Local;
    let mut detector = Detector::new(&mut exec);
    assert!(detector.file_exists(&HostPath::absolute(file.to_string_lossy())));
    assert!(!detector.file_exists(&HostPath::absolute(directory.path().to_string_lossy())));
    std::fs::remove_file(&file).unwrap();
    assert!(!detector.file_exists(&HostPath::absolute(file.to_string_lossy())));
}

#[cfg(unix)]
#[test]
fn disabled_services_are_found_from_unreadable_files_using_host_locator_overrides() {
    use super::super::{Shell, detect_with};
    use std::{
        os::unix::fs::PermissionsExt,
        process::{Command, Stdio},
    };
    let directory = tempfile::tempdir().unwrap();
    let codex = directory.path().join("custom codex home");
    let claude = directory.path().join("custom claude home");
    let home = directory.path().join("home");
    std::fs::create_dir_all(&codex).unwrap();
    std::fs::create_dir_all(&claude).unwrap();
    std::fs::create_dir_all(home.join(".gemini")).unwrap();
    for path in [
        codex.join("auth.json"),
        claude.join(".credentials.json"),
        home.join(".gemini/oauth_creds.json"),
    ] {
        std::fs::write(
            &path,
            "invalid and deliberately unreadable credential contents",
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    }
    let mut command = Command::new("/bin/sh");
    command
        .arg("-s")
        .env_clear()
        .env("HOME", &home)
        .env("CODEX_HOME", &codex)
        .env("CLAUDE_CONFIG_DIR", &claude)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut exec = Exec::Remote(Shell::start(command).unwrap());
    assert_eq!(
        detect_with(&mut exec),
        ids_of(&["codex", "claude", "gemini"])
    );

    let config = UsageConfig {
        hide_providers: providers()
            .map(|provider| provider.id().to_owned())
            .collect(),
        ..Default::default()
    };
    super::super::read(
        &Host::Local,
        &config,
        &Default::default(),
        &Default::default(),
        &mut Default::default(),
        |_, _| panic!("disabled providers must not be fetched"),
    )
    .unwrap();
    std::fs::remove_file(codex.join("auth.json")).unwrap();
    assert_eq!(detect_with(&mut exec), ids_of(&["claude", "gemini"]));
}
