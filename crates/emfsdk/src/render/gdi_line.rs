//! Classic GDI cosmetic lines retain 28.4 device coordinates. Their coverage
//! follows the legacy GIQ diamond-exit rule, including its slope-dependent
//! boundary ownership and excluded final endpoint. See Intel's Haswell PRM,
//! volume 7, "GIQ (Diamond) Sampling Rules – Legacy Mode", pp. 580–581, and
//! Microsoft Win32 GDI "Lines". This is distinct from DX10 line rasterization.

use std::cmp::Ordering;

#[derive(Clone, Copy)]
struct Ratio {
  numerator: i64,
  denominator: i64,
}

impl Ratio {
  fn new(numerator: i64, denominator: i64) -> Self {
    if denominator < 0 {
      Self {
        numerator: -numerator,
        denominator: -denominator,
      }
    } else {
      Self {
        numerator,
        denominator,
      }
    }
  }

  fn compare(self, other: Self) -> Ordering {
    (i128::from(self.numerator) * i128::from(other.denominator))
      .cmp(&(i128::from(other.numerator) * i128::from(self.denominator)))
  }
}

struct Line {
  x: i64,
  y: i64,
  dx: i64,
  dy: i64,
}

impl Line {
  fn inside(&self, t: Ratio, x: i64, y: i64) -> bool {
    let denominator = i128::from(t.denominator);
    let rx =
      i128::from(self.x - x * 16) * denominator + i128::from(self.dx) * i128::from(t.numerator);
    let ry =
      i128::from(self.y - y * 16) * denominator + i128::from(self.dy) * i128::from(t.numerator);
    let radius = 8 * denominator;
    let distance = rx.abs() + ry.abs();
    distance < radius
      || (rx == 0 && ry == radius)
      || (ry == 0 && rx == if self.dx == self.dy { -radius } else { radius })
      || (distance == radius
        && ry > 0
        && ((rx < 0 && self.dx == self.dy) || (rx > 0 && self.dx == -self.dy)))
  }

  fn covers(&self, x: i64, y: i64) -> bool {
    // In (x+y, x-y) coordinates the closed diamond is an axis-aligned box.
    // Keep exact ratios: epsilon comparisons would change boundary ownership.
    let mut enter = Ratio::new(0, 1);
    let mut leave: Option<Ratio> = None;
    for (origin, delta, center, inclusive_lower, inclusive_upper) in [
      (
        self.x + self.y,
        self.dx + self.dy,
        (x + y) * 16,
        false,
        true,
      ),
      (
        self.x - self.y,
        self.dx - self.dy,
        (x - y) * 16,
        true,
        false,
      ),
    ] {
      let low = center - 8;
      let high = center + 8;
      if delta == 0 {
        // A line parallel to a diamond edge has slope exactly +/-1. Only
        // the lower edges then belong to the legacy diamond.
        if origin < low
          || origin > high
          || (origin == low && !inclusive_lower)
          || (origin == high && !inclusive_upper)
        {
          return false;
        }
      } else {
        let mut start = Ratio::new(low - origin, delta);
        let mut end = Ratio::new(high - origin, delta);
        if start.compare(end).is_gt() {
          std::mem::swap(&mut start, &mut end);
        }
        if start.compare(enter).is_gt() {
          enter = start;
        }
        if leave.is_none_or(|old| end.compare(old).is_lt()) {
          leave = Some(end);
        }
      }
    }
    let Some(leave) = leave else { return false };
    if enter.compare(leave).is_gt() || leave.numerator > leave.denominator {
      return false;
    }
    if leave.numerator == leave.denominator && self.inside(leave, x, y) {
      return false;
    }
    // A tangent (including one at the start) counts only at an owned corner.
    enter.compare(leave).is_lt() || self.inside(leave, x, y)
  }
}

pub(super) fn visit_cosmetic_line(
  start: (f32, f32),
  end: (f32, f32),
  clip: (i32, i32, i32, i32),
  mut visit: impl FnMut(i32, i32),
) {
  if ![start.0, start.1, end.0, end.1]
    .iter()
    .all(|v| v.is_finite())
  {
    return;
  }
  let fixed = |value: f32| i64::from((f64::from(value) * 16.0).round() as i32);
  let (x0, y0) = (fixed(start.0), fixed(start.1));
  let (x1, y1) = (fixed(end.0), fixed(end.1));
  let line = Line {
    x: x0,
    y: y0,
    dx: x1 - x0,
    dy: y1 - y0,
  };
  if line.dx == 0 && line.dy == 0 {
    return;
  }
  let transpose = line.dy.abs() > line.dx.abs();
  let (major0, minor0, major1, minor1, min_major, max_major, min_minor, max_minor) = if transpose {
    (y0, x0, y1, x1, clip.1, clip.3, clip.0, clip.2)
  } else {
    (x0, y0, x1, y1, clip.0, clip.2, clip.1, clip.3)
  };
  let begin = (major0.min(major1) - 8)
    .div_euclid(16)
    .max(i64::from(min_major));
  let end = (major0.max(major1) + 8)
    .div_euclid(16)
    .min(i64::from(max_major) - 1);
  let major_delta = i128::from(major1 - major0);
  let minor_delta = i128::from(minor1 - minor0);
  let denominator = major_delta.abs() * 16;
  // Iterate only the visible major axis, with at most two candidate diamonds
  // per position. Clipping must not move endpoints or reset the line phase.
  for major in begin..=end {
    let numerator = (i128::from(minor0) * major_delta
      + i128::from(major * 16 - major0) * minor_delta)
      * major_delta.signum();
    let minor = numerator.div_euclid(denominator) as i64;
    for minor in minor..=minor + 1 {
      if minor < i64::from(min_minor) || minor >= i64::from(max_minor) {
        continue;
      }
      let (x, y) = if transpose {
        (minor, major)
      } else {
        (major, minor)
      };
      if line.covers(x, y) {
        visit(x as i32, y as i32);
      }
    }
  }
}
