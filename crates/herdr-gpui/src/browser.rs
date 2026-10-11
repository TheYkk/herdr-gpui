//! Browser tabs: web pages shown in place of the terminal, or beside it in
//! another editor group, next to a workspace's Herdr tabs. Herdr panes are always
//! terminals, so these tabs belong to this client alone; the daemon and its
//! other clients never see them. Pages are native web views drawn above the window, which is why the
//! window hides them whenever one of its own overlays is open. Each workspace
//! may also show VS Code in a panel to the right of its groups, or as a tab
//! in its groups.

#[cfg(any(target_os = "macos", windows, test))]
mod annotate;
// Linux builds show no pages, so there is nothing to annotate there.
#[cfg(any(target_os = "macos", windows))]
mod annotate_view;
mod code;
mod code_group;
mod code_start;
mod code_view;
mod feedback;
mod group_motion;
mod groups;
mod groups_view;
mod layouts;
mod location;
#[cfg(any(target_os = "macos", windows))]
mod native;
#[cfg(target_os = "macos")]
mod page_keys;
#[cfg(target_os = "macos")]
mod popup;
#[cfg(any(target_os = "macos", windows))]
mod preview;
#[cfg(target_os = "macos")]
mod snapshot;
mod store;
mod tab_appear;
mod tab_scroll;
mod view;
#[cfg(test)]
mod view_tests;

#[cfg(any(target_os = "macos", windows))]
pub(crate) use annotate_view::Annotations;
pub(crate) use feedback::{Batch, Feedback, FeedbackKey};
pub(crate) use group_motion::Fold;
#[cfg(test)]
pub(crate) use groups::GroupIds;
pub(crate) use groups::{GroupId, Pick, Shown, Slot};
pub(crate) use layouts::Layouts;
pub(crate) use location::{LocalFile, Location, OrchestratorRepo, ReviewCheckout};
#[cfg(any(target_os = "macos", windows))]
pub(crate) use native::Pages;
pub(crate) use store::{Scope, Store, Tab, TabId};
pub(crate) use tab_appear::{Leaving, Listed};
pub(crate) use tab_scroll::{Thumb, ThumbDrag};
pub(crate) use view::{Browser, scope};

/// Whether this build can show a page inside the window. Elsewhere a browser
/// tab request opens the system browser instead.
pub(crate) const EMBEDDED: bool = cfg!(any(target_os = "macos", windows));

const MAX_URL_BYTES: usize = 8192;

/// An address a browser tab may show: http or https with a host, bounded,
/// and free of whitespace and control characters. Anything that reaches a
/// page, from a terminal link, an agent, or the address field, is parsed into
/// this first.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub(crate) struct WebUrl(url::Url);

impl WebUrl {
    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub(crate) fn host(&self) -> &str {
        self.0.host_str().unwrap_or_default()
    }

    /// The scheme, host, and port, as in `http://127.0.0.1:8000`.
    pub(crate) fn origin(&self) -> String {
        self.0.origin().ascii_serialization()
    }

    /// The host and port, as in `127.0.0.1:8000`: what to name in a message,
    /// since the path and query may hold a secret such as a token.
    pub(crate) fn address(&self) -> String {
        match self.0.port_or_known_default() {
            Some(port) => format!("{}:{port}", self.host()),
            None => self.host().to_owned(),
        }
    }

    /// The port of an address that names this machine's own loopback
    /// (`localhost`, `127.0.0.1`, `[::1]`), or None for any other host. On a
    /// remote workspace such an address means the remote host's loopback.
    pub(crate) fn loopback_port(&self) -> Option<u16> {
        let loopback = match self.0.host()? {
            url::Host::Domain(name) => name.eq_ignore_ascii_case("localhost"),
            url::Host::Ipv4(address) => address.is_loopback(),
            url::Host::Ipv6(address) => address.is_loopback(),
        };
        loopback.then(|| self.0.port_or_known_default()).flatten()
    }

    /// The same page reached at `localhost:port`, as an SSH tunnel's local
    /// end serves it; the scheme, path, query, and fragment are kept.
    pub(crate) fn at_local_port(&self, port: u16) -> crate::Result<Self> {
        let mut url = self.0.clone();
        if url.set_host(Some("localhost")).is_err() || url.set_port(Some(port)).is_err() {
            return Err(crate::Error::InvalidBrowserUrl);
        }
        Self::try_from(url.as_str())
    }

