//! 2D collision detection: axis-aligned boxes and axis-aligned ellipses.
//!
//! Detection only. What a game does about a hit — restart, bounce, take damage
//! — is the game's rule, so it lives in the game's systems. This module is
//! geometry, and it never touches a rendering component.
//!
//! Every shape's bounds are its `Collider::size`, so the cheap pre-test is
//! always box-versus-box regardless of shape.
//!
//! Exactness, stated plainly: box↔box gives the true minimum translation
//! vector. Anything involving an ellipse is tested in a space where the ellipse
//! is a unit circle, which preserves *whether* they overlap but not distance —
//! so those depths are a first-order estimate, good enough to push out of a
//! penetration and not good enough for precise resting contact.

use scene_format::{Collider, Shape};

/// A collider at a place in the world.
#[derive(Clone, Copy, Debug)]
pub struct Placed {
    /// The node's translation. `Collider::offset` is added to it.
    pub at: [f32; 2],
    pub collider: Collider,
}

impl Placed {
    fn center(&self) -> [f32; 2] {
        [
            self.at[0] + self.collider.offset[0],
            self.at[1] + self.collider.offset[1],
        ]
    }

    /// Half extents of the bounding box, for either shape.
    fn half(&self) -> [f32; 2] {
        [self.collider.size[0] * 0.5, self.collider.size[1] * 0.5]
    }
}

/// One contact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// Unit direction to move `a` so it stops overlapping `b`.
    pub normal: [f32; 2],
    /// How far along `normal` that takes.
    pub depth: f32,
}

/// Do the bounding boxes overlap? The broadphase, and the whole test for
/// box↔box.
pub fn bounds_overlap(a: &Placed, b: &Placed) -> bool {
    let (ca, cb) = (a.center(), b.center());
    let (ha, hb) = (a.half(), b.half());
    (ca[0] - cb[0]).abs() < ha[0] + hb[0] && (ca[1] - cb[1]).abs() < ha[1] + hb[1]
}

/// Test `a` against `b`, returning how to separate `a`.
pub fn hit(a: &Placed, b: &Placed) -> Option<Hit> {
    if !bounds_overlap(a, b) {
        return None;
    }
    match (a.collider.shape, b.collider.shape) {
        (Shape::Box, Shape::Box) => Some(box_box(a, b)),
        (Shape::Box, Shape::Ellipse) => box_ellipse(a, b, false),
        (Shape::Ellipse, Shape::Box) => box_ellipse(b, a, true),
        (Shape::Ellipse, Shape::Ellipse) => ellipse_ellipse(a, b),
    }
}

/// Exact minimum translation vector: separate along whichever axis overlaps
/// least.
fn box_box(a: &Placed, b: &Placed) -> Hit {
    let (ca, cb) = (a.center(), b.center());
    let (ha, hb) = (a.half(), b.half());
    let dx = cb[0] - ca[0];
    let dy = cb[1] - ca[1];
    let px = ha[0] + hb[0] - dx.abs();
    let py = ha[1] + hb[1] - dy.abs();

    if px < py {
        Hit {
            normal: [-dx.signum(), 0.0],
            depth: px,
        }
    } else {
        Hit {
            normal: [0.0, -dy.signum()],
            depth: py,
        }
    }
}

/// Box against ellipse. Dividing by the ellipse's radii turns it into a unit
/// circle and leaves the box axis-aligned, so this reduces to circle-vs-box.
///
/// `flip` reverses the normal, for when the caller asked about the ellipse.
fn box_ellipse(bx: &Placed, el: &Placed, flip: bool) -> Option<Hit> {
    let (rx, ry) = (el.half()[0], el.half()[1]);
    if rx <= 0.0 || ry <= 0.0 {
        return None;
    }
    let (cb, ce) = (bx.center(), el.center());
    let hb = bx.half();

    // Box centre and half extents in the space where the ellipse is a unit
    // circle at the origin.
    let c = [(cb[0] - ce[0]) / rx, (cb[1] - ce[1]) / ry];
    let h = [hb[0] / rx, hb[1] / ry];

    // Closest point on the box to the origin.
    let p = [
        0.0f32.clamp(c[0] - h[0], c[0] + h[0]),
        0.0f32.clamp(c[1] - h[1], c[1] + h[1]),
    ];
    let dist = (p[0] * p[0] + p[1] * p[1]).sqrt();

    let (dir, depth_scaled) = if dist > 1e-6 {
        if dist >= 1.0 {
            return None;
        }
        // Move the box away from the circle: along +p.
        ([p[0] / dist, p[1] / dist], 1.0 - dist)
    } else {
        // The circle's centre is inside the box: leave by the nearest face.
        let out_x = h[0] - (c[0]).abs().min(h[0]);
        let px = h[0] - c[0].abs() + 1.0;
        let py = h[1] - c[1].abs() + 1.0;
        let _ = out_x;
        if px < py {
            ([-c[0].signum(), 0.0], px)
        } else {
            ([0.0, -c[1].signum()], py)
        }
    };

    // Back to world: a direction maps by the ellipse's radii, then renormalise.
    // The depth is scaled by how much the world stretches along that direction,
    // which is where this stops being exact.
    let w = [dir[0] * rx, dir[1] * ry];
    let wl = (w[0] * w[0] + w[1] * w[1]).sqrt();
    if wl <= 1e-9 {
        return None;
    }
    let normal = [w[0] / wl, w[1] / wl];
    let depth = depth_scaled * wl;
    Some(Hit {
        normal: if flip {
            [-normal[0], -normal[1]]
        } else {
            normal
        },
        depth,
    })
}

