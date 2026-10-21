//! Point input, drag and page export against a scripted protocol-4 extension.
//!
//! The scripted page is a top document that embeds two documents with the same URL, `C` and `E`,
//! and `C` embeds `G`. The top document is scrolled 100 pixels down, so a page point is 100
//! pixels higher in the viewport. The embedded documents sit at fixed places, and every document
//! answers what lies under a point from those places.

use super::interaction::{expected_view, inputs_seen, kinds, primitives_seen, unscoped, view};
use super::*;
use crate::browser::adapter::{PhysicalFile, RecordingDelivery, RecordingDestination};
use serde_json::json;

use super::super::super::geometry;
use super::super::super::keys;
use super::super::super::wire::{InputStep, MouseAction, Point};
use super::super::super::world::OwnerBox;
use crate::browser::adapter::PhysicalActionSubject;

/// How far the top document is scrolled, in pixels.
const SCROLL: f64 = 100.0;
/// The error text of a point that no document owns.
const UNRESOLVED: &str = "point has no current document subject";
/// The subject the top document reports for any point, as the all-open path reads it.
const TOP_ROLE: &str = "frame";

/// Where an embedded document sits in its parent's viewport.
#[derive(Clone, Copy)]
struct Region {
    left: f64,
    top: f64,
    width: f64,
    height: f64,
}

impl Region {
    fn holds(self, x: f64, y: f64) -> bool {
        x >= self.left && x < self.left + self.width && y >= self.top && y < self.top + self.height
    }
}

/// `C` in the top document.
const C_REGION: Region = Region {
    left: 100.0,
    top: 50.0,
    width: 300.0,
    height: 200.0,
};
/// `E` in the top document.
const E_REGION: Region = Region {
    left: 500.0,
    top: 50.0,
    width: 200.0,
    height: 200.0,
};
/// `G` in `C`.
const G_REGION: Region = Region {
    left: 20.0,
    top: 30.0,
    width: 100.0,
    height: 100.0,
};

/// How the owner elements answer a probe.
#[derive(Clone, Copy)]
enum Owners {
    /// The element under the point answers that it is the owner.
    Exact,
    /// No owner answers that it is under the point.
    Nobody,
    /// Every owner answers that it is under the point.
    Everybody,
}

/// The content box of an embedded document, as its owner element reports it.
fn owner_box(region: Region) -> OwnerBox {
    OwnerBox {
        left: region.left,
        top: region.top,
        width: region.width,
        height: region.height,
    }
}

fn region_of(frame: &str) -> Region {
    match frame {
        "C" => C_REGION,
        "E" => E_REGION,
        _ => G_REGION,
    }
}

/// The embedded document of `frame` that a point of its viewport lies over.
fn embed_at(frame: &str, x: f64, y: f64) -> Option<&'static str> {
    match frame {
        "T" if C_REGION.holds(x, y) => Some("C"),
        "T" if E_REGION.holds(x, y) => Some("E"),
        "C" if G_REGION.holds(x, y) => Some("G"),
        _ => None,
    }
}

fn nested_inventory() -> Value {
    json!({"frames": [frame("T", "t1", None, TOP), frame("C", "c1", Some("T"), CHILD),
                      frame("G", "g1", Some("C"), SECOND), frame("E", "e1", Some("T"), CHILD)],
           "complete": true, "unseen": []})
}

fn nested_documents() -> Vec<PhysicalDocument> {
    vec![
        physical("T.t1", None, TOP),
        physical("C.c1", Some("T.t1"), CHILD),
        physical("G.g1", Some("C.c1"), SECOND),
        physical("E.e1", Some("T.t1"), CHILD),
    ]
}

fn nested_scope(allowed: &[&str]) -> DocumentScope {
    DocumentScope {
        documents: nested_documents(),
        allowed: allowed.iter().map(|id| (*id).into()).collect(),
        subjects: vec![],
        watch_changes: false,
        strict_tree: false,
        settle_ms: 0,
        settle_requested: true,
    }
}

/// Every document allowed: no point needs placing.
fn all_open() -> DocumentScope {
    nested_scope(&["T.t1", "C.c1", "G.g1", "E.e1"])
}

/// A scope that leaves `E` unreadable, so every point is placed before input.
fn without_e() -> DocumentScope {
    nested_scope(&["T.t1", "C.c1", "G.g1"])
}

/// A scope that leaves `C` and `G` unreadable.
fn without_c() -> DocumentScope {
    nested_scope(&["T.t1", "E.e1"])
}

