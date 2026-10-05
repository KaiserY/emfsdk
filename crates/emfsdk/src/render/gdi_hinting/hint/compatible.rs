//! Deferred GDI compatible-width adjustment.
//!
//! ClearType keeps the fine-grid outline but uses the complete bi-level
//! program's advance. Relative hints establish a first-parent topology;
//! IUP[X] applies the width correction to that topology before interpolating
//! untouched points. Absolute moves do not erase a previously recorded link.
//! See Microsoft’s ClearType instruction guidance and US6377262B1. The exact
//! first-link, forward-reference and interpolation semantics are covered by
//! native GDI coordinate-readback controls.

use super::{F26Dot6, Point, math};

#[derive(Clone, Copy, Debug)]
pub(super) enum Link {
  Shift(usize),
  Interpolate {
    a: usize,
    b: usize,
    numerator: i32,
    denominator: i32,
  },
}

#[derive(Default, Debug)]
pub(super) struct CompatibleWidth {
  links: Vec<Option<Link>>,
  stem_children: Vec<Option<usize>>,
  stem_parents: Vec<Option<usize>>,
  original_advance: i32,
  mono_advance: i32,
  applied: bool,
}

impl CompatibleWidth {
  pub fn new(points: &[Point<F26Dot6>], advance: Option<F26Dot6>) -> Self {
    let Some(advance) = advance else {
      return Self::default();
    };
    let Some(left) = points.len().checked_sub(4) else {
      return Self::default();
    };
    let original_advance = (points[left + 1].x - points[left].x).to_bits();
    if original_advance == 0 || original_advance == advance.to_bits() {
      return Self::default();
    }
    Self {
      links: vec![None; points.len()],
      stem_children: vec![None; points.len()],
      stem_parents: vec![None; points.len()],
      original_advance,
      mono_advance: advance.to_bits(),
      applied: false,
    }
  }

  pub fn record(&mut self, point: usize, link: Link) {
    if self.applied || point >= self.links.len().saturating_sub(4) {
      return;
    }
    let valid = match link {
      Link::Shift(parent) => parent != point && parent < self.links.len(),
      Link::Interpolate {
        a, b, denominator, ..
      } => {
        a != point && b != point && a < self.links.len() && b < self.links.len() && denominator != 0
      }
    };
    if valid && self.links[point].is_none() {
      self.links[point] = Some(link);
    }
  }

  /// Indirect black distances preserve the stem width while its center
  /// follows the compatible advance. GDI also recognizes consecutive
  /// outline points for gray/white MIRP distances. Phantom links remain
  /// ordinary dependencies; they are not glyph stems.
  pub fn stem(&mut self, parent: usize, point: usize, black: bool) {
    let count = self.links.len().saturating_sub(4);
    if self.applied || parent >= count || point >= count || parent == point {
      return;
    }
    if black || parent.abs_diff(point) == 1 {
      self.stem_children[parent].get_or_insert(point);
      if matches!(self.links[point], Some(Link::Shift(p)) if p == parent) {
        self.stem_parents[point].get_or_insert(parent);
      }
    }
  }

