//! The closed route table, the context gate and the answer to each route.
//!
//! Every request passes the same gates in the same order: the `Host` header, protocol upgrades and
//! framing, the route, the context the request is in, the checks every change must pass, the
//! route's answer for that context, the rate limit of a watcher, and the body. A route not in the
//! table is not served, and a context a route does not name is refused (ADR-0199, amendment of
//! 2026-10-15).

use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use super::assets::{self, Asset};
use super::http::{header, Head, Incoming, Method, Response, Status};
use super::limits::retry_after_seconds;
use super::{same_secret, Shared};
use crate::install::HarnessAction;
use crate::language::workbench as words;
use crate::workbench::context::WebContext;
use crate::workbench::parts::{Audience, Part, Projection};
use crate::workbench::{
    validate_policy_document, validate_search_query, HarnessActError, PolicyWriteError,
    WorkbenchRuntimeIntent,
};

/// The prefix of every route that carries data or takes a change.
const API_PREFIX: &str = "/api/";

/// The bearer scheme, with the space that follows it.
const BEARER_PREFIX: &str = "Bearer ";

/// The authentication scheme a refused request is told to use.
const BEARER_SCHEME: &str = "Bearer";

/// The type of every JSON body.
const JSON_TYPE: &str = "application/json";

/// The type sent with a JSON body.
const JSON_RESPONSE_TYPE: &str = "application/json; charset=utf-8";

/// The most a small request body holds.
const SMALL_BODY: usize = 4 * 1024;

/// The most a request body holding a policy document holds.
const POLICY_BODY: usize = 256 * 1024;

/// The value of Sec-Fetch-Site for a request from the same origin.
const FETCH_SAME_ORIGIN: &str = "same-origin";

/// The value of Sec-Fetch-Site for a request the person typed or bookmarked.
const FETCH_NONE: &str = "none";

/// How a page is cached: kept, but checked against its tag every time.
const PAGE_CACHE: &str = "no-cache";

/// How a data response is cached: not at all.
const API_CACHE: &str = "no-store";

const NOSNIFF: &str = "nosniff";
const NO_REFERRER: &str = "no-referrer";
const SAME_ORIGIN_ONLY: &str = "same-origin";
const DENY_FRAMES: &str = "DENY";

/// What a page may load and do: its own origin, no frames, no plugins, no forms.
const PAGE_POLICY: &str = "default-src 'self'; connect-src 'self'; img-src 'self' data:; \
style-src 'self'; script-src 'self'; object-src 'none'; frame-src 'none'; base-uri 'none'; \
form-action 'none'; frame-ancestors 'none'";

/// Every route that is not a page file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Api {
    Context,
    Snapshot,
    Policy,
    Search,
    PolicyPreview,
    Rescan,
    Window,
    Runtime,
    PutPolicy,
    DeletePolicy,
    SetDiagnostics,
    SetupAll,
    HarnessAct,
    TestNotification,
}

/// What a watcher may do on a route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WatchAccess {
    /// The route answers a watcher.
    Open,
    /// The route answers the window only.
    NeedsWindow,
}

struct Route {
    api: Api,
    method: Method,
    path: &'static str,
    watch: WatchAccess,
    body_cap: usize,
}

const fn route(
    api: Api,
    method: Method,
    path: &'static str,
    watch: WatchAccess,
    body_cap: usize,
) -> Route {
    Route {
        api,
        method,
        path,
        watch,
        body_cap,
    }
}

