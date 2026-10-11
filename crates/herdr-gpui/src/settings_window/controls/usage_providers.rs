//! Settings > Status bar > Usage services: which AI services Herdr reads plan
//! usage for. A turned-off service is never probed, so its sign-in is not read
//! and nothing is sent to it.
use super::*;
use crate::{
    config::{preferences::Preference, usage_providers::ProviderEdit},
    usage,
};

impl SettingsWindow {
    pub(super) fn render_usage_provider_controls(&self, cx: &mut Context<Self>) -> Div {
        let config = &self.config.usage;
        let on = config.show;
        let mut chips = div().flex().flex_wrap().gap(px(8.));
        let mut enabled = 0;
        for provider in usage::providers() {
            let id = provider.id();
            let active = !config.hidden(provider);
            enabled += usize::from(active);
            let edit = if active {
                ProviderEdit::Disable(id)
            } else {
                ProviderEdit::Enable(id)
            };
            let selector = format!("settings-usage-provider-{id}");
            chips = chips.child(
                self.control_choice(selector.clone(), provider.name(), active, on)
                    .debug_selector(move || selector.clone())
                    .when(on, |chip| {
                        chip.on_click(cx.listener(move |this, _, _, cx| {
                            this.save_preference(Preference::UsageProviders(edit), cx);
                        }))
                    }),
            );
        }
        let total = usage::providers().count();
        self.control_card("Usage services")
            .child(self.control_note(
                "Choose which services to read plan usage for, such as only Claude. A service that is off is never contacted and its sign-in is not read; one that is on still appears only when it is signed in.",
            ))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(8.))
                    .child(self.bulk_choice(
                        "settings-usage-providers-all",
                        "Turn all on",
                        ProviderEdit::EnableAll,
                        on && enabled < total,
                        cx,
                    ))
                    .child(self.bulk_choice(
                        "settings-usage-providers-none",
                        "Turn all off",
                        ProviderEdit::DisableAll,
                        on && enabled > 0,
                        cx,
                    ))
                    .child(
                        self.control_note(format!("{enabled} of {total} on"))
                            .debug_selector(|| "settings-usage-providers-count".into()),
                    ),
            )
            .child(chips)
            .when(!on, |card| {
                card.child(self.control_note("Turn Usage on above to choose services."))
            })
    }

    /// A button that changes every service at once; idle when it would change
    /// nothing.
    fn bulk_choice(
        &self,
        id: &'static str,
        label: &'static str,
        edit: ProviderEdit,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        self.control_choice(id, label, false, enabled)
            .debug_selector(move || id.into())
            .when(enabled, |button| {
                button.on_click(cx.listener(move |this, _, _, cx| {
                    this.save_preference(Preference::UsageProviders(edit), cx);
                }))
            })
    }
}

#[cfg(test)]
mod tests;