fn subject_in(frame: &str) -> Value {
    json!({"role": "link", "name": format!("In {frame}")})
}

/// The answer of the scripted page to one world call.
fn answer(world: &wire::WorldCall, owners: Owners) -> Value {
    let message = &world.message;
    let frame = world
        .frame
        .as_ref()
        .map_or("T", |frame| frame.frame_id.as_str());
    let x = message["x"].as_f64().unwrap_or_default();
    let y = message["y"].as_f64().unwrap_or_default();
    match message["kind"].as_str().unwrap_or_default() {
        "scroll_point" => {
            returned(json!({"x": x, "y": y - SCROLL, "subject": {"role": TOP_ROLE, "name": "Top"}}))
        }
        "viewport_point" if y - SCROLL < 0.0 => refused("outside_viewport"),
        "viewport_point" => {
            returned(json!({"x": x, "y": y - SCROLL, "subject": {"role": TOP_ROLE, "name": "Top"}}))
        }
        "hit_point" => match embed_at(frame, x, y) {
            Some(_) => returned(json!({"x": x, "y": y, "embed": true, "subject": null})),
            None => returned(json!({"x": x, "y": y, "embed": false, "subject": subject_in(frame)})),
        },
        "owner_probe" => {
            let child = world.owner_of.as_deref().unwrap_or_default();
            let region = region_of(child);
            let hit = match owners {
                Owners::Exact => embed_at(frame, x, y) == Some(child),
                Owners::Nobody => false,
                Owners::Everybody => true,
            };
            returned(json!({"hit": hit, "left": region.left, "top": region.top}))
        }
        "owner_box" => {
            let region = region_of(world.owner_of.as_deref().unwrap_or_default());
            returned(json!({"left": region.left, "top": region.top,
                "width": region.width, "height": region.height}))
        }
        "activate" | "hover" | "box" => {
            let rectangle = if message["locator"] == "locator_a1_9" {
                json!({"left": 130, "top": 80, "width": 40, "height": 40})
            } else {
                json!({"left": 10, "top": 10, "width": 20, "height": 10})
            };
            returned(json!({"rectangle": rectangle, "subject": subject_in(frame)}))
        }
        "drop_files" => returned(json!({"uploaded_count": 1, "uploaded_bytes": 9})),
        _ => refused("unknown_kind"),
    }
}

/// The facts a recording operation of the scripted extension reports.
fn recording_facts(op: &wire::RecordingOp) -> Value {
    let summary = json!({"recording_id": "recording_1", "tab_id": 12, "state": "frozen",
        "frame_count": 3, "bytes_held": 100, "duration_ms": 900, "source_urls": [TOP]});
    match op {
        wire::RecordingOp::Export { .. } => json!({"result": "exported", "summary": summary,
            "encoded": {"frame_count": 3, "captured_frame_count": 3, "duration_ms": 900,
                "width": 640, "height": 400, "byte_count": 2048},
            "delivery": {"delivery": "attached", "tab_id": 12}}),
        _ => json!({"result": "started", "existing": false, "summary": summary}),
    }
}

/// A port whose extension holds the scripted page, reports `current_view`, and finishes every
/// input.
fn rig(owners: Owners, current_view: Value) -> (RelayBrowserPort, FakeExtension) {
    port_with(Box::new(move |correlation, primitive| {
        Reply::Now(match primitive {
            wire::Primitive::Frames(_) => FakeExtension::ok(correlation, nested_inventory()),
            wire::Primitive::ListTabs(_) => FakeExtension::ok(
                correlation,
                json!({"tabs": [{"tab_id": 12, "title": "Top", "url": TOP, "active": true,
                    "readiness": "complete"}]}),
            ),
            wire::Primitive::View(_) => FakeExtension::ok(correlation, current_view.clone()),
            wire::Primitive::Call(call) => FakeExtension::ok(
                correlation,
                json!({"answers": call.calls.iter()
                    .map(|world| answer(world, owners)).collect::<Vec<_>>()}),
            ),
            wire::Primitive::Input(input) => wire::Receipt {
                dispatched: true,
                ..FakeExtension::ok(
                    correlation,
                    json!({"completed_steps": input.steps.len(), "precondition": null}),
                )
            },
            wire::Primitive::Recording(op) => FakeExtension::ok(correlation, recording_facts(op)),
            _ => FakeExtension::ok(correlation, Value::Null),
        })
    }))
}