/// The closed table. Anything not here is not served.
static ROUTES: [Route; 14] = [
    route(
        Api::Context,
        Method::Get,
        "/api/context",
        WatchAccess::Open,
        0,
    ),
    route(
        Api::Snapshot,
        Method::Get,
        "/api/snapshot",
        WatchAccess::Open,
        0,
    ),
    route(
        Api::Policy,
        Method::Get,
        "/api/policy",
        WatchAccess::Open,
        0,
    ),
    route(
        Api::Search,
        Method::Post,
        "/api/search",
        WatchAccess::Open,
        SMALL_BODY,
    ),
    route(
        Api::PolicyPreview,
        Method::Post,
        "/api/policy/preview",
        WatchAccess::Open,
        POLICY_BODY,
    ),
    route(
        Api::Rescan,
        Method::Post,
        "/api/integrations/rescan",
        WatchAccess::Open,
        SMALL_BODY,
    ),
    route(
        Api::Window,
        Method::Post,
        "/api/window",
        WatchAccess::Open,
        SMALL_BODY,
    ),
    route(
        Api::Runtime,
        Method::Post,
        "/api/runtime",
        WatchAccess::Open,
        SMALL_BODY,
    ),
    route(
        Api::PutPolicy,
        Method::Put,
        "/api/policy/user",
        WatchAccess::NeedsWindow,
        POLICY_BODY,
    ),
    route(
        Api::DeletePolicy,
        Method::Delete,
        "/api/policy/user",
        WatchAccess::NeedsWindow,
        SMALL_BODY,
    ),
    route(
        Api::SetDiagnostics,
        Method::Put,
        "/api/controls/diagnostics",
        WatchAccess::NeedsWindow,
        SMALL_BODY,
    ),
    route(
        Api::SetupAll,
        Method::Post,
        "/api/integrations/setup-all",
        WatchAccess::NeedsWindow,
        SMALL_BODY,
    ),
    route(
        Api::HarnessAct,
        Method::Post,
        "/api/integrations/act",
        WatchAccess::NeedsWindow,
        SMALL_BODY,
    ),
    route(
        Api::TestNotification,
        Method::Post,
        "/api/notifications/test",
        WatchAccess::NeedsWindow,
        SMALL_BODY,
    ),
];

/// The closed codes an error body carries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ErrorCode {
    WrongKey,
    NeedsWindow,
    WatchingOff,
    TooMany,
    TooLarge,
    BadRequest,
    UnsupportedType,
    CrossSite,
    NotFound,
    MethodNotAllowed,
    Misdirected,
    Stale,
    Refused,
}

impl ErrorCode {
    const fn token(self) -> &'static str {
        match self {
            Self::WrongKey => "wrong_key",
            Self::NeedsWindow => "needs_window",
            Self::WatchingOff => "watching_off",
            Self::TooMany => "too_many",
            Self::TooLarge => "too_large",
            Self::BadRequest => "bad_request",
            Self::UnsupportedType => "unsupported_type",
            Self::CrossSite => "cross_site",
            Self::NotFound => "not_found",
            Self::MethodNotAllowed => "method_not_allowed",
            Self::Misdirected => "misdirected",
            Self::Stale => "stale",
            Self::Refused => "refused",
        }
    }
}

fn json_response(status: Status, body: &Value) -> Response {
    Response::new(status).with_body(
        JSON_RESPONSE_TYPE,
        serde_json::to_vec(body).expect("a JSON value serializes"),
    )
}

fn failure(status: Status, code: ErrorCode, message: &str) -> Response {
    json_response(
        status,
        &json!({ "error": { "code": code.token(), "message": message } }),
    )
}

fn needs_window() -> Response {
    failure(
        Status::Unauthorized,
        ErrorCode::NeedsWindow,
        words::CHANGE_IN_WINDOW,
    )
    .with_header(header::WWW_AUTHENTICATE, BEARER_SCHEME)
}

fn wrong_key() -> Response {
    failure(Status::Unauthorized, ErrorCode::WrongKey, words::WRONG_KEY)
        .with_header(header::WWW_AUTHENTICATE, BEARER_SCHEME)
}

fn watching_off() -> Response {
    failure(
        Status::Forbidden,
        ErrorCode::WatchingOff,
        words::WATCHING_OFF,
    )
}

fn too_many(wait: Duration) -> Response {
    failure(Status::TooManyRequests, ErrorCode::TooMany, words::TOO_MANY)
        .with_header(header::RETRY_AFTER, retry_after_seconds(wait).to_string())
}

fn bad_request() -> Response {
    failure(
        Status::BadRequest,
        ErrorCode::BadRequest,
        words::BAD_REQUEST,
    )
}

fn refused(message: &str) -> Response {
    failure(Status::Unprocessable, ErrorCode::Refused, message)
}

/// The answer to a request the server could not read.
pub(super) fn unreadable() -> Response {
    decorate(bad_request(), "")
}

/// Add the headers every response carries.
fn decorate(response: Response, path: &str) -> Response {
    let mut response = response
        .with_header(header::NOSNIFF, NOSNIFF)
        .with_header(header::REFERRER_POLICY, NO_REFERRER)
        .with_header(header::RESOURCE_POLICY, SAME_ORIGIN_ONLY);
    if path.starts_with(API_PREFIX) {
        response = response.with_header(header::CACHE_CONTROL, API_CACHE);
    }
    if response
        .header(header::CONTENT_TYPE)
        .is_some_and(|kind| kind.starts_with("text/html"))
    {
        response = response
            .with_header(header::FRAME_OPTIONS, DENY_FRAMES)
            .with_header(header::CONTENT_SECURITY_POLICY, PAGE_POLICY);
    }
    response
}

