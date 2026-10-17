//! Physical-pixel geometry and placement policy, independent of the native runtime.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub origin: Point,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct FrameInsets {
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
}

pub fn fit_visible(
    preferred: Point,
    width: u32,
    height: u32,
    work: Rect,
    margin: u32,
    frame: FrameInsets,
) -> Rect {
    let axis = |position: i32, length: u32, before: u32, after: u32, start: i32, available: u32| {
        let visible_length = i64::from(length.saturating_sub(before.saturating_add(after)).max(1));
        let visible_start = i64::from(position) + i64::from(before);
        let start = i64::from(start);
        let available = i64::from(available);
        let end = start + available;
        let corrected = if visible_start >= start && visible_start + visible_length <= end {
            visible_start
        } else if visible_length > available {
            // The original size is retained even when the display cannot hold
            // it. Keep the caption's leading edge reachable; never claim fit.
            start + i64::from(margin).min(available.saturating_sub(1))
        } else {
            let margin = i64::from(margin).min(available - visible_length);
            if visible_start < start {
                start + margin
            } else {
                end - margin - visible_length
            }
        };
        (corrected - i64::from(before)).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
    };
    Rect {
        origin: Point {
            x: axis(
                preferred.x,
                width,
                frame.left,
                frame.right,
                work.origin.x,
                work.width,
            ),
            y: axis(
                preferred.y,
                height,
                frame.top,
                frame.bottom,
                work.origin.y,
                work.height,
            ),
        },
        width,
        height,
    }
}

pub fn centred_visible(
    width: u32,
    height: u32,
    work: Rect,
    margin: u32,
    frame: FrameInsets,
) -> Rect {
    let axis = |length: u32, before: u32, after: u32, start: i32, available: u32| {
        let visible = length.saturating_sub(before.saturating_add(after)).max(1);
        let offset = if visible <= available {
            (available - visible) / 2
        } else {
            margin.min(available.saturating_sub(1))
        };
        (i64::from(start) + i64::from(offset) - i64::from(before))
            .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
    };
    Rect {
        origin: Point {
            x: axis(width, frame.left, frame.right, work.origin.x, work.width),
            y: axis(height, frame.top, frame.bottom, work.origin.y, work.height),
        },
        width,
        height,
    }
}

pub fn fit(preferred: Point, width: u32, height: u32, work: Rect) -> Rect {
    fit_visible(preferred, width, height, work, 0, FrameInsets::default())
}

pub fn centred(width: u32, height: u32, work: Rect) -> Rect {
    centred_visible(width, height, work, 0, FrameInsets::default())
}

pub fn closest_monitor(point: Point, monitors: &[Rect]) -> Option<usize> {
    monitors
        .iter()
        .enumerate()
        .min_by_key(|(_, rect)| {
            let distance = |value: i32, origin: i32, length: u32| {
                let value = i64::from(value);
                let nearest = value.clamp(
                    i64::from(origin),
                    i64::from(origin) + i64::from(length.saturating_sub(1)),
                );
                i128::from(value - nearest).pow(2)
            };
            distance(point.x, rect.origin.x, rect.width)
                + distance(point.y, rect.origin.y, rect.height)
        })
        .map(|(index, _)| index)
}

pub fn valid_logical_size(width: f64, height: f64) -> bool {
    [width, height]
        .iter()
        .all(|value| value.is_finite() && *value > 0.0 && *value <= 100_000.0)
}

pub fn monitor_for_rect(rect: Rect, monitors: &[Rect]) -> Option<usize> {
    let overlap = |left: i32, length: u32, other_left: i32, other_length: u32| {
        let start = i64::from(left).max(i64::from(other_left));
        let end = (i64::from(left) + i64::from(length))
            .min(i64::from(other_left) + i64::from(other_length));
        (end - start).max(0) as u64
    };
    let best = monitors
        .iter()
        .enumerate()
        .map(|(index, monitor)| {
            (
                index,
                overlap(rect.origin.x, rect.width, monitor.origin.x, monitor.width)
                    * overlap(rect.origin.y, rect.height, monitor.origin.y, monitor.height),
            )
        })
        .max_by_key(|(_, area)| *area);
    match best {
        Some((index, area)) if area > 0 => Some(index),
        _ => closest_monitor(midpoint(rect), monitors),
    }
}

pub fn midpoint(rect: Rect) -> Point {
    let axis = |origin: i32, length: u32| {
        (i64::from(origin) + i64::from(length / 2)).clamp(i64::from(i32::MIN), i64::from(i32::MAX))
            as i32
    };
    Point {
        x: axis(rect.origin.x, rect.width),
        y: axis(rect.origin.y, rect.height),
    }
}