fn exact() -> (RelayBrowserPort, FakeExtension) {
    rig(Owners::Exact, view())
}

fn at(x: f64, y: f64) -> PhysicalPoint {
    PhysicalPoint { x, y }
}

/// A page point over `G`, inside `C`: viewport (150, 100), local to `C` (50, 50), local to `G`
/// (30, 20).
const OVER_G: (f64, f64) = (150.0, 200.0);
/// A page point over `E`: viewport (550, 100), local to `E` (50, 50).
const OVER_E: (f64, f64) = (550.0, 200.0);

fn click_at(point: (f64, f64)) -> BrowserCommand {
    BrowserCommand::ActivatePoint {
        tab_id: 12,
        point: at(point.0, point.1),
        expected_viewport: expected_view(),
        button: "left".into(),
        click_count: 1,
        modifiers: vec![],
    }
}

fn hover_at(point: (f64, f64)) -> BrowserCommand {
    BrowserCommand::HoverPoint {
        tab_id: 12,
        point: at(point.0, point.1),
        expected_viewport: expected_view(),
    }
}

fn wheel_at(point: (f64, f64), direction: &str, ticks: u8) -> BrowserCommand {
    BrowserCommand::WheelAt {
        tab_id: 12,
        point: at(point.0, point.1),
        expected_viewport: expected_view(),
        direction: direction.into(),
        ticks,
    }
}

fn drag_points(start: (f64, f64), end: (f64, f64)) -> BrowserCommand {
    BrowserCommand::DragPoints {
        tab_id: 12,
        start: at(start.0, start.1),
        end: at(end.0, end.1),
        expected_viewport: expected_view(),
    }
}

fn picture() -> PhysicalFile {
    PhysicalFile {
        name: "photo.png".into(),
        media_type: "image/png".into(),
        data: "AAAA".into(),
        size: 3,
    }
}

fn drop_at(point: (f64, f64)) -> BrowserCommand {
    BrowserCommand::DropImageAt {
        tab_id: 12,
        point: at(point.0, point.1),
        expected_viewport: expected_view(),
        file: picture(),
    }
}

fn subject_name(outcome: &BrowserOutcome) -> Option<String> {
    match outcome {
        BrowserOutcome::Activated { subject, .. } | BrowserOutcome::Hovered { subject, .. } => {
            subject.as_ref().map(|subject| subject.name.clone())
        }
        other => panic!("no subject in {other:?}"),
    }
}

fn frame_ids(guard: &wire::DocumentGuard) -> Vec<&str> {
    guard
        .documents
        .iter()
        .map(|document| document.frame_id.as_str())
        .collect()
}

#[test]
fn an_open_scope_sends_a_point_click_without_routing_it() {
    let (port, fake) = exact();
    let result = unscoped(call(&port, scoped(all_open(), click_at(OVER_G))).unwrap());
    assert_eq!(
        subject_name(&result).as_deref(),
        Some("Top"),
        "the top document's own subject stands when nothing is routed"
    );
    assert_eq!(
        primitives_seen(&fake),
        ["view", "call", "input", "list_tabs"],
        "a scope names its tree, and nothing asks which document the point is in"
    );
    let calls = calls_seen(&fake);
    assert_eq!(kinds(&calls[0]), ["scroll_point"]);
    assert_eq!(frames_of(&calls[0]), ["T"]);
    assert_eq!(calls[0].calls[0].message["x"], 150);
    assert_eq!(calls[0].calls[0].message["y"], 200);
    assert!(calls[0].guard.is_some());
    let inputs = inputs_seen(&fake);
    assert_eq!(
        inputs[0].steps,
        keys::click(
            Point { x: 150.0, y: 100.0 },
            keys::mouse_button("left"),
            1,
            0
        ),
        "input goes to the position the top viewport reports"
    );
    assert_eq!(inputs[0].settle_ms, 250);
    assert!(inputs[0].guard.is_some());
}

#[test]
fn a_modified_point_click_holds_its_modifiers() {
    let (port, fake) = exact();
    call(
        &port,
        scoped(
            all_open(),
            BrowserCommand::ActivatePoint {
                tab_id: 12,
                point: at(OVER_E.0, OVER_E.1),
                expected_viewport: expected_view(),
                button: "left".into(),
                click_count: 2,
                modifiers: vec!["Shift".into()],
            },
        ),
    )
    .unwrap();
    assert_eq!(
        inputs_seen(&fake)[0].steps,
        keys::click(
            Point { x: 550.0, y: 100.0 },
            keys::mouse_button("left"),
            2,
            keys::modifier_mask(&["Shift"])
        )
    );
}