    /// What someone typed into the address field: a bare host such as
    /// `localhost:3000` or `example.com/docs` gets a scheme, local hosts plain
    /// http and everything else https.
    pub(crate) fn from_typed(text: &str) -> crate::Result<Self> {
        let text = text.trim();
        if let Ok(url) = Self::try_from(text) {
            return Ok(url);
        }
        if text.contains("://") {
            return Err(crate::Error::InvalidBrowserUrl);
        }
        let host = text.split(['/', ':', '?', '#']).next().unwrap_or_default();
        let local =
            matches!(host, "localhost" | "127.0.0.1" | "[::1]") || host.ends_with(".localhost");
        let scheme = if local { "http" } else { "https" };
        Self::try_from(format!("{scheme}://{text}").as_str())
    }
}

impl TryFrom<&str> for WebUrl {
    type Error = crate::Error;

    fn try_from(value: &str) -> crate::Result<Self> {
        if value.len() > MAX_URL_BYTES || value.chars().any(|c| c.is_control() || c.is_whitespace())
        {
            return Err(crate::Error::InvalidBrowserUrl);
        }
        let url = url::Url::parse(value).map_err(|_| crate::Error::InvalidBrowserUrl)?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none_or(str::is_empty) {
            return Err(crate::Error::InvalidBrowserUrl);
        }
        // Serialization can lengthen an address, for example by
        // percent-encoding, so the bound applies to what a page is given too.
        if url.as_str().len() > MAX_URL_BYTES {
            return Err(crate::Error::InvalidBrowserUrl);
        }
        Ok(Self(url))
    }
}

impl TryFrom<String> for WebUrl {
    type Error = crate::Error;

    fn try_from(value: String) -> crate::Result<Self> {
        Self::try_from(value.as_str())
    }
}

impl From<WebUrl> for String {
    fn from(url: WebUrl) -> Self {
        url.0.into()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn only_bounded_web_addresses_with_a_host_are_accepted() {
        for valid in [
            "http://localhost:3000/",
            "https://example.com/a?b#c",
            "https://[::1]:8080/",
        ] {
            assert!(WebUrl::try_from(valid).is_ok(), "{valid}");
        }
        for invalid in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,hi",
            "vscode://open",
            "https://",
            "https://a.test/\nb",
            "https://a.test/ b",
            "not a url",
        ] {
            assert!(
                matches!(
                    WebUrl::try_from(invalid),
                    Err(crate::Error::InvalidBrowserUrl)
                ),
                "{invalid}"
            );
        }
        let long = format!("https://a.test/{}", "x".repeat(MAX_URL_BYTES));
        assert!(WebUrl::try_from(long.as_str()).is_err());
    }

    #[test]
    fn loopback_addresses_move_to_a_tunnel_port() {
        let url = |text| WebUrl::try_from(text).unwrap();
        for (text, port) in [
            ("http://localhost:3000/", 3000),
            ("http://LOCALHOST/", 80),
            ("https://localhost/x", 443),
            ("http://127.0.0.1:8000/", 8000),
            ("http://127.1.2.3:8000/", 8000),
            ("http://[::1]:5173/", 5173),
        ] {
            assert_eq!(url(text).loopback_port(), Some(port), "{text}");
        }
        for text in [
            "http://example.com:3000/",
            "http://192.168.1.5:3000/",
            "http://app.localhost:3000/",
            "http://[::2]:3000/",
        ] {
            assert_eq!(url(text).loopback_port(), None, "{text}");
        }
        assert_eq!(
            url("https://[::1]:5173/a/b?c=1#d")
                .at_local_port(41000)
                .unwrap()
                .as_str(),
            "https://localhost:41000/a/b?c=1#d"
        );
        assert_eq!(
            url("http://127.0.0.1:8000/")
                .at_local_port(41001)
                .unwrap()
                .as_str(),
            "http://localhost:41001/"
        );
    }

    #[test]
    fn typed_addresses_gain_a_scheme() {
        let typed = |text| WebUrl::from_typed(text).map(|url| url.as_str().to_owned());
        assert_eq!(typed("localhost:3000").unwrap(), "http://localhost:3000/");
        assert_eq!(typed("app.localhost/x").unwrap(), "http://app.localhost/x");
        assert_eq!(
            typed(" example.com/docs ").unwrap(),
            "https://example.com/docs"
        );
        assert_eq!(typed("http://a.test").unwrap(), "http://a.test/");
        assert!(typed("file:///etc/passwd").is_err());
        assert!(typed("ftp://a.test").is_err());
        assert!(typed("").is_err());
    }

    #[test]
    fn saved_addresses_are_validated_when_read() {
        assert!(serde_json::from_str::<WebUrl>(r#""https://a.test/""#).is_ok());
        assert!(serde_json::from_str::<WebUrl>(r#""file:///etc/passwd""#).is_err());
    }
}
