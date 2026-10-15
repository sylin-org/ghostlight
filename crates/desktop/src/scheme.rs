//! The private scheme the window is served through, the content policy it is served under and
//! the rules that keep the webview on it (ADR-0199 Decision 7).
//!
//! The shell renders only what it holds in memory. On Linux WebKitGTK registers the `ghostlight`
//! scheme itself and pages live at `ghostlight://localhost/<path>`. WebView2 cannot register a
//! scheme, so on Windows `wry` intercepts `https://ghostlight.localhost/<path>` before any lookup
//! or socket. Either way the handler sees the request rewritten back to the `ghostlight://`
//! form, and this module accepts both forms on every platform so the rules are tested everywhere.
//!
//! Every answer carries [`CONTENT_POLICY`], which allows nothing from the network, and every HTML
//! page also carries it as a meta element, so a webview that ignores a custom-scheme response
//! header still enforces it. [`harden`] applies the matching webview settings in one place.

use std::borrow::Cow;

use wry::http::{header, Response, StatusCode};
use wry::{NewWindowResponse, WebViewBuilder};

use crate::bundle::{validate_path, BundleSet};

/// The name of the private custom scheme.
pub const SCHEME: &str = "ghostlight";

/// The page origin on Windows, where WebView2 serves the scheme through an https host.
const ORIGIN_HTTPS: &str = "https://ghostlight.localhost";

/// The page origin where the webview registers the scheme itself.
const ORIGIN_SCHEME: &str = "ghostlight://localhost";

/// The path an empty request path maps to.
const INDEX_PATH: &str = "index.html";

/// The content policy of every answer. Nothing may load from the network: with `default-src
/// 'none'`, connect, font, frame, media, object, worker and manifest loads are all refused. The
/// shell's bridge is the webview's own message channel, not a network request.
pub const CONTENT_POLICY: &str = "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

/// The permissions policy value: no device access and no clipboard reads for any page.
const PERMISSIONS_POLICY: &str = "camera=(), microphone=(), geolocation=(), usb=(), serial=(), hid=(), bluetooth=(), clipboard-read=()";

const HEADER_CONTENT_SECURITY_POLICY: &str = "Content-Security-Policy";
const HEADER_CONTENT_TYPE_OPTIONS: &str = "X-Content-Type-Options";
const HEADER_CACHE_CONTROL: &str = "Cache-Control";
const HEADER_REFERRER_POLICY: &str = "Referrer-Policy";
const HEADER_PERMISSIONS_POLICY: &str = "Permissions-Policy";

const VALUE_NOSNIFF: &str = "nosniff";
const VALUE_NO_STORE: &str = "no-store";
const VALUE_NO_REFERRER: &str = "no-referrer";

/// The fixed headers every answer carries, as name and value.
const SECURITY_HEADERS: [(&str, &str); 5] = [
    (HEADER_CONTENT_SECURITY_POLICY, CONTENT_POLICY),
    (HEADER_CONTENT_TYPE_OPTIONS, VALUE_NOSNIFF),
    (HEADER_CACHE_CONTROL, VALUE_NO_STORE),
    (HEADER_REFERRER_POLICY, VALUE_NO_REFERRER),
    (HEADER_PERMISSIONS_POLICY, PERMISSIONS_POLICY),
];

const METHOD_GET: &str = "GET";
const METHOD_HEAD: &str = "HEAD";

/// The start of the HTML element the policy meta element goes into.
const HEAD_START: &[u8] = b"<head";

/// The media type prefix of HTML answers.
const HTML_TYPE: &str = "text/html";

/// The window ground colour, the night ground `#0f0e12`, so a window never flashes white.
const GROUND: (u8, u8, u8, u8) = (15, 14, 18, 255);

/// The origin pages are loaded from on this platform.
pub fn origin() -> &'static str {
    if cfg!(windows) {
        ORIGIN_HTTPS
    } else {
        ORIGIN_SCHEME
    }
}