#[test]
fn a_stale_view_refuses_the_point_before_anything_is_sent() {
    let moved = json!({"visual_page_x": 0, "visual_page_y": 0, "visual_css_width": 800,
        "visual_css_height": 600, "content_css_width": 800, "content_css_height": 2000,
        "device_scale": 1, "zoom": 1.5});
    for command in [
        click_at(OVER_G),
        hover_at(OVER_G),
        wheel_at(OVER_G, "down", 1),
        drag_points(OVER_G, OVER_E),
        drop_at(OVER_G),
    ] {
        let (port, fake) = rig(Owners::Exact, moved.clone());
        assert_eq!(
            call(&port, scoped(all_open(), command)),
            Err(BrowserError::StaleView)
        );
        assert_eq!(primitives_seen(&fake), ["view"]);
    }
}

#[test]
fn a_scoped_point_is_routed_through_nested_documents_to_its_subject() {
    let (port, fake) = exact();
    let (observation, result) =
        unwrap_scoped(call(&port, scoped(without_e(), click_at(OVER_G))).unwrap());
    assert_eq!(subject_name(&result).as_deref(), Some("In G"));
    assert!(
        observation.unavailable.is_empty(),
        "routing reads no content, so it visits nothing"
    );
    let calls = calls_seen(&fake);
    assert_eq!(calls.len(), 4, "the scroll, then one round trip per level");
    assert_eq!(calls[1].mode, wire::CallMode::Parallel);
    assert_eq!(
        kinds(&calls[1]),
        ["hit_point", "owner_probe", "owner_probe"]
    );
    assert_eq!(frames_of(&calls[1]), ["T", "T", "T"]);
    assert_eq!(
        calls[1]
            .calls
            .iter()
            .map(|world| world.owner_of.as_deref())
            .collect::<Vec<_>>(),
        [None, Some("C"), Some("E")]
    );
    assert_eq!(calls[1].calls[0].message["page"], false);
    assert_eq!(kinds(&calls[2]), ["hit_point", "owner_probe"]);
    assert_eq!(frames_of(&calls[2]), ["C", "C"]);
    assert_eq!(calls[2].calls[0].message["x"], 50);
    assert_eq!(calls[2].calls[0].message["y"], 50);
    assert_eq!(kinds(&calls[3]), ["hit_point"]);
    assert_eq!(frames_of(&calls[3]), ["G"]);
    assert_eq!(calls[3].calls[0].message["x"], 30);
    assert_eq!(calls[3].calls[0].message["y"], 20);
    let input = &inputs_seen(&fake)[0];
    assert_eq!(
        input.steps,
        keys::click(
            Point { x: 150.0, y: 100.0 },
            keys::mouse_button("left"),
            1,
            0
        ),
        "routing changes who is asked, not where the pointer goes"
    );
    assert!(
        frame_ids(input.guard.as_ref().unwrap()).contains(&"G"),
        "the guard that rides on the input pins the document the point landed in"
    );
}

#[test]
fn identical_embeds_are_told_apart_by_their_owners() {
    let (port, fake) = exact();
    let result = unscoped(call(&port, scoped(without_c(), click_at(OVER_E))).unwrap());
    assert_eq!(subject_name(&result).as_deref(), Some("In E"));
    let calls = calls_seen(&fake);
    assert_eq!(
        calls[1]
            .calls
            .iter()
            .map(|world| world.owner_of.as_deref())
            .collect::<Vec<_>>(),
        [None, Some("C"), Some("E")],
        "both embeds share a URL, so only the element under the point can say which is which"
    );
    assert_eq!(calls.len(), 3, "the scroll, the top, then E");
    assert_eq!(frames_of(&calls[2]), ["E"]);
}

#[test]
fn a_point_over_a_document_the_scope_leaves_out_is_refused_before_input() {
    for command in [
        click_at(OVER_G),
        hover_at(OVER_G),
        wheel_at(OVER_G, "down", 1),
    ] {
        let (port, fake) = exact();
        assert_eq!(
            call(&port, scoped(without_c(), command)),
            Err(BrowserError::DocumentUnavailable)
        );
        assert!(inputs_seen(&fake).is_empty());
    }
    let (port, fake) = exact();
    assert_eq!(
        call(&port, scoped(without_c(), drop_at(OVER_G))),
        Err(BrowserError::DocumentUnavailable)
    );
    assert!(
        calls_seen(&fake)
            .iter()
            .all(|call| !kinds(call).contains(&"drop_files".to_owned())),
        "the drop is never delivered"
    );
}

