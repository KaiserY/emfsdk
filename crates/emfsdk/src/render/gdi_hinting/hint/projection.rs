// Adapted from skrifa 0.46.2; see render/gdi_hinting/{LICENSE-MIT,README.md}.
//! Point projection.

use super::graphics::{CoordAxis, GraphicsState};
use skrifa::raw::types::{F26Dot6, Point};

impl GraphicsState<'_> {
  /// Updates cached state that is derived from projection vectors.
  pub fn update_projection_state(&mut self) {
    // 1.0 in 2.14 fixed point.
    const ONE: i32 = 0x4000;
    // Based on Compute_Funcs() at
    // <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L2482>.
    // FreeType uses function pointers to select between various "modes"
    // but we use the CoordAxis type instead.
    if self.freedom_vector.x == ONE {
      self.fdotp = self.proj_vector.x;
    } else if self.freedom_vector.y == ONE {
      self.fdotp = self.proj_vector.y;
    } else {
      let px = self.proj_vector.x;
      let py = self.proj_vector.y;
      let fx = self.freedom_vector.x;
      let fy = self.freedom_vector.y;
      self.fdotp = (px * fx + py * fy) >> 14;
    }
    self.proj_axis = CoordAxis::Both;
    if self.proj_vector.x == ONE {
      self.proj_axis = CoordAxis::X;
    } else if self.proj_vector.y == ONE {
      self.proj_axis = CoordAxis::Y;
    }
    self.dual_proj_axis = CoordAxis::Both;
    if self.dual_proj_vector.x == ONE {
      self.dual_proj_axis = CoordAxis::X;
    } else if self.dual_proj_vector.y == ONE {
      self.dual_proj_axis = CoordAxis::Y;
    }
    self.freedom_axis = CoordAxis::Both;
    if self.fdotp == ONE {
      if self.freedom_vector.x == ONE {
        self.freedom_axis = CoordAxis::X;
      } else if self.freedom_vector.y == ONE {
        self.freedom_axis = CoordAxis::Y;
      }
    }
    // At small sizes, fdotp can become too small resulting in overflows
    // and spikes.
    if self.fdotp.abs() < 0x400 {
      self.fdotp = ONE;
    }
  }

  /// Computes the projection of vector given by (v1 - v2) along the
  /// current projection vector.
  #[inline(always)]
  pub fn project(&self, v1: Point<F26Dot6>, v2: Point<F26Dot6>) -> F26Dot6 {
    match self.proj_axis {
      // <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L2431>
      CoordAxis::X => v1.x - v2.x,
      // <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L2461>
      CoordAxis::Y => v1.y - v2.y,
      CoordAxis::Both => {
        // <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L2373>
        let dx = v1.x - v2.x;
        let dy = v1.y - v2.y;
        F26Dot6::from_bits(dot14(
          dx.to_bits(),
          dy.to_bits(),
          self.proj_vector.x,
          self.proj_vector.y,
        ))
      }
    }
  }

  /// Computes the projection of vector given by (v1 - v2) along the
  /// current dual projection vector.
  #[inline(always)]
  pub fn dual_project(&self, v1: Point<F26Dot6>, v2: Point<F26Dot6>) -> F26Dot6 {
    match self.dual_proj_axis {
      // <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L2431>
      CoordAxis::X => v1.x - v2.x,
      // <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L2461>
      CoordAxis::Y => v1.y - v2.y,
      CoordAxis::Both => {
        // https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L2402
        let dx = v1.x - v2.x;
        let dy = v1.y - v2.y;
        F26Dot6::from_bits(dot14(
          dx.to_bits(),
          dy.to_bits(),
          self.dual_proj_vector.x,
          self.dual_proj_vector.y,
        ))
      }
    }
  }

  /// Computes the projection of vector given by (v1 - v2) along the
  /// current dual projection vector for unscaled points.
  #[inline(always)]
  pub fn dual_project_unscaled(&self, v1: Point<i32>, v2: Point<i32>) -> i32 {
    match self.dual_proj_axis {
      // <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L2431>
      CoordAxis::X => v1.x - v2.x,
      // <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L2461>
      CoordAxis::Y => v1.y - v2.y,
      CoordAxis::Both => {
        // https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L2402
        let dx = v1.x - v2.x;
        let dy = v1.y - v2.y;
        dot14(dx, dy, self.dual_proj_vector.x, self.dual_proj_vector.y)
      }
    }
  }
}

/// Dot product for vectors in 2.14 fixed point.
fn dot14(ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
  let mut v1 = ax as i64 * bx as i64;
  let v2 = ay as i64 * by as i64;
  v1 += v2;
  v1 += 0x2000 + (v1 >> 63);
  (v1 >> 14) as i32
}
