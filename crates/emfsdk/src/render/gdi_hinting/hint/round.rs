// Adapted from skrifa 0.46.2; see render/gdi_hinting/{LICENSE-MIT,README.md}.
//! Rounding state.

use super::{super::F26Dot6, graphics::GraphicsState};

/// Rounding strategies supported by the interpreter.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub enum RoundMode {
  /// Distances are rounded to the closest grid line.
  ///
  /// Set by `RTG` instruction.
  #[default]
  Grid,
  /// Distances are rounded to the nearest half grid line.
  ///
  /// Set by `RTHG` instruction.
  HalfGrid,
  /// Distances are rounded to the closest half or integer pixel.
  ///
  /// Set by `RTDG` instruction.
  DoubleGrid,
  /// Distances are rounded down to the closest integer grid line.
  ///
  /// Set by `RDTG` instruction.
  DownToGrid,
  /// Distances are rounded up to the closest integer pixel boundary.
  ///
  /// Set by `RUTG` instruction.
  UpToGrid,
  /// Rounding is turned off.
  ///
  /// Set by `ROFF` instruction.
  Off,
  /// Allows fine control over the effects of the round state variable by
  /// allowing you to set the values of three components of the round_state:
  /// period, phase, and threshold.
  ///
  /// More formally, maps the domain of 26.6 fixed point numbers into a set
  /// of discrete values that are separated by equal distances.
  ///
  /// Set by `SROUND` instruction.
  Super,
  /// Analogous to `Super`. The grid period is sqrt(2)/2 pixels rather than 1
  /// pixel. It is useful for measuring at a 45 degree angle with the
  /// coordinate axes.
  ///
  /// Set by `S45ROUND` instruction.
  Super45,
}

/// Graphics state that controls rounding.
///
/// See <https://developer.apple.com/fonts/TrueType-Reference-Manual/RM04/Chap4.html#round%20state>
#[derive(Copy, Clone, Debug)]
pub struct RoundState {
  pub mode: RoundMode,
  pub threshold: i32,
  pub phase: i32,
  pub period: i32,
}

impl Default for RoundState {
  fn default() -> Self {
    Self {
      mode: RoundMode::Grid,
      threshold: 0,
      phase: 0,
      period: 64,
    }
  }
}

impl RoundState {
  pub fn round(&self, distance: F26Dot6) -> F26Dot6 {
    use super::math;
    use RoundMode::*;
    let distance = distance.to_bits();
    let round_bias = self.threshold.wrapping_sub(self.phase);
    let neg_distance = distance.wrapping_neg();
    let result = match self.mode {
      // <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L1958>
      HalfGrid => {
        if distance >= 0 {
          math::floor(distance).wrapping_add(32).max(0)
        } else {
          (math::floor(neg_distance).wrapping_add(32).wrapping_neg()).min(0)
        }
      }
      // <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L1913>
      Grid => {
        if distance >= 0 {
          math::round(distance).max(0)
        } else {
          (math::round(neg_distance).wrapping_neg()).min(0)
        }
      }
      // <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L2094>
      DoubleGrid => {
        if distance >= 0 {
          math::round_pad(distance, 32).max(0)
        } else {
          (math::round_pad(neg_distance, 32).wrapping_neg()).min(0)
        }
      }
      // <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L2005>
      DownToGrid => {
        if distance >= 0 {
          math::floor(distance).max(0)
        } else {
          (math::floor(neg_distance).wrapping_neg()).min(0)
        }
      }
      // <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L2049>
      UpToGrid => {
        if distance >= 0 {
          math::ceil(distance).max(0)
        } else {
          (math::ceil(neg_distance).wrapping_neg()).min(0)
        }
      }
      // <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L2145>
      Super => {
        if distance >= 0 {
          let val = (distance.wrapping_add(round_bias) & self.period.wrapping_neg())
            .wrapping_add(self.phase);
          if val < 0 { self.phase } else { val }
        } else {
          let val = ((round_bias.wrapping_sub(distance)) & self.period.wrapping_neg())
            .wrapping_neg()
            .wrapping_sub(self.phase);
          if val > 0 {
            self.phase.wrapping_neg()
          } else {
            val
          }
        }
      }
      // <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L2199>
      Super45 => {
        if distance >= 0 {
          let val = distance
            .wrapping_add(round_bias)
            .wrapping_div(self.period)
            .wrapping_mul(self.period)
            .wrapping_add(self.phase);
          if val < 0 { self.phase } else { val }
        } else {
          let val = round_bias
            .wrapping_sub(distance)
            .wrapping_div(self.period)
            .wrapping_mul(self.period)
            .wrapping_neg()
            .wrapping_sub(self.phase);
          if val > 0 {
            self.phase.wrapping_neg()
          } else {
            val
          }
        }
      }
      // <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/truetype/ttinterp.c#L1870>
      Off => distance,
    };
    F26Dot6::from_bits(result)
  }
}

impl GraphicsState<'_> {
  pub fn virtual_x_grid(&self) -> bool {
    // GDI uses the fine grid for every projection with an X component,
    // including diagonal projections and a non-horizontal freedom vector.
    // Only a purely vertical projection retains the physical pixel grid.
    self.target.gdi && self.glyph_program && self.proj_vector.x != 0
  }
  pub fn gdi_cutin(&self) -> F26Dot6 {
    if self.virtual_x_grid() {
      F26Dot6::from_bits(self.control_value_cutin.to_bits() / 16)
    } else {
      self.control_value_cutin
    }
  }
  pub fn gdi_minimum(&self) -> F26Dot6 {
    if self.virtual_x_grid() {
      F26Dot6::from_bits(self.min_distance.to_bits() / 2)
    } else {
      self.min_distance
    }
  }

  pub fn round(&self, distance: F26Dot6) -> F26Dot6 {
    if self.virtual_x_grid() {
      distance
        .to_bits()
        .checked_mul(16)
        .map(|value| {
          F26Dot6::from_bits(self.round_state.round(F26Dot6::from_bits(value)).to_bits() / 16)
        })
        .unwrap_or(distance)
    } else {
      self.round_state.round(distance)
    }
  }
}
