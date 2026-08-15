use serde::{Deserialize, Serialize};
use std::sync::Arc;

use reqwest::cookie::{CookieStore, Jar};
use url::Url;

/// A single cookie name/value pair, suitable for display in the UI (Req 8.2).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CookiePair {
    pub name: String,
    pub value: String,
}

/// Handle to an inspectable cookie jar shared across the session (Req 8.1, 8.2, 8.7).
///
/// Wraps `Arc<reqwest::cookie::Jar>` so the same jar instance can be given to
/// `reqwest::Client::builder().cookie_provider(..)` while also allowing the UI
/// to read stored cookies for display in the "Cookies" tab.
#[derive(Clone)]
pub struct CookieJarHandle {
    jar: Arc<Jar>,
}

impl CookieJarHandle {
    /// Creates a new empty cookie jar.
    pub fn new() -> Self {
        Self {
            jar: Arc::new(Jar::default()),
        }
    }

    /// Returns the `Arc<Jar>` for passing to `Client::builder().cookie_provider(..)`.
    pub fn provider(&self) -> Arc<Jar> {
        Arc::clone(&self.jar)
    }

    /// Reads cookies applicable to `url`, excluding expired ones.
    ///
    /// Returns an empty `Vec` (not an error) if:
    /// - No cookies are stored for the URL
    /// - The URL is invalid / cannot be parsed
    ///
    /// Expired cookies are already excluded by the Jar's internal `CookieStore`
    /// implementation when reading via `cookies()`.
    pub fn read_for_url(&self, url: &str) -> Vec<CookiePair> {
        let parsed = match Url::parse(url) {
            Ok(u) => u,
            Err(_) => return Vec::new(),
        };

        // Jar::cookies returns the Cookie header value for the given URL,
        // e.g. "name1=value1; name2=value2". Returns None if no cookies apply.
        let header_value = match self.jar.cookies(&parsed) {
            Some(val) => val,
            None => return Vec::new(),
        };

        let header_str = match header_value.to_str() {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        if header_str.is_empty() {
            return Vec::new();
        }

        header_str
            .split("; ")
            .filter_map(|pair: &str| {
                let mut parts = pair.splitn(2, '=');
                let name = parts.next()?.trim().to_string();
                let value = parts.next().unwrap_or("").trim().to_string();
                if name.is_empty() {
                    None
                } else {
                    Some(CookiePair { name, value })
                }
            })
            .collect()
    }
}

impl Default for CookieJarHandle {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_jar_returns_empty_cookies() {
        let handle = CookieJarHandle::new();
        let cookies = handle.read_for_url("https://example.com");
        assert!(cookies.is_empty());
    }

    #[test]
    fn invalid_url_returns_empty_vec() {
        let handle = CookieJarHandle::new();
        let cookies = handle.read_for_url("not a valid url");
        assert!(cookies.is_empty());
    }

    #[test]
    fn provider_returns_cloned_arc() {
        let handle = CookieJarHandle::new();
        let p1 = handle.provider();
        let p2 = handle.provider();
        // Both point to the same underlying Jar
        assert!(Arc::ptr_eq(&p1, &p2));
    }

    #[test]
    fn reads_cookies_set_via_jar() {
        let handle = CookieJarHandle::new();
        let url = "https://example.com/path".parse::<Url>().unwrap();

        // Simulate a Set-Cookie by adding directly to the jar
        handle
            .jar
            .add_cookie_str("session_id=abc123; Path=/", &url);
        handle
            .jar
            .add_cookie_str("tracking=xyz; Path=/", &url);

        let cookies = handle.read_for_url("https://example.com/path");
        assert_eq!(cookies.len(), 2);

        let names: Vec<&str> = cookies.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"session_id"));
        assert!(names.contains(&"tracking"));

        let session = cookies.iter().find(|c| c.name == "session_id").unwrap();
        assert_eq!(session.value, "abc123");
    }

    #[test]
    fn cookies_scoped_by_domain() {
        let handle = CookieJarHandle::new();
        let url_a = "https://a.example.com/".parse::<Url>().unwrap();
        let url_b = "https://b.example.com/".parse::<Url>().unwrap();

        handle.jar.add_cookie_str("key=val_a; Path=/", &url_a);
        handle.jar.add_cookie_str("key=val_b; Path=/", &url_b);

        let cookies_a = handle.read_for_url("https://a.example.com/");
        assert_eq!(cookies_a.len(), 1);
        assert_eq!(cookies_a[0].value, "val_a");

        let cookies_b = handle.read_for_url("https://b.example.com/");
        assert_eq!(cookies_b.len(), 1);
        assert_eq!(cookies_b[0].value, "val_b");
    }

    #[test]
    fn empty_string_url_returns_empty_vec() {
        let handle = CookieJarHandle::new();
        let cookies = handle.read_for_url("");
        assert!(cookies.is_empty());
    }
}
