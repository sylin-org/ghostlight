use super::*;
use crate::browser::adapter::chromium::protocol4::wire::Rect;

fn view() -> ViewFacts {
    ViewFacts {
        visual_page_x: 10.0,
        visual_page_y: 200.0,
        visual_css_width: 1280.0,
        visual_css_height: 720.0,
        content_css_width: 1280.0,
        content_css_height: 4000.0,
        device_scale: 1.0,
        zoom: 1.0,
    }
}

fn expected() -> ViewportGeometry {
    ViewportGeometry {
        scope: CaptureScope::Viewport,
        page_x: 10.0,
        page_y: 200.0,
        css_width: 1280.0,
        css_height: 720.0,
        visual_page_x: 10.0,
        visual_page_y: 200.0,
        visual_css_width: 1280.0,
        visual_css_height: 720.0,
        device_scale: 1.0,
        zoom: 1.0,
        output_scale: 1.0,
    }
}

fn captured() -> Captured {
    Captured {
        mime_type: "image/jpeg".into(),
        data: String::new(),
        width: 640,
        height: 360,
        output_scale: 0.5,
        clip: Rect {
            x: 10.0,
            y: 200.0,
            width: 1280.0,
            height: 720.0,
        },
        view: view(),
    }
}

fn owner(left: f64, top: f64) -> OwnerBox {
    OwnerBox {
        left,
        top,
        width: 300.0,
        height: 200.0,
    }
}

fn hit(x: f64, y: f64, embed: bool) -> Hit {
    Hit {
        x,
        y,
        embed,
        subject: (!embed).then(|| PhysicalActionSubject {
            role: "button".into(),
            name: "Save".into(),
        }),
    }
}

fn probe(hit: bool, left: f64, top: f64) -> Option<OwnerProbe> {
    Some(OwnerProbe { hit, left, top })
}

#[test]
fn ordinary_captures_shrink_when_needed_but_never_magnify() {
    let budget = ordinary_budget();
    assert_eq!(output_scale(400.0, 300.0, &budget), Ok(1.0));
    assert_eq!(output_scale(5000.0, 5000.0, &budget), Ok(0.4));
}

#[test]
fn region_captures_spend_the_full_bounded_output_budget_on_magnification() {
    let budget = region_budget();
    let scale = output_scale(400.0, 300.0, &budget).expect("a scale");
    assert!(scale > 1.0);
    assert!(400.0 * scale <= f64::from(MAX_SIDE));
    assert!(300.0 * scale <= f64::from(MAX_SIDE));
    assert!(400.0 * 300.0 * scale * scale <= f64::from(MAX_PIXELS) + 1.0);
}

#[test]
fn a_huge_clip_never_scales_below_the_floor() {
    let scale = output_scale(1.0e9, 1.0e9, &ordinary_budget()).expect("a scale");
    assert_eq!(scale, MIN_SCALE);
}

#[test]
fn a_tall_clip_is_bound_by_its_longest_side() {
    let scale = output_scale(100.0, 24_000.0, &ordinary_budget()).expect("a scale");
    assert!((scale - 0.1).abs() < 1.0e-12);
}

#[test]
fn an_empty_or_unmeasured_clip_has_no_scale() {
    let budget = ordinary_budget();
    assert_eq!(
        output_scale(0.0, 10.0, &budget),
        Err(GeometryError::NotPositive("width"))
    );
    assert_eq!(
        output_scale(10.0, f64::NAN, &budget),
        Err(GeometryError::NotPositive("height"))
    );
    assert_eq!(
        output_scale(-1.0, 10.0, &budget),
        Err(GeometryError::NotPositive("width"))
    );
}

#[test]
fn the_budgets_carry_todays_limits() {
    let ordinary = ordinary_budget();
    assert_eq!(ordinary.max_side, 2400);
    assert_eq!(ordinary.max_pixels, 4_000_000);
    assert_eq!(ordinary.min_scale, 0.05);
    assert_eq!(ordinary.qualities, [55, 30]);
    assert_eq!(ordinary.max_base64_chars, 6_000_000);
    assert!(!ordinary.magnify);
    let region = region_budget();
    assert!(region.magnify);
    assert_eq!(
        CaptureBudget {
            magnify: false,
            ..region
        },
        ordinary
    );
}

#[test]
fn region_geometry_rejects_negative_and_empty_rectangles() {
    let rectangle = |x, y, width, height| PhysicalRectangle {
        x,
        y,
        width,
        height,
    };
    assert_eq!(
        region_clip(&rectangle(10.0, 20.0, 400.0, 300.0)),
        Ok(CaptureClip::PageRect {
            x: 10.0,
            y: 20.0,
            width: 400.0,
            height: 300.0
        })
    );
    assert_eq!(
        region_clip(&rectangle(-1.0, 0.0, 1.0, 1.0)),
        Err(GeometryError::Negative("x"))
    );
    assert_eq!(
        region_clip(&rectangle(0.0, f64::NAN, 1.0, 1.0)),
        Err(GeometryError::Negative("y"))
    );
    assert_eq!(
        region_clip(&rectangle(0.0, 0.0, 0.0, 1.0)),
        Err(GeometryError::NotPositive("width"))
    );
    assert_eq!(
        region_clip(&rectangle(0.0, 0.0, 1.0, f64::NAN)),
        Err(GeometryError::NotPositive("height"))
    );
}

#[test]
fn a_geometry_error_is_a_plain_primitive_failure() {
    let error = BrowserError::from(GeometryError::NotPositive("width"));
    assert!(
        matches!(&error, BrowserError::Primitive(message) if message == "width must be positive")
    );
}