/// The URL of the bundle file at `path` on this platform.
pub fn page_url(path: &str) -> String {
    format!("{}/{path}", origin())
}

/// The bundle path a request URI names, or `None` when the URI is not a page of this scheme.
///
/// Both origin forms are accepted on every platform. The query and the fragment are dropped, the
/// empty path becomes `index.html`, and anything else is refused: another host, `http` in place of
/// `https`, any percent escape, and any path the bundle would not hold.
pub fn request_path(uri: &str) -> Option<String> {
    let rest = [ORIGIN_SCHEME, ORIGIN_HTTPS]
        .iter()
        .find_map(|origin| uri.strip_prefix(origin)?.strip_prefix('/'))?;
    let path = rest.split(['?', '#']).next().unwrap_or_default();
    if path.contains('%') {
        return None;
    }
    let path = if path.is_empty() { INDEX_PATH } else { path };
    validate_path(path).ok()?;
    Some(path.to_string())
}

/// Whether the webview may navigate to `url`: only the origin itself or a path under it.
pub fn allows_navigation(origin: &str, url: &str) -> bool {
    url.strip_prefix(origin)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// Inserts the content policy as a meta element right after the first `<head ...>` start tag.
///
/// The tag name is matched ASCII case-insensitively and the tag may carry attributes. Returns
/// `None` when the document has no head start tag.
pub fn with_policy_meta(html: &[u8]) -> Option<Vec<u8>> {
    let start = html
        .windows(HEAD_START.len())
        .enumerate()
        .find(|(index, window)| {
            window.eq_ignore_ascii_case(HEAD_START)
                && html
                    .get(index + HEAD_START.len())
                    .is_some_and(|next| matches!(next, b'>' | b'/') || next.is_ascii_whitespace())
        })
        .map(|(index, _)| index)?;
    let close = start + html[start..].iter().position(|byte| *byte == b'>')? + 1;
    let meta = format!(
        "<meta http-equiv=\"{HEADER_CONTENT_SECURITY_POLICY}\" content=\"{CONTENT_POLICY}\">"
    );
    let mut out = Vec::with_capacity(html.len() + meta.len());
    out.extend_from_slice(&html[..close]);
    out.extend_from_slice(meta.as_bytes());
    out.extend_from_slice(&html[close..]);
    Some(out)
}

/// Answers one request of the private scheme from `bundles`.
///
/// Only `GET` and `HEAD` are served; every other method is 405. An unknown path is 404. A page is
/// never served without the policy: an HTML file with no head start tag is 500. Every answer
/// carries the fixed security headers, and `HEAD` sends them without the body.
pub fn respond(bundles: &BundleSet, method: &str, uri: &str) -> Response<Cow<'static, [u8]>> {
    if method != METHOD_GET && method != METHOD_HEAD {
        return answer(StatusCode::METHOD_NOT_ALLOWED, None, Vec::new());
    }
    let Some(file) = request_path(uri).and_then(|path| bundles.get(&path)) else {
        return answer(StatusCode::NOT_FOUND, None, Vec::new());
    };
    let body = if file.content_type.starts_with(HTML_TYPE) {
        match with_policy_meta(&file.bytes) {
            Some(body) => body,
            None => return answer(StatusCode::INTERNAL_SERVER_ERROR, None, Vec::new()),
        }
    } else {
        file.bytes.to_vec()
    };
    let body = if method == METHOD_HEAD {
        Vec::new()
    } else {
        body
    };
    answer(StatusCode::OK, Some(file.content_type), body)
}

fn answer(
    status: StatusCode,
    content_type: Option<&str>,
    body: Vec<u8>,
) -> Response<Cow<'static, [u8]>> {
    let mut builder = Response::builder().status(status);
    if let Some(content_type) = content_type {
        builder = builder.header(header::CONTENT_TYPE, content_type);
    }
    for (name, value) in SECURITY_HEADERS {
        builder = builder.header(name, value);
    }
    builder
        .body(Cow::Owned(body))
        .expect("fixed header names and values are valid")
}