pub fn monitor_for_preference(
    name: Option<&str>,
    anchor: Point,
    monitors: &[(Option<&str>, Rect)],
) -> Option<usize> {
    if let Some(name) = name {
        if let Some(index) = monitors
            .iter()
            .position(|(candidate, _)| *candidate == Some(name))
        {
            return Some(index);
        }
    }
    closest_monitor(
        anchor,
        &monitors.iter().map(|(_, rect)| *rect).collect::<Vec<_>>(),
    )
}

/// Coalesces display notifications received synchronously inside a native setter.
#[derive(Default)]
pub(crate) struct ApplyState {
    applying: bool,
    deferred: std::collections::BTreeSet<String>,
}

impl ApplyState {
    pub fn begin(&mut self) -> bool {
        if self.applying {
            return false;
        }
        self.applying = true;
        true
    }

    pub fn request_display_change(&mut self, label: &str) -> bool {
        if self.applying {
            self.deferred.insert(label.to_string());
            false
        } else {
            true
        }
    }

    pub fn finish(&mut self) -> Vec<String> {
        self.applying = false;
        std::mem::take(&mut self.deferred).into_iter().collect()
    }

    pub fn forget(&mut self, label: &str) {
        self.deferred.remove(label);
    }
}

#[derive(Default, Debug)]
pub struct Placement {
    pub preferred: Option<Point>,
    pub dragging: bool,
    pub drag_start: Option<Point>,
}

impl Placement {
    pub fn begin_user_move(&mut self, position: Point) {
        self.dragging = true;
        self.drag_start = Some(position);
    }

    pub fn finish_user_move(&mut self, position: Point) -> bool {
        let changed = self.dragging && self.drag_start.is_some_and(|start| start != position);
        self.dragging = false;
        self.drag_start = None;
        if changed {
            self.preferred = Some(position);
        }
        changed
    }

    pub fn target(&self, current: Point, width: u32, height: u32, work: Rect) -> Rect {
        fit(self.preferred.unwrap_or(current), width, height, work)
    }