  pub fn apply(&mut self, points: &mut [Point<F26Dot6>]) {
    if self.applied || self.links.is_empty() || points.len() != self.links.len() {
      return;
    }
    self.applied = true;
    let left = points.len() - 4;
    let current_advance = (points[left + 1].x - points[left].x).to_bits();
    let adjustment = math::mul_div(
      current_advance,
      self.mono_advance.wrapping_sub(self.original_advance),
      self.original_advance,
    );
    // Resolve forward references without recursion. Unresolvable cycles do
    // not move points; a malformed font cannot make this pass loop forever.
    let mut values = vec![None; points.len()];
    for i in 0..left {
      let Some(partner) = self.stem_children[i].or(self.stem_parents[i]) else {
        continue;
      };
      if partner < i || values[i].is_some() || values[partner].is_some() {
        continue;
      }
      // A new stem can close a pre-existing relative/interpolation link
      // back onto its own reference. Native GDI leaves the new child
      // fixed and normalizes the earlier link independently. Do not turn
      // that cycle into a fictitious, rigid two-point stem.
      let mut cyclic = false;
      for (point, child) in [(i, partner), (partner, i)] {
        if self.stem_parents[point] == Some(child)
          || !matches!(self.links[child], Some(Link::Shift(p)) if p == point)
        {
          continue;
        }
        let delta = match self.links[point] {
          Some(Link::Shift(p)) if p == child => Some(self.normalize(points[point].x.to_bits())),
          Some(Link::Interpolate {
            a,
            b,
            numerator,
            denominator,
          }) if a == child || b == child => Some(interpolate(
            self.normalize(points[a].x.to_bits()),
            self.normalize(points[b].x.to_bits()),
            numerator,
            denominator,
          )),
          _ => None,
        };
        if let Some(delta) = delta {
          values[point] = Some(delta);
          values[child] = Some(0);
          cyclic = true;
          break;
        }
      }
      if cyclic {
        continue;
      }
      let reference = if self.stem_children[i] == Some(partner) {
        i
      } else {
        partner
      };
      // Interpolation between both horizontal bearings already anchors the
      // stem to the advance. Keep that explicit metric relationship instead
      // of replacing it with independent center normalization.
      if matches!(self.links[reference], Some(Link::Interpolate { a, b, .. })
        if (a == left && b == left + 1) || (b == left && a == left + 1))
      {
        continue;
      }
      let midpoint =
        (i64::from(points[i].x.to_bits()) + i64::from(points[partner].x.to_bits())) / 2;
      let delta = self.normalize(midpoint as i32);
      values[i] = Some(delta);
      values[partner] = Some(delta);
    }
    let mut dependents = vec![Vec::new(); points.len()];
    let mut pending = vec![0u8; points.len()];
    let mut ready = Vec::new();
    for (i, link) in self.links.iter().enumerate() {
      if values[i].is_some() {
        ready.push(i);
        continue;
      }
      match *link {
        None => {
          values[i] = Some(if i == left + 1 { adjustment } else { 0 });
          ready.push(i);
        }
        Some(Link::Shift(parent)) => {
          dependents[parent].push(i);
          pending[i] = 1;
        }
        Some(Link::Interpolate { a, b, .. }) => {
          dependents[a].push(i);
          dependents[b].push(i);
          pending[i] = 2;
        }
      }
    }
    while let Some(parent) = ready.pop() {
      for &i in &dependents[parent] {
        pending[i] -= 1;
        if pending[i] != 0 {
          continue;
        }
        values[i] = match self.links[i] {
          Some(Link::Shift(p)) => values[p],
          Some(Link::Interpolate {
            a,
            b,
            numerator,
            denominator,
          }) => {
            values[a].zip(values[b]).map(|(a, b)| {
              // Native compatibility interpolation truncates the correction;
              // ordinary outline IP/IUP retain their own rounding rules.
              interpolate(a, b, numerator, denominator)
            })
          }
          None => None,
        };
        ready.push(i);
      }
    }
    for (point, delta) in points[..left].iter_mut().zip(values) {
      point.x = point.x.wrapping_add(F26Dot6::from_bits(delta.unwrap_or(0)));
    }
  }

  fn normalize(&self, coordinate: i32) -> i32 {
    math::mul_div(
      coordinate,
      self.mono_advance.wrapping_sub(self.original_advance),
      self.original_advance,
    )
  }
}

fn interpolate(a: i32, b: i32, numerator: i32, denominator: i32) -> i32 {
  ((i128::from(a) * (i128::from(denominator) - i128::from(numerator))
    + i128::from(b) * i128::from(numerator))
    / i128::from(denominator))
  .clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32
}