#[test]
fn a_point_that_zero_or_several_owners_claim_is_unresolved_and_sends_nothing() {
    for owners in [Owners::Nobody, Owners::Everybody] {
        let (port, fake) = rig(owners, view());
        assert_eq!(
            call(&port, scoped(without_c(), click_at(OVER_E))),
            Err(BrowserError::Primitive(UNRESOLVED.into())),
            "an embed no single owner claims is never guessed"
        );
        assert!(inputs_seen(&fake).is_empty());
    }
}

#[test]
fn a_restricted_click_on_a_target_lands_when_the_point_is_where_the_target_is() {
    let (port, fake) = exact();
    let result = unscoped(
        call(
            &port,
            scoped(
                without_c(),
                BrowserCommand::Activate {
                    tab_id: 12,
                    locator: "T:t1:locator_a1_2".into(),
                    button: "left".into(),
                    click_count: 1,
                    modifiers: vec![],
                },
            ),
        )
        .unwrap(),
    );
    assert_eq!(subject_name(&result).as_deref(), Some("In T"));
    assert_eq!(inputs_seen(&fake).len(), 1);
    let calls = calls_seen(&fake);
    assert_eq!(
        kinds(&calls[1]),
        ["hit_point", "owner_probe", "owner_probe"]
    );
}

#[test]
fn a_restricted_click_covered_by_a_denied_document_is_refused_before_input() {
    let covered = |scope| {
        let (port, fake) = exact();
        let result = call(
            &port,
            scoped(
                scope,
                BrowserCommand::Activate {
                    tab_id: 12,
                    locator: "T:t1:locator_a1_9".into(),
                    button: "left".into(),
                    click_count: 1,
                    modifiers: vec![],
                },
            ),
        );
        (result, inputs_seen(&fake).len())
    };
    assert_eq!(
        covered(without_c()),
        (Err(BrowserError::DocumentUnavailable), 0),
        "the target is in the top document, but a document that is not allowed lies over it"
    );
    let (result, inputs) = covered(without_e());
    assert!(result.is_ok());
    assert_eq!(inputs, 1);
}

#[test]
fn a_hover_at_a_point_moves_the_pointer_there() {
    let (port, fake) = exact();
    let result = unscoped(call(&port, scoped(without_e(), hover_at(OVER_G))).unwrap());
    assert!(matches!(
        result,
        BrowserOutcome::Hovered { tab_id: 12, ref subject } if subject.as_ref().is_some_and(|subject| subject.name == "In G")
    ));
    let inputs = inputs_seen(&fake);
    assert_eq!(inputs[0].steps, keys::hover(Point { x: 150.0, y: 100.0 }));
    assert_eq!(inputs[0].settle_ms, 0);
}

#[test]
fn a_wheel_turns_once_a_tick_in_the_direction_asked() {
    for (direction, ticks, delta) in [("down", 3_u8, 120.0), ("up", 2, -120.0)] {
        let (port, fake) = exact();
        let result = unscoped(
            call(
                &port,
                scoped(all_open(), wheel_at(OVER_E, direction, ticks)),
            )
            .unwrap(),
        );
        assert_eq!(
            result,
            BrowserOutcome::Scrolled {
                tab_id: 12,
                x: 550.0,
                y: 100.0,
                subject: Some(PhysicalActionSubject {
                    role: TOP_ROLE.into(),
                    name: "Top".into(),
                    rebound: false,
                })
            },
            "the receipt reports where the wheel turned, in the top viewport"
        );
        let input = &inputs_seen(&fake)[0];
        assert_eq!(input.steps.len(), usize::from(ticks));
        for step in &input.steps {
            let InputStep::Mouse {
                action,
                x,
                y,
                delta_x,
                delta_y,
                ..
            } = step
            else {
                panic!("a mouse step, not {step:?}");
            };
            assert_eq!(*action, MouseAction::Wheel);
            assert_eq!((*x, *y), (550.0, 100.0));
            assert_eq!((*delta_x, *delta_y), (0.0, delta));
        }
        assert_eq!(input.settle_ms, 120);
    }
}

