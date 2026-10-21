use super::*;

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

/// A view handle stays current while the viewport, the device scale and the zoom stay within
/// their tolerances, and goes stale beyond them, so a point picked on a screenshot never lands on
/// a page that moved under it, while a sub-pixel jitter does not refuse a fresh view. A value
/// that is not a number is never current.
#[test]
fn a_view_is_current_only_within_its_tolerances() {
    type Change = fn(&mut ViewFacts, f64);
    let changes: [(&str, Change, f64); 6] = [
        ("page x", |v, d| v.visual_page_x += d, POSITION_TOLERANCE),
        ("page y", |v, d| v.visual_page_y -= d, POSITION_TOLERANCE),
        ("width", |v, d| v.visual_css_width += d, POSITION_TOLERANCE),
        (
            "height",
            |v, d| v.visual_css_height -= d,
            POSITION_TOLERANCE,
        ),
        (
            "device scale",
            |v, d| v.device_scale += d,
            DEVICE_SCALE_TOLERANCE,
        ),
        ("zoom", |v, d| v.zoom += d, ZOOM_TOLERANCE),
    ];
    assert!(check_view(&view(), &expected()).is_ok());
    for (name, change, tolerance) in changes {
        let mut within = view();
        change(&mut within, tolerance / 2.0);
        assert!(check_view(&within, &expected()).is_ok(), "{name} within");
        let mut beyond = view();
        change(&mut beyond, tolerance * 1.5);
        assert!(
            matches!(
                check_view(&beyond, &expected()),
                Err(BrowserError::StaleView)
            ),
            "{name} beyond"
        );
        let mut unreadable = view();
        change(&mut unreadable, f64::NAN);
        assert!(
            check_view(&unreadable, &expected()).is_err(),
            "{name} unreadable"
        );
    }
}
