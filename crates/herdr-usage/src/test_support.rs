//! Synthetic plan readings for the isolated native Settings smoke check.
use super::*;
use std::io;

pub fn native_fixture() -> io::Result<Usage> {
    let providers = [
        registry::find("codex").ok_or_else(|| io::Error::other("native Codex provider missing"))?,
        registry::find("claude")
            .ok_or_else(|| io::Error::other("native Claude provider missing"))?,
    ];
    let readings = providers
        .into_iter()
        .map(|provider| Reading {
            provider,
            report: Some(report(provider)),
            error: None,
            access: None,
        })
        .collect();
    let (requests, incoming) = mpsc::sync_channel::<Task>(1);
    let (outgoing, results) = mpsc::sync_channel(4);
    thread::Builder::new()
        .name("herdr-native-usage-fixture".into())
        .spawn(move || {
            for task in incoming {
                let job = match task {
                    Task::Read(job) => job,
                    Task::Detect(host) => {
                        let installed = Ok(Default::default());
                        if outgoing.send(Message::Installed(host, installed)).is_err() {
                            return;
                        }
                        continue;
                    }
                };
                for provider in providers {
                    if job.config.hidden(provider) {
                        continue;
                    }
                    if outgoing
                        .send(Message::Reading(
                            job.host.clone(),
                            provider,
                            Ok(report(provider)),
                        ))
                        .is_err()
                    {
                        return;
                    }
                }
                if outgoing.send(Message::Done(job.host, Ok(()))).is_err() {
                    return;
                }
            }
        })?;
    Ok(Usage {
        host: Some(Host::Local),
        entries: HashMap::from([(
            Host::Local,
            Entry {
                readings,
                due: Some(Instant::now() + REFRESH),
                ..Default::default()
            },
        )]),
        worker: Some(Worker { requests, results }),
        ..Default::default()
    })
}

fn report(provider: Provider) -> Report {
    Report::new(
        provider,
        model::Account::default(),
        vec![model::Window::new(model::Kind::Session, 35., None, None)],
    )
}

impl Usage {
    pub fn native_forget_claude_report(&mut self) -> io::Result<()> {
        let reading = self
            .entries
            .get_mut(&Host::Local)
            .and_then(|entry| {
                entry
                    .readings
                    .iter_mut()
                    .find(|reading| reading.provider.id() == "claude")
            })
            .ok_or_else(|| io::Error::other("missing native Claude reading"))?;
        reading.report = None;
        reading.access = Some(Access::Needed);
        Ok(())
    }
}

impl Usage {
    /// An isolated worker whose next metadata request is answered by the caller.
    pub fn detection_peer(&mut self) -> impl FnMut(&[&str]) -> io::Result<()> + use<> {
        let (requests, incoming) = mpsc::sync_channel(1);
        let (outgoing, results) = mpsc::sync_channel(1);
        self.worker = Some(Worker { requests, results });
        self.detecting = false;
        move |ids| {
            let Ok(Task::Detect(host)) = incoming.try_recv() else {
                return Err(io::Error::other("expected an installation check"));
            };
            let installed = registry::all()
                .filter(|provider| ids.contains(&provider.id()))
                .collect();
            outgoing
                .send(Message::Installed(host, Ok(installed)))
                .map_err(|_| io::Error::other("installation receiver closed"))
        }
    }
}