#[test]
fn a_drag_between_points_moves_in_twelve_strides() {
    let (port, fake) = exact();
    let result = unscoped(call(&port, scoped(all_open(), drag_points(OVER_G, OVER_E))).unwrap());
    let BrowserOutcome::Dragged {
        tab,
        source_subject,
        destination_subject,
        ..
    } = result
    else {
        panic!("a drag");
    };
    assert_eq!(tab.tab_id, 12);
    assert_eq!(source_subject.unwrap().name, "Top");
    assert_eq!(destination_subject.unwrap().name, "Top");
    let calls = calls_seen(&fake);
    assert_eq!(calls.len(), 1, "an open scope routes nothing");
    assert_eq!(calls[0].mode, wire::CallMode::Sequence);
    assert_eq!(kinds(&calls[0]), ["viewport_point", "viewport_point"]);
    assert!(calls[0].guard.is_some());
    let inputs = inputs_seen(&fake);
    assert_eq!(inputs[0].steps.len(), 1);
    let InputStep::Drag { path, modifiers } = &inputs[0].steps[0] else {
        panic!("a drag step");
    };
    assert_eq!(*modifiers, 0);
    assert_eq!(path.len(), 13, "the press point, then twelve moves");
    assert_eq!(*path.first().unwrap(), Point { x: 150.0, y: 100.0 });
    assert_eq!(*path.last().unwrap(), Point { x: 550.0, y: 100.0 });
    let strides: Vec<f64> = path.windows(2).map(|pair| pair[1].x - pair[0].x).collect();
    assert!(
        strides
            .iter()
            .all(|stride| (stride - 400.0 / 12.0).abs() < 1e-6),
        "equal strides, not {strides:?}"
    );
}

#[test]
fn a_restricted_drag_between_points_names_the_subjects_at_both_ends() {
    let (port, fake) = exact();
    let scope = nested_scope(&["T.t1", "G.g1", "E.e1"]);
    let (_, result) =
        unwrap_scoped(call(&port, scoped(scope, drag_points(OVER_G, OVER_E))).unwrap());
    let BrowserOutcome::Dragged {
        source_subject,
        destination_subject,
        ..
    } = result
    else {
        panic!("a drag");
    };
    assert_eq!(source_subject.unwrap().name, "In G");
    assert_eq!(destination_subject.unwrap().name, "In E");
    let guard = inputs_seen(&fake)[0].guard.clone().unwrap();
    assert!(frame_ids(&guard).contains(&"G") && frame_ids(&guard).contains(&"E"));
}

#[test]
fn a_drag_point_outside_the_viewport_is_a_plain_failure() {
    let (port, fake) = exact();
    let error = call(
        &port,
        scoped(all_open(), drag_points((150.0, 50.0), OVER_E)),
    )
    .unwrap_err();
    assert_eq!(error, BrowserError::Primitive("outside_viewport".into()));
    assert!(inputs_seen(&fake).is_empty());
}

#[test]
fn offsets_through_two_nested_frames_place_a_dragged_target() {
    let (port, fake) = exact();
    let result = unscoped(
        call(
            &port,
            scoped(
                all_open(),
                BrowserCommand::Drag {
                    tab_id: 12,
                    source_locator: "G:g1:locator_a1_1".into(),
                    destination_locator: "T:t1:locator_a1_2".into(),
                },
            ),
        )
        .unwrap(),
    );
    let BrowserOutcome::Dragged {
        source_subject,
        destination_subject,
        ..
    } = result
    else {
        panic!("a drag");
    };
    assert_eq!(source_subject.unwrap().name, "In G");
    assert_eq!(destination_subject.unwrap().name, "In T");
    let calls = calls_seen(&fake);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].mode, wire::CallMode::Sequence);
    assert_eq!(
        kinds(&calls[0]),
        ["hover", "hover", "box", "box", "owner_box", "owner_box"],
        "the destination is scrolled to first, the source second, then both are measured"
    );
    assert_eq!(frames_of(&calls[0]), ["T", "G", "G", "T", "C", "T"]);
    assert_eq!(
        calls[0]
            .calls
            .iter()
            .map(|world| world.owner_of.as_deref())
            .collect::<Vec<_>>(),
        [None, None, None, None, Some("G"), Some("C")]
    );
    let start =
        Point { x: 20.0, y: 15.0 } + geometry::offset(&[owner_box(G_REGION), owner_box(C_REGION)]);
    assert_eq!(start, Point { x: 140.0, y: 95.0 });
    let inputs = inputs_seen(&fake);
    assert_eq!(
        inputs[0].steps,
        [InputStep::Drag {
            path: keys::drag_path(start, Point { x: 20.0, y: 15.0 }, 12),
            modifiers: 0
        }],
        "the source centre is shifted by the content boxes of both frames above it"
    );
    assert_eq!(inputs[0].settle_ms, 0);
}

