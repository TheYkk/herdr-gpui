//! Which plan-usage services the status bar reads, saved as the
//! `[usage] hide_providers` list so Settings and hand edits stay one thing.
use super::*;

/// One change to the set of usage services that are turned off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProviderEdit {
    Enable(&'static str),
    Disable(&'static str),
    EnableAll,
    DisableAll,
}

/// Lists longer than this are written one id per line.
const INLINE_LIMIT: usize = 4;

/// The `hide_providers` value after `edit`, or `None` when nothing is left
/// hidden and the key should go. Ids this build does not know are kept, so a
/// config shared with a newer build loses nothing.
pub(super) fn hide_providers(
    document: &toml_edit::DocumentMut,
    edit: ProviderEdit,
) -> Result<Option<toml_edit::Value>> {
    let mut hidden = match document.get("usage") {
        None => Vec::new(),
        Some(usage) => {
            let usage = usage.as_table_like().ok_or(Error::InvalidUsageTable)?;
            match usage.get("hide_providers") {
                None => Vec::new(),
                Some(item) => item
                    .as_array()
                    .ok_or(Error::InvalidHideProviders)?
                    .iter()
                    .map(|id| id.as_str().map(str::to_owned))
                    .collect::<Option<Vec<_>>>()
                    .ok_or(Error::InvalidHideProviders)?,
            }
        }
    };
    match edit {
        ProviderEdit::Enable(id) => hidden.retain(|hidden| hidden != id),
        ProviderEdit::Disable(id) => {
            if !hidden.iter().any(|hidden| hidden == id) {
                hidden.push(id.to_owned());
            }
        }
        ProviderEdit::EnableAll => {
            hidden.retain(|id| !crate::usage::providers().any(|known| known.id() == id));
        }
        ProviderEdit::DisableAll => {
            for provider in crate::usage::providers() {
                if !hidden.iter().any(|hidden| hidden == provider.id()) {
                    hidden.push(provider.id().to_owned());
                }
            }
        }
    }
    if hidden.is_empty() {
        return Ok(None);
    }
    let mut ids = toml_edit::Array::new();
    for id in &hidden {
        ids.push(id.as_str());
    }
    if hidden.len() > INLINE_LIMIT {
        for id in ids.iter_mut() {
            id.decor_mut().set_prefix("\n  ");
        }
        ids.set_trailing("\n");
        ids.set_trailing_comma(true);
    }
    Ok(Some(toml_edit::Value::Array(ids)))
}