/// Answer one request.
pub(super) fn answer(shared: &Shared, head: &Head, incoming: &mut Incoming) -> Response {
    decorate(respond(shared, head, incoming), &head.path)
}

/// Whether a path could name a file outside the table.
fn unsafe_path(path: &str) -> bool {
    path.contains("..") || path.contains('%') || path.contains('\\')
}

fn respond(shared: &Shared, head: &Head, incoming: &mut Incoming) -> Response {
    // 2. Only this server's own address is served.
    let own = format!("127.0.0.1:{}", shared.port);
    let localhost = format!("localhost:{}", shared.port);
    match head.header(header::HOST) {
        Some(host) if host == own => {}
        Some(host)
            if host == localhost
                && matches!(head.method, Method::Get | Method::Head)
                && !head.path.starts_with(API_PREFIX) =>
        {
            return Response::new(Status::PermanentRedirect)
                .with_header(header::LOCATION, format!("http://{own}{}", head.path));
        }
        _ => {
            return failure(
                Status::Misdirected,
                ErrorCode::Misdirected,
                words::MISDIRECTED,
            );
        }
    }

    // 3. No protocol change, no framing the server does not read, no preflight.
    if head.header(header::UPGRADE).is_some() || head.header(header::TRANSFER_ENCODING).is_some() {
        return bad_request();
    }
    if head.method == Method::Options || head.method == Method::Other {
        return failure(
            Status::MethodNotAllowed,
            ErrorCode::MethodNotAllowed,
            words::METHOD_NOT_ALLOWED,
        );
    }

    // 4. The route.
    let target = match find(head) {
        Ok(target) => target,
        Err(response) => return response,
    };

    // 5. The context.
    let context = match context_of(shared, head) {
        Ok(context) => context,
        Err(response) => return response,
    };

    // 6. Every change must be JSON from this origin.
    if matches!(head.method, Method::Post | Method::Put | Method::Delete) {
        if let Err(response) = check_change(shared, head) {
            return response;
        }
    }

    match target {
        Target::Page(asset) => page(head, asset),
        Target::Api(route) => api(shared, route, context, head, incoming),
    }
}