/// A port whose extension holds the scripted page, and whose hover of each locator in `followed`
/// reports that the call followed that handle through a re-render.
fn following_hovers(followed: &'static [&'static str]) -> (RelayBrowserPort, FakeExtension) {
    port_with(Box::new(move |correlation, primitive| {
        Reply::Now(match primitive {
            wire::Primitive::Frames(_) => FakeExtension::ok(correlation, nested_inventory()),
            wire::Primitive::ListTabs(_) => FakeExtension::ok(
                correlation,
                json!({"tabs": [{"tab_id": 12, "title": "Top", "url": TOP, "active": true,
                    "readiness": "complete"}]}),
            ),
            wire::Primitive::View(_) => FakeExtension::ok(correlation, view()),
            wire::Primitive::Call(call) => FakeExtension::ok(
                correlation,
                json!({"answers": call.calls.iter().map(|world| {
                    let answered = answer(world, Owners::Exact);
                    let locator = world.message["locator"].as_str().unwrap_or_default();
                    if world.message["kind"] == "hover" && followed.contains(&locator) {
                        let mut envelope = answered["envelope"].clone();
                        envelope["rebound"] = json!([locator]);
                        json!({"status": "returned", "envelope": envelope})
                    } else {
                        answered
                    }
                }).collect::<Vec<_>>()}),
            ),
            wire::Primitive::Input(input) => wire::Receipt {
                dispatched: true,
                ..FakeExtension::ok(
                    correlation,
                    json!({"completed_steps": input.steps.len(), "precondition": null}),
                )
            },
            _ => FakeExtension::ok(correlation, Value::Null),
        })
    }))
}

/// The measurements that follow the preparing hovers find both ends live and say nothing of a
/// follow, so a drag takes the fact from the hover that brought each end into view.
#[test]
fn a_drag_whose_preparing_hover_followed_a_handle_reports_it_at_that_end() {
    let cases: [(&'static [&'static str], bool, bool); 4] = [
        (&[], false, false),
        (&["locator_a1_1"], true, false),
        (&["locator_a1_2"], false, true),
        (&["locator_a1_1", "locator_a1_2"], true, true),
    ];
    for (followed, from, onto) in cases {
        let (port, _fake) = following_hovers(followed);
        let result = unscoped(
            call(
                &port,
                scoped(
                    all_open(),
                    BrowserCommand::Drag {
                        tab_id: 12,
                        source_locator: "G:g1:locator_a1_1".into(),
                        destination_locator: "T:t1:locator_a1_2".into(),
                    },
                ),
            )
            .unwrap(),
        );
        let BrowserOutcome::Dragged {
            source_subject,
            destination_subject,
            ..
        } = result
        else {
            panic!("a drag");
        };
        assert_eq!(source_subject.unwrap().rebound, from, "{followed:?}");
        assert_eq!(destination_subject.unwrap().rebound, onto, "{followed:?}");
    }
}

#[test]
fn a_dialog_while_the_pointer_is_down_leaves_a_drag_half_done() {
    let (port, _fake) = port_with(Box::new(|correlation, primitive| {
        Reply::Now(match primitive {
            wire::Primitive::View(_) => FakeExtension::ok(correlation, view()),
            wire::Primitive::Call(call) => FakeExtension::ok(
                correlation,
                json!({"answers": call.calls.iter()
                    .map(|world| answer(world, Owners::Exact)).collect::<Vec<_>>()}),
            ),
            wire::Primitive::Input(_) => wire::Receipt {
                dispatched: true,
                ..FakeExtension::ok(
                    correlation,
                    json!({"completed_steps": 0,
                        "stopped": {"reason": "dialog", "dialog_type": "confirm"}}),
                )
            },
            _ => FakeExtension::ok(correlation, Value::Null),
        })
    }));
    assert!(matches!(
        call(&port, scoped(all_open(), drag_points(OVER_G, OVER_E))),
        Err(BrowserError::EffectUnknown(_))
    ));
}

