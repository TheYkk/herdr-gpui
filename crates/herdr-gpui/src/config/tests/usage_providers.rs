use super::*;
use crate::config::{preferences::Preference, usage_providers::ProviderEdit};

fn save(edit: ProviderEdit, path: &Path) -> anyhow::Result<Config> {
    Config::save_preference_path(Preference::UsageProviders(edit), path)?;
    Ok(Config::parse(&fs::read_to_string(path)?)?)
}

/// Settings > Status bar turns a service off and on in place, so the user's
/// comments, other `[usage]` keys, and providers survive.
#[test]
fn a_service_turns_off_and_on_without_disturbing_the_usage_table() -> anyhow::Result<()> {
    let temp = TempDirectory::new()?;
    let path = temp.0.join("config.toml");
    let original = "theme = 'Nord' # keep\n[usage]\nshow = true # bar\n";
    fs::write(&path, original)?;

    let config = save(ProviderEdit::Disable("cursor"), &path)?;
    assert_eq!(config.usage.hide_providers, ["cursor"]);
    let config = save(ProviderEdit::Disable("openrouter"), &path)?;
    assert_eq!(config.usage.hide_providers, ["cursor", "openrouter"]);
    // Turning off what is already off changes nothing.
    let config = save(ProviderEdit::Disable("cursor"), &path)?;
    assert_eq!(config.usage.hide_providers, ["cursor", "openrouter"]);

    let config = save(ProviderEdit::Enable("cursor"), &path)?;
    assert_eq!(config.usage.hide_providers, ["openrouter"]);
    let config = save(ProviderEdit::Enable("openrouter"), &path)?;
    assert!(config.usage.hide_providers.is_empty());
    // Nothing left hidden removes the key rather than leaving `[]` behind.
    let text = fs::read_to_string(&path)?;
    assert!(!text.contains("hide_providers"), "{text}");
    assert!(text.contains("theme = 'Nord' # keep") && text.contains("show = true # bar"));
    assert!(config.usage.show);
    Ok(())
}

#[test]
fn turning_every_service_off_leaves_only_the_ones_picked() -> anyhow::Result<()> {
    let temp = TempDirectory::new()?;
    let path = temp.0.join("config.toml");
    fs::write(&path, "[usage]\nshow_providers = ['cursor']\n")?;

    let config = save(ProviderEdit::DisableAll, &path)?;
    let total = crate::usage::providers().count();
    assert_eq!(config.usage.hide_providers.len(), total);
    assert!(total > 1);

    let config = save(ProviderEdit::Enable("claude"), &path)?;
    assert_eq!(config.usage.hide_providers.len(), total - 1);
    assert!(!config.usage.hide_providers.iter().any(|id| id == "claude"));
    assert!(config.usage.hide_providers.iter().any(|id| id == "codex"));
    // Only the list changes: forced services and other keys stay put.
    assert_eq!(config.usage.show_providers, ["cursor"]);

    let config = save(ProviderEdit::EnableAll, &path)?;
    assert!(config.usage.hide_providers.is_empty());
    assert_eq!(config.usage.show_providers, ["cursor"]);
    Ok(())
}

/// A config written by a newer build may name services this one lacks. Those
/// ids stay in the file, even through "turn all on".
#[test]
fn unknown_services_are_kept_when_the_known_ones_change() -> anyhow::Result<()> {
    let temp = TempDirectory::new()?;
    let path = temp.0.join("config.toml");
    fs::write(&path, "[usage]\nhide_providers = ['from-the-future']\n")?;
    save(ProviderEdit::Disable("claude"), &path)?;
    let table: toml::Table = toml::from_str(&fs::read_to_string(&path)?)?;
    let hidden = table["usage"]["hide_providers"]
        .as_array()
        .context("hide_providers must be a list")?;
    assert_eq!(
        hidden
            .iter()
            .filter_map(toml::Value::as_str)
            .collect::<Vec<_>>(),
        ["from-the-future", "claude"]
    );
    save(ProviderEdit::EnableAll, &path)?;
    let table: toml::Table = toml::from_str(&fs::read_to_string(&path)?)?;
    assert_eq!(
        table["usage"]["hide_providers"][0].as_str(),
        Some("from-the-future")
    );
    assert_eq!(
        table["usage"]["hide_providers"].as_array().map(Vec::len),
        Some(1)
    );
    Ok(())
}

#[test]
fn a_long_list_is_written_one_service_per_line() -> anyhow::Result<()> {
    let temp = TempDirectory::new()?;
    let path = temp.0.join("config.toml");
    fs::write(&path, "")?;
    save(ProviderEdit::DisableAll, &path)?;
    let text = fs::read_to_string(&path)?;
    assert!(
        text.contains("hide_providers = [\n  \"codex\",\n"),
        "{text}"
    );
    // A short list stays on one line.
    fs::write(&path, "")?;
    save(ProviderEdit::Disable("claude"), &path)?;
    assert!(fs::read_to_string(&path)?.contains("hide_providers = [\"claude\"]"));
    Ok(())
}

#[test]
fn malformed_usage_tables_are_rejected_and_left_untouched() -> anyhow::Result<()> {
    let temp = TempDirectory::new()?;
    let path = temp.0.join("config.toml");
    for (original, expected) in [
        ("usage = false\n", "usage must be a TOML table"),
        (
            "[usage]\nhide_providers = 'claude'\n",
            "usage.hide_providers must be a list of provider ids",
        ),
        (
            "[usage]\nhide_providers = ['claude', 3]\n",
            "usage.hide_providers must be a list of provider ids",
        ),
    ] {
        fs::write(&path, original)?;
        let error = Config::save_preference_path(
            Preference::UsageProviders(ProviderEdit::Disable("codex")),
            &path,
        )
        .err()
        .context("malformed usage settings must be rejected")?;
        assert!(
            std::error::Error::source(&error)
                .is_some_and(|source| { source.to_string() == expected }),
            "{error:?}"
        );
        assert_eq!(fs::read_to_string(&path)?, original);
    }
    Ok(())
}