enum Target {
    Page(&'static Asset),
    Api(&'static Route),
}

/// Find the route a request names, or the refusal that says why there is none.
fn find(head: &Head) -> Result<Target, Response> {
    let not_found = || failure(Status::NotFound, ErrorCode::NotFound, words::NOT_FOUND);
    if unsafe_path(&head.path) {
        return Err(not_found());
    }
    if let Some(asset) = assets::lookup(&head.path) {
        return if matches!(head.method, Method::Get | Method::Head) {
            Ok(Target::Page(asset))
        } else {
            Err(method_not_allowed(&[Method::Get, Method::Head]))
        };
    }
    let mut allowed = Vec::new();
    for route in &ROUTES {
        if route.path != head.path {
            continue;
        }
        if route.method == head.method {
            return Ok(Target::Api(route));
        }
        allowed.push(route.method);
    }
    if allowed.is_empty() {
        Err(not_found())
    } else {
        Err(method_not_allowed(&allowed))
    }
}

fn method_not_allowed(allowed: &[Method]) -> Response {
    let list: Vec<&str> = allowed.iter().map(|method| method.token()).collect();
    failure(
        Status::MethodNotAllowed,
        ErrorCode::MethodNotAllowed,
        words::METHOD_NOT_ALLOWED,
    )
    .with_header(header::ALLOW, list.join(", "))
}

/// The context of a request: the bearer makes it Control; none makes it Watch or Closed.
fn context_of(shared: &Shared, head: &Head) -> Result<WebContext, Response> {
    match head.header(header::AUTHORIZATION) {
        None => Ok(if shared.watching.is_on() {
            WebContext::Watch
        } else {
            WebContext::Closed
        }),
        Some(value) => {
            let carried = value.strip_prefix(BEARER_PREFIX);
            if carried.is_some_and(|token| same_secret(token.as_bytes(), shared.bearer.as_bytes()))
            {
                Ok(WebContext::Control)
            } else {
                shared
                    .wrong_keys
                    .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                Err(wrong_key())
            }
        }
    }
}

/// A change is JSON, and comes from this origin when the browser says where it comes from.
fn check_change(shared: &Shared, head: &Head) -> Result<(), Response> {
    let has_body = head.content_length > 0;
    if head.method != Method::Delete || has_body {
        let json = head.header(header::CONTENT_TYPE).is_some_and(|kind| {
            kind.split(';')
                .next()
                .is_some_and(|media| media.trim().eq_ignore_ascii_case(JSON_TYPE))
        });
        if !json {
            return Err(failure(
                Status::UnsupportedMediaType,
                ErrorCode::UnsupportedType,
                words::UNSUPPORTED_TYPE,
            ));
        }
    }
    let cross_site = || failure(Status::Forbidden, ErrorCode::CrossSite, words::CROSS_SITE);
    let origin = format!("http://127.0.0.1:{}", shared.port);
    if head
        .header(header::ORIGIN)
        .is_some_and(|value| value != origin)
    {
        return Err(cross_site());
    }
    if head
        .header(header::SEC_FETCH_SITE)
        .is_some_and(|value| value != FETCH_SAME_ORIGIN && value != FETCH_NONE)
    {
        return Err(cross_site());
    }
    Ok(())
}

fn page(head: &Head, asset: &'static Asset) -> Response {
    let tag = assets::etag(asset);
    let unchanged = head
        .header(header::IF_NONE_MATCH)
        .is_some_and(|value| value == tag);
    let response = if unchanged {
        Response::new(Status::NotModified)
    } else {
        Response::new(Status::Ok).with_body(asset.kind.media_type(), asset.bytes.to_vec())
    };
    let response = response
        .with_header(header::ETAG, tag)
        .with_header(header::CACHE_CONTROL, PAGE_CACHE);
    if head.method == Method::Head {
        response.head_only()
    } else {
        response
    }
}

fn api(
    shared: &Shared,
    route: &Route,
    context: WebContext,
    head: &Head,
    incoming: &mut Incoming,
) -> Response {
    // 7. The route's answer for the context.
    let audience = match context {
        WebContext::Control => Audience::Control,
        WebContext::Watch => {
            if route.watch == WatchAccess::NeedsWindow {
                return needs_window();
            }
            Audience::Watch
        }
        WebContext::Closed => {
            if route.api == Api::Context {
                return json_response(Status::Ok, &shared.facade.context_document(context));
            }
            return watching_off();
        }
    };
    let projection = Projection::new(audience);
    let watching = audience == Audience::Watch;

    // 8. A watcher's rate limit.
    let mut preview_permit = None;
    if watching {
        match route.api {
            Api::Search => {
                if let Err(wait) = shared.limits.search() {
                    return too_many(wait);
                }
            }
            Api::PolicyPreview => match shared.limits.preview() {
                Ok(permit) => preview_permit = Some(permit),
                Err(wait) => return too_many(wait),
            },
            Api::Window => {
                if let Err(wait) = shared.limits.window() {
                    return too_many(wait);
                }
            }
            _ => {}
        }
    }

    // 9. The body.
    let body = match read_body(route, head, incoming) {
        Ok(body) => body,
        Err(response) => return response,
    };

    let response = match route.api {
        Api::Context => json_response(Status::Ok, &shared.facade.context_document(context)),
        Api::Snapshot => snapshot(shared, head, &projection),
        Api::Policy => json_response(Status::Ok, &projection.policy(&shared.facade.policy())),
        Api::Search => search(shared, &body),
        Api::PolicyPreview => preview(shared, &body),
        Api::Rescan => rescan(shared, &projection, watching),
        Api::Window => window(shared, &body),
        Api::Runtime => runtime(shared, &body, watching),
        Api::PutPolicy => put_policy(shared, head, &body),
        Api::DeletePolicy => delete_policy(shared, head),
        Api::SetDiagnostics => set_diagnostics(shared, &projection, &body),
        Api::SetupAll => setup_all(shared, &body),
        Api::HarnessAct => harness_act(shared, &projection, &body),
        Api::TestNotification => test_notification(shared, &body),
    };
    drop(preview_permit);
    response
}

fn read_body(route: &Route, head: &Head, incoming: &mut Incoming) -> Result<Vec<u8>, Response> {
    if !matches!(head.method, Method::Post | Method::Put | Method::Delete) {
        return Ok(Vec::new());
    }
    if head.content_length > route.body_cap {
        return Err(failure(
            Status::PayloadTooLarge,
            ErrorCode::TooLarge,
            words::TOO_LARGE,
        ));
    }
    incoming
        .read_body(head.content_length)
        .map_err(|_| bad_request())
}

/// Read a body that must be one JSON object holding exactly the arguments `T` names.
fn decode<T: DeserializeOwned>(body: &[u8]) -> Result<T, Response> {
    let value: Value = serde_json::from_slice(body).map_err(|_| bad_request())?;
    if !value.is_object() {
        return Err(bad_request());
    }
    serde_json::from_value(value).map_err(|_| bad_request())
}

/// A body with no arguments: an empty JSON object.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoArguments {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchArguments {
    query: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentArguments {
    document: String,
}

/// The views the window can be asked to bring forward.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum WindowView {
    Workbench,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WindowArguments {
    view: WindowView,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeArguments {
    intent: WorkbenchRuntimeIntent,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticsArguments {
    on: bool,
}

/// The client acts a page can ask for. A check is not one: it changes nothing.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum HarnessWebAct {
    Install,
    Fix,
    Uninstall,
}

impl From<HarnessWebAct> for HarnessAction {
    fn from(act: HarnessWebAct) -> Self {
        match act {
            HarnessWebAct::Install => Self::Install,
            HarnessWebAct::Fix => Self::Fix,
            HarnessWebAct::Uninstall => Self::Uninstall,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HarnessArguments {
    id: String,
    action: HarnessWebAct,
    expected_state: String,
}

/// The query key that names the parts a snapshot should hold.
const PARTS_KEY: &str = "parts";

/// The parts a request asks for: every part when it names none.
fn requested_parts(query: Option<&str>) -> Result<Vec<Part>, Response> {
    let Some(query) = query else {
        return Ok(Part::ALL.to_vec());
    };
    let Some(list) = url::form_urlencoded::parse(query.as_bytes())
        .find(|(key, _)| key == PARTS_KEY)
        .map(|(_, value)| value.into_owned())
    else {
        return Ok(Part::ALL.to_vec());
    };
    let mut parts = Vec::new();
    for token in list.split(',') {
        let part = Part::from_token(token).map_err(|_| bad_request())?;
        if parts.contains(&part) {
            return Err(bad_request());
        }
        parts.push(part);
    }
    Ok(parts)
}

fn snapshot(shared: &Shared, head: &Head, projection: &Projection) -> Response {
    let parts = match requested_parts(head.query.as_deref()) {
        Ok(parts) => parts,
        Err(response) => return response,
    };
    let snapshot = shared.facade.snapshot();
    let body: serde_json::Map<String, Value> = parts
        .into_iter()
        .map(|part| (part.token().to_owned(), projection.part(part, &snapshot)))
        .collect();
    json_response(
        Status::Ok,
        &json!({ "seq": snapshot.seq, "instance": shared.instance, "parts": body }),
    )
}

fn search(shared: &Shared, body: &[u8]) -> Response {
    let arguments: SearchArguments = match decode(body) {
        Ok(arguments) => arguments,
        Err(response) => return response,
    };
    if let Err(message) = validate_search_query(&arguments.query) {
        return refused(&message);
    }
    // Search reads only what a watcher is shown, so no hit needs filtering for one.
    let hits = shared.facade.search(&arguments.query);
    json_response(
        Status::Ok,
        &serde_json::to_value(hits).expect("search hits are plain data"),
    )
}

fn preview(shared: &Shared, body: &[u8]) -> Response {
    let arguments: DocumentArguments = match decode(body) {
        Ok(arguments) => arguments,
        Err(response) => return response,
    };
    if let Err(message) = validate_policy_document(&arguments.document) {
        return refused(&message);
    }
    match shared.facade.preview_user_policy(&arguments.document) {
        Ok(preview) => json_response(
            Status::Ok,
            &serde_json::to_value(preview).expect("a preview is plain data"),
        ),
        Err(message) => refused(&message),
    }
}

fn rescan(shared: &Shared, projection: &Projection, watching: bool) -> Response {
    let scan = || {
        shared
            .facade
            .refresh_harnesses()
            .map_err(|error| error.to_string())
    };
    let outcome = if watching {
        shared
            .limits
            .rescan_for_watcher(scan, || Ok(shared.facade.cached_harnesses()))
    } else {
        shared.limits.rescan_for_window(scan)
    };
    match outcome {
        Ok(summaries) => json_response(Status::Ok, &projection.harnesses(&summaries)),
        Err(message) => refused(&message),
    }
}

fn window(shared: &Shared, body: &[u8]) -> Response {
    let WindowArguments {
        view: WindowView::Workbench,
    } = match decode(body) {
        Ok(arguments) => arguments,
        Err(response) => return response,
    };
    match shared.facade.reveal() {
        Ok(()) => json_response(Status::Ok, &json!({})),
        Err(error) => refused(&error.to_string()),
    }
}

fn runtime(shared: &Shared, body: &[u8], watching: bool) -> Response {
    let arguments: RuntimeArguments = match decode(body) {
        Ok(arguments) => arguments,
        Err(response) => return response,
    };
    if watching {
        if arguments.intent != WorkbenchRuntimeIntent::Hold {
            return needs_window();
        }
        if let Err(wait) = shared.limits.hold() {
            return too_many(wait);
        }
    }
    let result = shared.facade.apply_runtime_intent(arguments.intent);
    json_response(
        Status::Ok,
        &serde_json::to_value(result).expect("an intent result is plain data"),
    )
}

/// The tag a write quotes, without the quotes a client may put around it.
fn if_match(head: &Head) -> Option<&str> {
    head.header(header::IF_MATCH).map(|value| {
        let value = value.strip_prefix("W/").unwrap_or(value);
        value.trim_matches('"')
    })
}

fn precondition_required() -> Response {
    failure(
        Status::PreconditionRequired,
        ErrorCode::Stale,
        words::STALE_POLICY,
    )
}

fn policy_write_failure(error: PolicyWriteError) -> Response {
    match error {
        PolicyWriteError::Stale => failure(
            Status::PreconditionFailed,
            ErrorCode::Stale,
            words::STALE_POLICY,
        ),
        PolicyWriteError::Authoring(error) => refused(&error.to_string()),
    }
}

fn put_policy(shared: &Shared, head: &Head, body: &[u8]) -> Response {
    let Some(tag) = if_match(head) else {
        return precondition_required();
    };
    let arguments: DocumentArguments = match decode(body) {
        Ok(arguments) => arguments,
        Err(response) => return response,
    };
    if let Err(message) = validate_policy_document(&arguments.document) {
        return refused(&message);
    }
    match shared.facade.apply_user_policy_if(&arguments.document, tag) {
        Ok(result) => json_response(
            Status::Ok,
            &serde_json::to_value(result).expect("an intent result is plain data"),
        ),
        Err(error) => policy_write_failure(error),
    }
}

fn delete_policy(shared: &Shared, head: &Head) -> Response {
    let Some(tag) = if_match(head) else {
        return precondition_required();
    };
    match shared.facade.remove_user_policy_if(tag) {
        Ok(result) => json_response(
            Status::Ok,
            &serde_json::to_value(result).expect("an intent result is plain data"),
        ),
        Err(error) => policy_write_failure(error),
    }
}

fn set_diagnostics(shared: &Shared, projection: &Projection, body: &[u8]) -> Response {
    let arguments: DiagnosticsArguments = match decode(body) {
        Ok(arguments) => arguments,
        Err(response) => return response,
    };
    let report = shared.facade.set_diagnostics(arguments.on);
    json_response(Status::Ok, &projection.diagnostics_report(&report))
}

fn setup_all(shared: &Shared, body: &[u8]) -> Response {
    if let Err(response) = decode::<NoArguments>(body) {
        return response;
    }
    match shared.facade.setup_detected_harnesses() {
        Ok(result) => json_response(
            Status::Ok,
            &serde_json::to_value(result).expect("a setup result is plain data"),
        ),
        Err(error) => refused(&error.to_string()),
    }
}

fn harness_act(shared: &Shared, projection: &Projection, body: &[u8]) -> Response {
    let arguments: HarnessArguments = match decode(body) {
        Ok(arguments) => arguments,
        Err(response) => return response,
    };
    match shared.facade.manage_harness_expecting(
        &arguments.id,
        arguments.action.into(),
        &arguments.expected_state,
    ) {
        Ok(result) => json_response(Status::Ok, &projection.harness_result(&result)),
        Err(HarnessActError::Stale) => {
            failure(Status::Conflict, ErrorCode::Stale, words::STALE_HARNESS)
        }
        Err(HarnessActError::Failed(error)) => refused(&error.to_string()),
    }
}

fn test_notification(shared: &Shared, body: &[u8]) -> Response {
    if let Err(response) = decode::<NoArguments>(body) {
        return response;
    }
    match shared.facade.test_notification() {
        Ok(()) => json_response(Status::Ok, &json!({})),
        Err(error) => refused(&error.to_string()),
    }
}