#[test]
fn a_dropped_image_is_delivered_inside_the_document_under_the_point() {
    for scope in [all_open(), without_e()] {
        let (port, fake) = exact();
        let result = unscoped(call(&port, scoped(scope, drop_at(OVER_G))).unwrap());
        assert_eq!(
            result,
            BrowserOutcome::FilesUploaded {
                tab_id: 12,
                uploaded_count: 1,
                uploaded_bytes: 9,
                subject: Some(PhysicalActionSubject {
                    role: "link".into(),
                    name: "In G".into(),
                    rebound: false,
                })
            }
        );
        let calls = calls_seen(&fake);
        let drop = calls.last().unwrap();
        assert_eq!(kinds(drop), ["drop_files"]);
        assert_eq!(
            frames_of(drop),
            ["G"],
            "the deepest document, even for an open scope"
        );
        let message = &drop.calls[0].message;
        assert_eq!(
            (message["x"].clone(), message["y"].clone()),
            (json!(30), json!(20))
        );
        assert_eq!(message["files"][0]["name"], "photo.png");
        assert!(frame_ids(drop.guard.as_ref().unwrap()).contains(&"G"));
        assert!(inputs_seen(&fake).is_empty());
    }
}

fn export_to(locator: &str) -> BrowserCommand {
    BrowserCommand::ExportRecording {
        destination: RecordingDestination::Target {
            tab_id: 12,
            locator: locator.into(),
            file_name: "flow.gif".into(),
        },
        max_output_bytes: 4096,
    }
}

fn exports_seen(fake: &FakeExtension) -> Vec<wire::RecordingOp> {
    fake.wait(|_| true)
        .requests
        .iter()
        .filter_map(|(_, primitive)| match primitive {
            wire::Primitive::Recording(op @ wire::RecordingOp::Export { .. }) => Some(op.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_recording_exported_into_a_page_carries_its_attach_call_and_guard() {
    let (port, fake) = exact();
    call(
        &port,
        scoped(all_open(), BrowserCommand::StartRecording { tab_id: 12 }),
    )
    .unwrap();
    let (_, result) =
        unwrap_scoped(call(&port, scoped(all_open(), export_to("C:c1:locator_a1_4"))).unwrap());
    assert!(matches!(
        result,
        BrowserOutcome::RecordingExported {
            delivery: RecordingDelivery::Attached { tab_id: 12 },
            ..
        }
    ));
    let exports = exports_seen(&fake);
    assert_eq!(exports.len(), 1);
    let wire::RecordingOp::Export {
        recording_id,
        destination:
            wire::RecordingTarget::Page {
                tab_id,
                call,
                guard,
            },
        max_output_bytes,
    } = &exports[0]
    else {
        panic!("an export into a page");
    };
    assert_eq!(recording_id, "recording_1");
    assert_eq!(*tab_id, 12);
    assert_eq!(*max_output_bytes, 4096);
    assert_eq!(call.frame.as_ref().unwrap().frame_id, "C");
    assert_eq!(
        call.message,
        json!({"kind": "attach_file", "locator": "locator_a1_4", "file_name": "flow.gif"})
    );
    assert!(
        guard.is_some(),
        "the extension checks the guard right before the call"
    );
}

#[test]
fn a_document_change_during_a_page_export_is_a_navigated_target() {
    // The file input's document reloaded between the description and the export, so the
    // export names a target that navigated, not a document whose access is in doubt.
    let (port, _fake) = port_with(Box::new(|correlation, primitive| {
        Reply::Now(match primitive {
            wire::Primitive::Frames(_) => FakeExtension::ok(correlation, nested_inventory()),
            wire::Primitive::Recording(wire::RecordingOp::Export { .. }) => {
                FakeExtension::failed(correlation, wire::Failure::DocumentChanged)
            }
            wire::Primitive::Recording(op) => FakeExtension::ok(correlation, recording_facts(op)),
            _ => FakeExtension::ok(correlation, Value::Null),
        })
    }));
    call(
        &port,
        scoped(all_open(), BrowserCommand::StartRecording { tab_id: 12 }),
    )
    .unwrap();
    assert_eq!(
        call(&port, scoped(all_open(), export_to("C:c1:locator_a1_4"))),
        Err(BrowserError::stale(StaleReason::Navigated))
    );
}

#[test]
fn a_recording_is_never_exported_into_a_document_the_scope_leaves_out() {
    let (port, fake) = exact();
    call(
        &port,
        scoped(all_open(), BrowserCommand::StartRecording { tab_id: 12 }),
    )
    .unwrap();
    assert_eq!(
        call(&port, scoped(without_c(), export_to("C:c1:locator_a1_4"))),
        Err(BrowserError::DocumentUnavailable)
    );
    assert!(exports_seen(&fake).is_empty());
}