/// Applies every webview setting that keeps the window on the private scheme and off the
/// network: navigation limited to [`origin`], new windows denied, downloads refused, no zoom
/// hotkeys, clipboard access or autofill, an in-private profile that writes no web data, the
/// night ground, and devtools only in debug builds. On Windows it also selects the https scheme
/// form WebView2 needs. This is checked live, not in `cargo test`.
pub fn harden(builder: WebViewBuilder<'_>) -> WebViewBuilder<'_> {
    let builder = builder
        .with_navigation_handler(|url| allows_navigation(origin(), &url))
        .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
        .with_download_started_handler(|_, _| false)
        .with_devtools(cfg!(debug_assertions))
        .with_hotkeys_zoom(false)
        .with_clipboard(false)
        .with_general_autofill_enabled(false)
        .with_incognito(true)
        .with_background_color(GROUND);
    #[cfg(windows)]
    let builder = wry::WebViewBuilderExtWindows::with_https_scheme(builder, true);
    builder
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::Bundle;

    fn file(path: &str, body: &str) -> (String, Vec<u8>) {
        (path.to_string(), body.as_bytes().to_vec())
    }

    fn set() -> BundleSet {
        let shell = Bundle::shell(vec![file("_shell/s.css", "body{}")]).unwrap();
        let service = Bundle::service(vec![
            file(
                "index.html",
                "<!doctype html><html><head><title>x</title></head></html>",
            ),
            file("bare.html", "<html><body>no head</body></html>"),
            file("a.css", "a { color: red }\n"),
        ])
        .unwrap();
        let mut set = BundleSet::new(shell);
        set.set_service(service);
        set
    }

    fn header_value<'a>(response: &'a Response<Cow<'static, [u8]>>, name: &str) -> Option<&'a str> {
        response.headers().get(name).and_then(|v| v.to_str().ok())
    }

    #[test]
    fn both_origin_forms_name_a_path() {
        assert_eq!(
            request_path("ghostlight://localhost/a/b.css").as_deref(),
            Some("a/b.css")
        );
        assert_eq!(
            request_path("https://ghostlight.localhost/a/b.css").as_deref(),
            Some("a/b.css")
        );
    }

    #[test]
    fn the_query_and_fragment_are_dropped_and_empty_is_the_index() {
        assert_eq!(
            request_path("ghostlight://localhost/a.css?v=1#top").as_deref(),
            Some("a.css")
        );
        assert_eq!(
            request_path("https://ghostlight.localhost/").as_deref(),
            Some("index.html")
        );
        assert_eq!(
            request_path("ghostlight://localhost/?x=1").as_deref(),
            Some("index.html")
        );
    }

    #[test]
    fn escapes_foreign_hosts_and_bad_paths_are_refused() {
        for uri in [
            "ghostlight://localhost/%2e%2e/x",
            "ghostlight://localhost/a%2Fb",
            "ghostlight://evil/x",
            "http://ghostlight.localhost/x",
            "https://ghostlight.localhost.evil.example/x",
            "https://ghostlight.localhostx/x",
            "ghostlight://localhost",
            "ghostlight://localhost/../x.html",
            "ghostlight://localhost/a//b.html",
            "about:blank",
            "",
        ] {
            assert_eq!(request_path(uri), None, "{uri}");
        }
    }

    #[test]
    fn page_url_joins_the_platform_origin_and_the_path() {
        assert_eq!(page_url("x.html"), format!("{}/x.html", origin()));
        assert_eq!(request_path(&page_url("x.html")).as_deref(), Some("x.html"));
    }

    #[test]
    fn an_absent_path_is_404_and_a_foreign_uri_is_404() {
        let set = set();
        assert_eq!(
            respond(&set, "GET", "ghostlight://localhost/missing.css").status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            respond(&set, "GET", "ghostlight://evil/a.css").status(),
            StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn a_method_other_than_get_or_head_is_405_with_no_body() {
        let response = respond(&set(), "POST", "ghostlight://localhost/a.css");
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert!(response.body().is_empty());
    }

    #[test]
    fn a_found_file_carries_the_policy_and_the_fixed_headers() {
        let response = respond(&set(), "GET", "https://ghostlight.localhost/a.css");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            header_value(&response, "content-type"),
            Some("text/css; charset=utf-8")
        );
        assert_eq!(
            header_value(&response, "content-security-policy"),
            Some(CONTENT_POLICY)
        );
        assert_eq!(
            header_value(&response, "x-content-type-options"),
            Some("nosniff")
        );
        assert_eq!(header_value(&response, "cache-control"), Some("no-store"));
        assert_eq!(
            header_value(&response, "referrer-policy"),
            Some("no-referrer")
        );
        assert_eq!(
            header_value(&response, "permissions-policy"),
            Some(PERMISSIONS_POLICY)
        );
    }

    #[test]
    fn a_css_file_is_served_byte_for_byte() {
        let response = respond(&set(), "GET", "ghostlight://localhost/a.css");
        assert_eq!(&**response.body(), b"a { color: red }\n");
    }

    #[test]
    fn a_shell_file_is_served_from_the_shell_bundle() {
        let response = respond(&set(), "GET", "ghostlight://localhost/_shell/s.css");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(&**response.body(), b"body{}");
    }

    #[test]
    fn head_sends_the_headers_without_the_body() {
        let response = respond(&set(), "HEAD", "ghostlight://localhost/a.css");
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.body().is_empty());
        assert_eq!(
            header_value(&response, "content-security-policy"),
            Some(CONTENT_POLICY)
        );
        assert_eq!(
            header_value(&response, "content-type"),
            Some("text/css; charset=utf-8")
        );
    }

    #[test]
    fn an_html_file_gains_one_meta_element_right_after_its_head_tag() {
        let response = respond(&set(), "GET", "ghostlight://localhost/");
        let body = String::from_utf8(response.body().to_vec()).unwrap();
        let meta =
            format!("<meta http-equiv=\"Content-Security-Policy\" content=\"{CONTENT_POLICY}\">");
        assert_eq!(body.matches("<meta").count(), 1);
        assert!(body.contains(&format!("<head>{meta}<title>")));
    }

    #[test]
    fn the_meta_element_follows_a_head_tag_with_attributes_in_any_case() {
        let out = with_policy_meta(b"<HTML><HEAD lang=\"en\"><title>x</title></HEAD>").unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("<HTML><HEAD lang=\"en\"><meta http-equiv="));
        assert!(text.ends_with("><title>x</title></HEAD>"));
    }

    #[test]
    fn a_header_element_is_not_a_head_tag() {
        assert_eq!(with_policy_meta(b"<html><header>x</header></html>"), None);
    }

    #[test]
    fn html_without_a_head_is_500_and_never_served_bare() {
        let response = respond(&set(), "GET", "ghostlight://localhost/bare.html");
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(response.body().is_empty());
    }

    #[test]
    fn navigation_is_allowed_to_the_origin_and_its_subpaths_only() {
        for origin in [ORIGIN_HTTPS, ORIGIN_SCHEME] {
            assert!(allows_navigation(origin, origin), "{origin}");
            assert!(allows_navigation(origin, &format!("{origin}/")), "{origin}");
            assert!(
                allows_navigation(origin, &format!("{origin}/a/b.html?x=1")),
                "{origin}"
            );
        }
    }

    #[test]
    fn confusable_hosts_other_schemes_and_blank_are_not_navigable() {
        for url in [
            "https://ghostlight.localhost.example/",
            "https://ghostlight.localhostx/",
            "https://ghostlight.localhost:8080/",
            "https://ghostlight.localhost@evil.example/",
            "http://ghostlight.localhost/",
            "ghostlight://localhost.evil/",
            "https://example.com/",
            "about:blank",
            "",
        ] {
            assert!(!allows_navigation(ORIGIN_HTTPS, url), "{url}");
            assert!(!allows_navigation(ORIGIN_SCHEME, url), "{url}");
        }
    }
}
