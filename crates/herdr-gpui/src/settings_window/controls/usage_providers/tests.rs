#![allow(clippy::unwrap_used)]

use super::*;
use crate::settings_window::controls::{
    preferences::PreferenceIo,
    tests::{skill_fixture, skill_load},
};
use core::prelude::v1::test;
use gpui::VisualTestContext;
use std::sync::{Arc, Mutex};

fn recording(
    cx: &mut VisualTestContext,
    view: &Entity<SettingsWindow>,
) -> Arc<Mutex<Vec<Preference>>> {
    let edits = Arc::new(Mutex::new(Vec::new()));
    let captured = edits.clone();
    view.update(cx, |view, _| {
        view.section = Section::StatusBar;
        view.controls.preference_io = Some(PreferenceIo {
            write: Arc::new(move |edit| {
                captured.lock().unwrap().push(edit);
                Ok(())
            }),
            load: skill_load,
        });
    });
    cx.simulate_resize(size(px(960.), px(4000.)));
    edits
}

fn click(
    cx: &mut VisualTestContext,
    edits: &Arc<Mutex<Vec<Preference>>>,
    selector: &'static str,
) -> Option<Preference> {
    cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
    let point = cx.debug_bounds(selector).unwrap().center();
    cx.simulate_click(point, Default::default());
    cx.run_until_parked();
    edits.lock().unwrap().pop()
}

#[gpui::test]
fn each_service_toggles_on_its_own(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(skill_fixture);
    let edits = recording(cx, &view);
    assert_eq!(
        click(cx, &edits, "settings-usage-provider-claude"),
        Some(Preference::UsageProviders(ProviderEdit::Disable("claude")))
    );
    view.update(cx, |view, cx| {
        view.config.usage.hide_providers = vec!["claude".into()];
        cx.notify();
    });
    assert_eq!(
        click(cx, &edits, "settings-usage-provider-claude"),
        Some(Preference::UsageProviders(ProviderEdit::Enable("claude")))
    );
    // Another service keeps its own state.
    assert_eq!(
        click(cx, &edits, "settings-usage-provider-codex"),
        Some(Preference::UsageProviders(ProviderEdit::Disable("codex")))
    );
}

#[gpui::test]
fn bulk_buttons_act_only_when_they_would_change_something(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(skill_fixture);
    let edits = recording(cx, &view);
    // Everything starts on, so "Turn all on" has nothing to do.
    assert_eq!(click(cx, &edits, "settings-usage-providers-all"), None);
    assert_eq!(
        click(cx, &edits, "settings-usage-providers-none"),
        Some(Preference::UsageProviders(ProviderEdit::DisableAll))
    );
    view.update(cx, |view, cx| {
        view.config.usage.hide_providers = usage::providers()
            .map(|provider| provider.id().to_owned())
            .collect();
        cx.notify();
    });
    assert_eq!(click(cx, &edits, "settings-usage-providers-none"), None);
    assert_eq!(
        click(cx, &edits, "settings-usage-providers-all"),
        Some(Preference::UsageProviders(ProviderEdit::EnableAll))
    );
}

#[gpui::test]
fn services_wait_while_usage_is_off(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(skill_fixture);
    let edits = recording(cx, &view);
    view.update(cx, |view, cx| {
        view.config.usage.show = false;
        cx.notify();
    });
    for selector in [
        "settings-usage-provider-claude",
        "settings-usage-providers-none",
    ] {
        assert_eq!(click(cx, &edits, selector), None, "{selector}");
    }
}