    pub fn target_visible(
        &self,
        current: Point,
        width: u32,
        height: u32,
        work: Rect,
        margin: u32,
        frame: FrameInsets,
    ) -> Rect {
        fit_visible(
            self.preferred.unwrap_or(current),
            width,
            height,
            work,
            margin,
            frame,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_dpi_changes_are_reconciled_once_after_the_native_setter_finishes() {
        let mut state = ApplyState::default();
        assert!(state.begin());
        assert!(!state.begin());
        assert!(!state.request_display_change("main"));
        assert!(!state.request_display_change("main"));
        assert_eq!(state.finish(), vec!["main"]);
        assert!(state.begin());
        assert!(state.finish().is_empty());
        assert!(state.request_display_change("main"));
    }

    #[test]
    fn nested_display_changes_keep_each_live_window_and_forget_destroyed_windows() {
        let mut state = ApplyState::default();
        assert!(state.begin());
        for label in ["main", "classic-login", "web-browser-1"] {
            assert!(!state.request_display_change(label));
        }
        state.forget("classic-login");
        assert_eq!(state.finish(), vec!["main", "web-browser-1"]);
        assert!(state.finish().is_empty());
    }

    fn work(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect {
            origin: Point { x, y },
            width,
            height,
        }
    }

    #[test]
    fn an_in_bounds_window_does_not_move() {
        assert_eq!(
            fit(Point { x: 80, y: 90 }, 600, 400, work(0, 0, 1920, 1040)),
            work(80, 90, 600, 400)
        );
    }

    #[test]
    fn bottom_right_overflow_moves_only_the_required_distance() {
        assert_eq!(
            fit(Point { x: 1600, y: 900 }, 660, 680, work(0, 0, 1920, 1040)),
            work(1260, 360, 660, 680)
        );
    }

    #[test]
    fn left_and_top_overflow_respects_taskbar_insets() {
        assert_eq!(
            fit(Point { x: -50, y: -30 }, 420, 480, work(48, 24, 1872, 1016)),
            work(48, 24, 420, 480)
        );
    }

    #[test]
    fn negative_monitor_origins_are_preserved() {
        assert_eq!(
            fit(
                Point { x: -100, y: -30 },
                600,
                700,
                work(-1920, -1080, 1920, 1040)
            ),
            work(-600, -740, 600, 700)
        );
    }

    #[test]
    fn an_oversized_window_preserves_its_size_and_keeps_the_origin_reachable() {
        assert_eq!(
            fit(Point { x: 400, y: 300 }, 2400, 1600, work(0, 0, 1280, 680)),
            work(0, 0, 2400, 1600)
        );
    }

    #[test]
    fn popup_centres_in_the_owner_monitor_work_area() {
        assert_eq!(
            centred(900, 700, work(-1600, 40, 1600, 860)),
            work(-1250, 120, 900, 700)
        );
    }

    #[test]
    fn oversized_popup_preserves_its_size_and_keeps_the_origin_reachable() {
        assert_eq!(
            centred(1024, 720, work(1920, 0, 800, 600)),
            work(1920, 0, 1024, 720)
        );
    }

    #[test]
    fn growing_then_shrinking_restores_the_preferred_position() {
        let p = Placement {
            preferred: Some(Point { x: 1400, y: 700 }),
            dragging: false,
            drag_start: None,
        };
        let grown = p.target(Point { x: 1400, y: 700 }, 880, 680, work(0, 0, 1920, 1040));
        assert_eq!(grown.origin, Point { x: 1040, y: 360 });
        assert_eq!(
            p.target(grown.origin, 420, 300, work(0, 0, 1920, 1040))
                .origin,
            Point { x: 1400, y: 700 }
        );
        assert_eq!(p.preferred, Some(Point { x: 1400, y: 700 }));
    }

    #[test]
    fn only_a_user_drag_replaces_the_saved_preference() {
        let mut p = Placement {
            preferred: Some(Point { x: 100, y: 100 }),
            dragging: false,
            drag_start: None,
        };
        p.finish_user_move(Point { x: 20, y: 30 });
        assert_eq!(p.preferred, Some(Point { x: 100, y: 100 }));
        p.begin_user_move(Point { x: 100, y: 100 });
        p.finish_user_move(Point { x: 500, y: 400 });
        assert_eq!(p.preferred, Some(Point { x: 500, y: 400 }));
        assert!(!p.dragging);
    }

    #[test]
    fn monitor_selection_accepts_negative_coordinates_and_missing_displays() {
        let monitors = [work(-1600, 0, 1600, 900), work(0, 0, 1920, 1040)];
        assert_eq!(
            closest_monitor(Point { x: -1200, y: 200 }, &monitors),
            Some(0)
        );
        assert_eq!(
            closest_monitor(Point { x: 3000, y: 200 }, &monitors),
            Some(1)
        );
        assert_eq!(closest_monitor(Point { x: 0, y: 0 }, &[]), None);
    }

    #[test]
    fn dimensions_reject_nan_infinity_zero_negative_and_unbounded_values() {
        for value in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            0.0,
            -1.0,
            100_001.0,
        ] {
            assert!(!valid_logical_size(value, 480.0));
            assert!(!valid_logical_size(420.0, value));
        }
        assert!(valid_logical_size(420.5, 680.25));
    }

    #[test]
    fn extreme_coordinates_do_not_overflow() {
        assert_eq!(
            fit(
                Point {
                    x: i32::MAX,
                    y: i32::MIN
                },
                u32::MAX,
                u32::MAX,
                work(-1920, 0, 1920, 1040)
            ),
            work(-1920, 0, u32::MAX, u32::MAX)
        );
    }

    #[test]
    fn a_window_spanning_two_monitors_uses_its_largest_intersection() {
        let monitors = [work(0, 0, 1920, 1040), work(1920, 0, 1920, 1040)];
        assert_eq!(
            monitor_for_rect(work(1800, 100, 660, 680), &monitors),
            Some(1)
        );
    }

    #[test]
    fn a_portrait_monitor_with_negative_origin_is_selected_by_overlap() {
        let monitors = [work(-1440, -539, 1440, 2500), work(0, 0, 2560, 1380)];
        assert_eq!(
            monitor_for_rect(work(-100, 300, 660, 680), &monitors),
            Some(1)
        );
        assert_eq!(
            monitor_for_rect(work(-650, 1300, 660, 680), &monitors),
            Some(0)
        );
    }

    #[test]
    fn display_identity_survives_a_resolution_change_that_moves_its_old_centre() {
        let monitors = [
            (Some("primary"), work(0, 0, 1920, 1040)),
            (Some("secondary"), work(1920, 0, 1920, 1040)),
        ];
        assert_eq!(
            monitor_for_preference(Some("primary"), Point { x: 1920, y: 1000 }, &monitors),
            Some(0)
        );
        assert_eq!(
            monitor_for_preference(Some("disconnected"), Point { x: 5000, y: 1000 }, &monitors),
            Some(1)
        );
    }

    #[test]
    fn clicking_the_titlebar_or_cancelling_a_drag_preserves_the_preference() {
        let mut placement = Placement {
            preferred: Some(Point { x: 1400, y: 700 }),
            ..Default::default()
        };
        let clamped = Point { x: 1040, y: 360 };
        placement.begin_user_move(clamped);
        assert!(!placement.finish_user_move(clamped));
        assert_eq!(placement.preferred, Some(Point { x: 1400, y: 700 }));
        assert!(!placement.dragging);
    }

    #[test]
    fn correction_uses_visible_edges_and_a_physical_margin() {
        let frame = FrameInsets {
            left: 8,
            top: 0,
            right: 8,
            bottom: 8,
        };
        assert_eq!(
            fit_visible(
                Point { x: 1600, y: 900 },
                660,
                680,
                work(0, 0, 1920, 1040),
                13,
                frame
            ),
            work(1255, 355, 660, 680)
        );
    }

    #[test]
    fn a_single_overflow_axis_does_not_move_the_other_axis() {
        let frame = FrameInsets {
            left: 8,
            top: 0,
            right: 8,
            bottom: 8,
        };
        assert_eq!(
            fit_visible(
                Point { x: -20, y: -530 },
                660,
                680,
                work(-1440, -539, 1440, 2500),
                13,
                frame
            ),
            work(-665, -530, 660, 680)
        );
    }

    #[test]
    fn invisible_borders_outside_the_work_area_do_not_move_a_visible_in_bounds_window() {
        let frame = FrameInsets {
            left: 8,
            top: 0,
            right: 8,
            bottom: 8,
        };
        assert_eq!(
            fit_visible(
                Point { x: -8, y: 0 },
                420,
                480,
                work(0, 0, 1920, 1040),
                13,
                frame
            ),
            work(-8, 0, 420, 480)
        );
    }

    #[test]
    fn popup_centres_the_visible_rectangle_with_asymmetric_frame_insets() {
        let frame = FrameInsets {
            left: 8,
            top: 0,
            right: 8,
            bottom: 8,
        };
        assert_eq!(
            centred_visible(660, 680, work(0, 0, 1920, 1040), 13, frame),
            work(630, 184, 660, 680)
        );
    }

    #[test]
    fn in_bounds_edges_stay_put_and_a_one_pixel_overflow_gets_the_requested_margin() {
        let screen = work(0, 0, 1000, 800);
        for gap in [0, 5, 10] {
            let point = Point {
                x: 1000 - 420 - gap,
                y: 800 - 480 - gap,
            };
            assert_eq!(
                fit_visible(point, 420, 480, screen, 10, FrameInsets::default()).origin,
                point
            );
        }
        assert_eq!(
            fit_visible(
                Point { x: 581, y: 315 },
                420,
                480,
                screen,
                10,
                FrameInsets::default()
            )
            .origin,
            Point { x: 570, y: 315 }
        );
        assert_eq!(
            fit_visible(
                Point { x: 581, y: 321 },
                420,
                480,
                screen,
                10,
                FrameInsets::default()
            )
            .origin,
            Point { x: 570, y: 310 }
        );
    }

    #[test]
    fn insufficient_margin_space_preserves_size_without_overflowing_the_opposite_edge() {
        let screen = work(0, 0, 1000, 800);
        assert_eq!(
            fit_visible(
                Point { x: 10, y: -10 },
                995,
                795,
                screen,
                10,
                FrameInsets::default()
            ),
            work(0, 5, 995, 795)
        );
    }

    #[test]
    fn every_ordered_size_transition_is_reversible_across_edges_taskbars_and_dpi() {
        // Representative route sizes were measured from the real zh-TW app
        // with its download banner and two service accounts. Other locales and
        // content remain dynamic; popup entries use their configured sizes.
        let sizes: [(f64, f64); 17] = [
            (420.0, 452.8),
            (420.0, 320.0),
            (420.0, 591.0),
            (420.0, 382.0),
            (420.0, 305.0),
            (420.0, 400.0),
            (420.0, 307.0),
            (420.0, 522.0),
            (480.0, 438.0),
            (660.0, 1016.0),
            (420.0, 535.0),
            (880.0, 522.0),
            (640.0, 720.0),
            (900.0, 700.0),
            (600.0, 720.0),
            (1024.0, 720.0),
            (850.0, 596.0),
        ];
        let mut transitions = 0;
        for scale in [1.0_f64, 1.25, 1.5, 2.0] {
            let margin = (10.0 * scale).round() as u32;
            let border = (6.4 * scale).round() as u32;
            let frame = FrameInsets {
                left: border,
                right: border,
                top: 0,
                bottom: border,
            };
            for (origin_x, origin_y) in [(0, 0), (-1920, -539)] {
                for screen in [
                    work(origin_x, origin_y + 40, 1920, 1040),
                    work(origin_x, origin_y, 1920, 1040),
                    work(origin_x + 40, origin_y, 1880, 1080),
                    work(origin_x, origin_y, 1880, 1080),
                ] {
                    for (index_a, &(a_width, a_height)) in sizes.iter().enumerate() {
                        for (index_b, &(b_width, b_height)) in sizes.iter().enumerate() {
                            if index_a == index_b {
                                continue;
                            }
                            let (aw, ah) = (
                                (a_width * scale).round() as u32,
                                (a_height * scale).round() as u32,
                            );
                            let (bw, bh) = (
                                (b_width * scale).round() as u32,
                                (b_height * scale).round() as u32,
                            );
                            for horizontal in [0, 1, 2] {
                                for vertical in [0, 1, 2] {
                                    let visible_width = aw.saturating_sub(border * 2);
                                    let visible_height = ah.saturating_sub(border);
                                    let x = i64::from(screen.origin.x)
                                        + (i64::from(screen.width) - i64::from(visible_width))
                                            * horizontal
                                            / 2
                                        - i64::from(border);
                                    let y = i64::from(screen.origin.y)
                                        + (i64::from(screen.height) - i64::from(visible_height))
                                            * vertical
                                            / 2;
                                    let preferred = Point {
                                        x: x as i32,
                                        y: y as i32,
                                    };
                                    let placement = Placement {
                                        preferred: Some(preferred),
                                        ..Default::default()
                                    };
                                    let a = placement
                                        .target_visible(preferred, aw, ah, screen, margin, frame);
                                    let b = placement
                                        .target_visible(a.origin, bw, bh, screen, margin, frame);
                                    let restored = placement
                                        .target_visible(b.origin, aw, ah, screen, margin, frame);
                                    assert_eq!((a.width, a.height), (aw, ah));
                                    assert_eq!((b.width, b.height), (bw, bh));
                                    assert_eq!(
                                        restored, a,
                                        "A→B→A must restore the same placement"
                                    );
                                    assert_eq!(placement.preferred, Some(preferred));
                                    for rect in [a, b, restored] {
                                        let visible_x =
                                            i64::from(rect.origin.x) + i64::from(border);
                                        let visible_y = i64::from(rect.origin.y);
                                        let visible_w =
                                            i64::from(rect.width.saturating_sub(border * 2));
                                        let visible_h =
                                            i64::from(rect.height.saturating_sub(border));
                                        assert!(visible_x >= i64::from(screen.origin.x));
                                        assert!(visible_y >= i64::from(screen.origin.y));
                                        if visible_w <= i64::from(screen.width) {
                                            assert!(
                                                visible_x + visible_w
                                                    <= i64::from(screen.origin.x)
                                                        + i64::from(screen.width)
                                            );
                                        }
                                        if visible_h <= i64::from(screen.height) {
                                            assert!(
                                                visible_y + visible_h
                                                    <= i64::from(screen.origin.y)
                                                        + i64::from(screen.height)
                                            );
                                        }
                                        // Check each axis against the original user
                                        // preference, including the unchanged axis
                                        // when only the other one overflows.
                                        let mut corrected_gaps = Vec::new();
                                        for (original, actual, length, start, available) in [
                                            (
                                                i64::from(preferred.x) + i64::from(border),
                                                visible_x,
                                                visible_w,
                                                i64::from(screen.origin.x),
                                                i64::from(screen.width),
                                            ),
                                            (
                                                i64::from(preferred.y),
                                                visible_y,
                                                visible_h,
                                                i64::from(screen.origin.y),
                                                i64::from(screen.height),
                                            ),
                                        ] {
                                            let end = start + available;
                                            if length > available {
                                                assert_eq!(actual, start + i64::from(margin));
                                            } else if original >= start && original + length <= end
                                            {
                                                assert_eq!(
                                                    actual, original,
                                                    "an in-bounds axis must not move"
                                                );
                                            } else {
                                                let gap = if original < start {
                                                    actual - start
                                                } else {
                                                    end - (actual + length)
                                                };
                                                assert_eq!(
                                                    gap,
                                                    i64::from(margin).min(available - length)
                                                );
                                                if available - length >= i64::from(margin) {
                                                    corrected_gaps.push(gap);
                                                }
                                            }
                                        }
                                        if corrected_gaps.len() == 2 {
                                            assert_eq!(corrected_gaps[0], corrected_gaps[1], "both overflowing corner edges must have equal margins");
                                        }
                                    }
                                    transitions += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(transitions, 78_336);
    }
}