/// Ellipse against ellipse. Dividing by `a`'s radii makes `a` a unit circle and
/// leaves `b` an axis-aligned ellipse, then Newton finds the closest point on
/// `b` to the circle's centre.
fn ellipse_ellipse(a: &Placed, b: &Placed) -> Option<Hit> {
    let (arx, ary) = (a.half()[0], a.half()[1]);
    let (brx, bry) = (b.half()[0], b.half()[1]);
    if arx <= 0.0 || ary <= 0.0 || brx <= 0.0 || bry <= 0.0 {
        return None;
    }
    let (ca, cb) = (a.center(), b.center());

    // In `a`-normalised space: unit circle at the origin, ellipse at `d` with
    // radii `p`, `q`.
    let d = [(cb[0] - ca[0]) / arx, (cb[1] - ca[1]) / ary];
    let p = brx / arx;
    let q = bry / ary;

    // Closest point on the ellipse to the origin: minimise
    // |d + (p cos t, q sin t)|². Newton on the derivative, from the angle of
    // the direction back towards the origin.
    let mut t = (-d[1]).atan2(-d[0]);
    for _ in 0..8 {
        let (s, c) = t.sin_cos();
        let x = d[0] + p * c;
        let y = d[1] + q * s;
        // f(t) = derivative of the squared distance, halved.
        let f = -p * s * x + q * c * y;
        let df = -p * c * x + p * p * s * s - q * s * y + q * q * c * c;
        if df.abs() < 1e-9 {
            break;
        }
        let step = f / df;
        t -= step;
        if step.abs() < 1e-6 {
            break;
        }
    }
    let (s, c) = t.sin_cos();
    let closest = [d[0] + p * c, d[1] + q * s];
    let dist = (closest[0] * closest[0] + closest[1] * closest[1]).sqrt();
    if dist >= 1.0 {
        return None;
    }

    // Direction to move `a` away from `b`, in normalised space.
    let dir = if dist > 1e-6 {
        [-closest[0] / dist, -closest[1] / dist]
    } else {
        let n = (d[0] * d[0] + d[1] * d[1]).sqrt();
        if n > 1e-6 {
            [-d[0] / n, -d[1] / n]
        } else {
            [-1.0, 0.0]
        }
    };

    let w = [dir[0] * arx, dir[1] * ary];
    let wl = (w[0] * w[0] + w[1] * w[1]).sqrt();
    if wl <= 1e-9 {
        return None;
    }
    Some(Hit {
        normal: [w[0] / wl, w[1] / wl],
        depth: (1.0 - dist) * wl,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placed(shape: Shape, at: [f32; 2], size: [f32; 2]) -> Placed {
        Placed {
            at,
            collider: Collider {
                shape,
                size,
                offset: [0.0, 0.0],
            },
        }
    }

    fn b(at: [f32; 2], size: [f32; 2]) -> Placed {
        placed(Shape::Box, at, size)
    }

    fn e(at: [f32; 2], size: [f32; 2]) -> Placed {
        placed(Shape::Ellipse, at, size)
    }

    // ── box ↔ box: exact ────────────────────────────────────────────────────

    #[test]
    fn separated_boxes_do_not_hit() {
        assert!(hit(&b([0.0, 0.0], [1.0, 1.0]), &b([2.0, 0.0], [1.0, 1.0])).is_none());
    }

    #[test]
    fn box_mtv_uses_the_shallower_axis() {
        // Overlap 0.5 on x, 0.9 on y — separate along x.
        let h = hit(&b([0.0, 0.0], [1.0, 1.0]), &b([0.5, 0.1], [1.0, 1.0])).unwrap();
        assert_eq!(h.normal, [-1.0, 0.0]);
        assert!((h.depth - 0.5).abs() < 1e-5, "depth {}", h.depth);
    }

    #[test]
    fn box_normal_points_away_from_the_other_box() {
        let left = hit(&b([0.0, 0.0], [1.0, 1.0]), &b([-0.5, 0.0], [1.0, 1.0])).unwrap();
        assert_eq!(left.normal, [1.0, 0.0], "b is left, so a moves right");
    }

    #[test]
    fn touching_boxes_do_not_hit() {
        assert!(hit(&b([0.0, 0.0], [1.0, 1.0]), &b([1.0, 0.0], [1.0, 1.0])).is_none());
    }

    // ── circle is an ellipse with equal radii, so these are exact ──────────

    #[test]
    fn circle_and_box_side_by_side() {
        // Circle r = 0.5 centred 0.9 right of a unit box: gap 0.9 - 0.5 - 0.5.
        assert!(hit(&b([0.0, 0.0], [1.0, 1.0]), &e([1.1, 0.0], [1.0, 1.0])).is_none());
        let h = hit(&b([0.0, 0.0], [1.0, 1.0]), &e([0.9, 0.0], [1.0, 1.0])).unwrap();
        assert_eq!(h.normal, [-1.0, 0.0]);
        assert!((h.depth - 0.1).abs() < 1e-4, "depth {}", h.depth);
    }

    #[test]
    fn circle_clears_a_box_corner_diagonally() {
        // Corner at (0.5, 0.5); circle r = 0.5 centred at (1.0, 1.0) is
        // sqrt(0.5) ~ 0.707 away, so it does not reach.
        assert!(hit(&b([0.0, 0.0], [1.0, 1.0]), &e([1.0, 1.0], [1.0, 1.0])).is_none());
        // Move it in and it does.
        assert!(hit(&b([0.0, 0.0], [1.0, 1.0]), &e([0.8, 0.8], [1.0, 1.0])).is_some());
    }

    /// Ellipse-versus-ellipse solves for the closest point iteratively, so its
    /// results carry solver noise and are compared with a tolerance.
    #[test]
    fn two_circles_separate_along_their_centres() {
        let h = hit(&e([0.0, 0.0], [2.0, 2.0]), &e([1.5, 0.0], [2.0, 2.0])).unwrap();
        assert!((h.normal[0] + 1.0).abs() < 1e-4, "normal {:?}", h.normal);
        assert!(h.normal[1].abs() < 1e-4, "normal {:?}", h.normal);
        assert!((h.depth - 0.5).abs() < 1e-4, "depth {}", h.depth);
        assert!(hit(&e([0.0, 0.0], [2.0, 2.0]), &e([2.5, 0.0], [2.0, 2.0])).is_none());
    }

    // ── ellipse: the shape actually matters ────────────────────────────────

    /// A wide flat ellipse reaches sideways where its bounding box would also
    /// reach, but not diagonally — this is the case a bounding box gets wrong.
    #[test]
    fn a_flat_ellipse_does_not_reach_its_bounding_box_corner() {
        let flat = e([0.0, 0.0], [4.0, 0.4]);
        // Straight out along x, inside: 1.8 < 2.0.
        assert!(hit(&flat, &e([1.8, 0.0], [0.05, 0.05])).is_some());
        // Same distance but diagonal: outside the ellipse, inside the box.
        let corner = e([1.8, 0.18], [0.05, 0.05]);
        assert!(bounds_overlap(&flat, &corner), "bounds do overlap");
        assert!(
            hit(&flat, &corner).is_none(),
            "the ellipse should not reach its box corner"
        );
    }

    #[test]
    fn ellipse_versus_box_is_symmetric_in_detection() {
        let bx = b([0.0, 0.0], [1.0, 1.0]);
        let el = e([0.7, 0.0], [1.0, 0.6]);
        assert_eq!(hit(&bx, &el).is_some(), hit(&el, &bx).is_some());
    }

    #[test]
    fn flipping_the_arguments_flips_the_normal() {
        let bx = b([0.0, 0.0], [1.0, 1.0]);
        let el = e([0.9, 0.0], [1.0, 1.0]);
        let ab = hit(&bx, &el).unwrap();
        let ba = hit(&el, &bx).unwrap();
        assert!((ab.normal[0] + ba.normal[0]).abs() < 1e-4);
        assert!((ab.normal[1] + ba.normal[1]).abs() < 1e-4);
    }

    // ── offset ────────────────────────────────────────────────────────────

    #[test]
    fn offset_moves_the_collider_off_the_node_origin() {
        let mut feet = b([0.0, 0.0], [1.0, 1.0]);
        feet.collider.offset = [0.0, -2.0];
        // At the node's origin there is nothing to hit any more.
        assert!(hit(&feet, &b([0.0, 0.0], [1.0, 1.0])).is_none());
        // Two units down there is.
        assert!(hit(&feet, &b([0.0, -2.0], [1.0, 1.0])).is_some());
    }

    // ── degenerate ────────────────────────────────────────────────────────

    #[test]
    fn a_zero_sized_collider_never_hits() {
        assert!(hit(&e([0.0, 0.0], [0.0, 0.0]), &b([0.0, 0.0], [1.0, 1.0])).is_none());
    }

    #[test]
    fn depth_is_always_positive_when_there_is_a_hit() {
        let cases = [
            (b([0.0, 0.0], [1.0, 1.0]), b([0.3, 0.3], [1.0, 1.0])),
            (b([0.0, 0.0], [1.0, 1.0]), e([0.6, 0.2], [1.0, 0.8])),
            (e([0.0, 0.0], [2.0, 1.0]), e([1.0, 0.2], [1.0, 1.0])),
        ];
        for (x, y) in cases {
            if let Some(h) = hit(&x, &y) {
                assert!(h.depth > 0.0, "depth {} for {x:?} {y:?}", h.depth);
                let len = (h.normal[0] * h.normal[0] + h.normal[1] * h.normal[1]).sqrt();
                assert!((len - 1.0).abs() < 1e-4, "normal not unit: {:?}", h.normal);
            }
        }
    }
}
