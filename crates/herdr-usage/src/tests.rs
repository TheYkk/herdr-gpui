#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{
    Reading, UsageConfig,
    cookies::CookieJar,
    model::{Account, Kind, Provider, Report, WEEK, Window},
    probe::{Consent, Exec, Probe},
    registry,
};
use crate::Error;
use std::time::{Duration, Instant, SystemTime};

#[cfg(unix)]
mod antigravity_consent;
mod browser_cookies;
#[cfg(unix)]
mod claude_base_url;
#[cfg(unix)]
mod claude_keychain;
#[cfg(feature = "azure")]
mod cloud_load;
mod installed;
mod keychain_access;
mod labels;
#[cfg(unix)]
mod linuxbrew_path;
#[cfg(unix)]
mod opencode_auth;
mod pace_exhausted;
mod provider_data;
mod provider_settings;
#[cfg(unix)]
mod quiet_app_data;
mod refresh;
mod rejected_backoff;
#[cfg(unix)]
mod remote_hosts;
mod settings_during_refresh;
mod settings_rejected_read;
mod shown_providers;
mod visibility;
mod wsl_hosts;

fn at(seconds: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
}

fn provider(id: &str) -> Provider {
    registry::find(id).unwrap()
}

fn report(provider: Provider, used: f64) -> Report {
    Report::new(
        provider,
        Account::default(),
        vec![Window::new(Kind::Session, used, None, None)],
    )
}

fn begin(usage: &mut super::Usage, host: &super::Host, now: Instant) {
    usage.host = Some(host.clone());
    usage.begin(host.clone(), now);
}
