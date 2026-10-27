use super::*;
use crate::{Host, Message, Task, Usage, Worker};
use std::sync::mpsc;

#[test]
fn a_late_rejection_from_old_settings_does_not_delay_the_updated_sign_in() {
    let now = Instant::now();
    let claude = provider("claude");
    let config = UsageConfig::default();
    let mut usage = Usage::default();
    usage.poll(
        Some(Host::Local),
        &config,
        &Default::default(),
        0,
        false,
        now,
    );
    begin(&mut usage, &Host::Local, now);

    // The user replaced their credentials while the old request was running.
    usage.poll(
        Some(Host::Local),
        &config,
        &Default::default(),
        1,
        false,
        now,
    );
    usage.apply(
        Message::Reading(Host::Local, claude, Err(Error::Rejected)),
        &config,
        now,
    );
    usage.apply(Message::Done(Host::Local, Ok(())), &config, now);

    let (requests, incoming) = mpsc::sync_channel(1);
    let (_outgoing, results) = mpsc::sync_channel(1);
    usage.worker = Some(Worker { requests, results });
    usage.poll(
        Some(Host::Local),
        &config,
        &Default::default(),
        1,
        true,
        now,
    );
    let Task::Read(job) = incoming.try_recv().unwrap() else {
        panic!("updated settings must start a quota read");
    };
    assert!(
        !job.held.contains(&claude),
        "the old token cannot back off the new one"
    );
    assert!(usage.busy());
}