#[test]
fn an_unchanged_view_is_current() {
    assert!(check_view(&view(), &expected()).is_ok());
}

#[test]
fn positions_and_sizes_may_drift_one_pixel() {
    for change in [
        |v: &mut ViewFacts| v.visual_page_x += 1.0,
        |v: &mut ViewFacts| v.visual_page_y -= 1.0,
        |v: &mut ViewFacts| v.visual_css_width += 1.0,
        |v: &mut ViewFacts| v.visual_css_height -= 1.0,
    ] {
        let mut current = view();
        change(&mut current);
        assert!(check_view(&current, &expected()).is_ok());
    }
}

#[test]
fn a_drift_beyond_a_pixel_is_stale() {
    for change in [
        |v: &mut ViewFacts| v.visual_page_x += 1.5,
        |v: &mut ViewFacts| v.visual_page_y -= 1.5,
        |v: &mut ViewFacts| v.visual_css_width += 2.0,
        |v: &mut ViewFacts| v.visual_css_height -= 2.0,
    ] {
        let mut current = view();
        change(&mut current);
        assert!(
            matches!(check_view(&current, &expected()), Err(BrowserError::Primitive(message)) if message == STALE_VIEW)
        );
    }
}

#[test]
fn device_scale_and_zoom_have_their_own_tolerances() {
    let mut current = view();
    current.device_scale = 1.019;
    assert!(check_view(&current, &expected()).is_ok());
    current.device_scale = 1.03;
    assert!(check_view(&current, &expected()).is_err());

    let mut current = view();
    current.zoom = 1.001;
    assert!(check_view(&current, &expected()).is_ok());
    current.zoom = 1.002;
    assert!(check_view(&current, &expected()).is_err());
}

#[test]
fn a_view_that_is_not_a_number_is_never_current() {
    let mut current = view();
    current.visual_page_x = f64::NAN;
    assert!(check_view(&current, &expected()).is_err());
    let mut current = view();
    current.zoom = f64::INFINITY;
    assert!(check_view(&current, &expected()).is_err());
}

#[test]
fn the_viewport_records_the_clip_the_view_and_the_scale() {
    let geometry = viewport(CaptureScope::Region, &captured());
    assert_eq!(
        geometry,
        ViewportGeometry {
            scope: CaptureScope::Region,
            output_scale: 0.5,
            ..expected()
        }
    );
}

#[test]
fn an_offset_adds_every_owner_between_a_document_and_the_top() {
    assert_eq!(offset(&[]), Point { x: 0.0, y: 0.0 });
    assert_eq!(offset(&[owner(40.0, 60.0)]), Point { x: 40.0, y: 60.0 });
    assert_eq!(
        offset(&[owner(40.0, 60.0), owner(10.5, 3.0)]),
        Point { x: 50.5, y: 63.0 }
    );
}

#[test]
fn points_add_and_subtract() {
    let a = Point { x: 5.0, y: 7.0 };
    let b = Point { x: 2.0, y: 10.0 };
    assert_eq!(a + b, Point { x: 7.0, y: 17.0 });
    assert_eq!(a - b, Point { x: 3.0, y: -3.0 });
}

#[test]
fn a_point_on_no_embed_lands_on_its_subject() {
    let step = route_step(&hit(30.0, 40.0, false), [("a", probe(false, 0.0, 0.0))]);
    assert_eq!(
        step,
        RouteStep::Subject {
            subject: Some(PhysicalActionSubject {
                role: "button".into(),
                name: "Save".into()
            })
        }
    );
}

#[test]
fn a_point_under_an_embed_descends_into_the_one_owner() {
    let step = route_step(
        &hit(150.0, 90.0, true),
        [
            ("a", probe(false, 0.0, 0.0)),
            ("b", probe(true, 100.0, 50.0)),
        ],
    );
    assert_eq!(
        step,
        RouteStep::Descend {
            child: "b",
            local: Point { x: 50.0, y: 40.0 }
        }
    );
}

#[test]
fn no_owner_leaves_the_point_unresolved() {
    let step = route_step(
        &hit(150.0, 90.0, true),
        [("a", probe(false, 0.0, 0.0)), ("b", probe(false, 0.0, 0.0))],
    );
    assert_eq!(step, RouteStep::Unresolved);
}

#[test]
fn two_owners_leave_the_point_unresolved() {
    let step = route_step(
        &hit(150.0, 90.0, true),
        [
            ("a", probe(true, 100.0, 50.0)),
            ("b", probe(true, 100.0, 50.0)),
        ],
    );
    assert_eq!(step, RouteStep::Unresolved);
}

#[test]
fn an_embed_nothing_probes_leaves_the_point_unresolved() {
    let none: [(&str, Option<OwnerProbe>); 0] = [];
    assert_eq!(
        route_step(&hit(1.0, 1.0, true), none),
        RouteStep::Unresolved
    );
}

#[test]
fn a_probe_that_got_no_answer_is_not_an_owner() {
    let step = route_step(
        &hit(150.0, 90.0, true),
        [("a", None), ("b", probe(true, 100.0, 50.0))],
    );
    assert_eq!(
        step,
        RouteStep::Descend {
            child: "b",
            local: Point { x: 50.0, y: 40.0 }
        }
    );
    let only_missing = route_step(&hit(150.0, 90.0, true), [("a", None)]);
    assert_eq!(only_missing, RouteStep::Unresolved);
}
