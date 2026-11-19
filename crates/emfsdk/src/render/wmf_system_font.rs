//! Windows raster System font metrics for an explicitly supplied WMF DC.
//!
//! MS-WMF 3.1.5 leaves the initial font to the player. The conversion DC and
//! playback DC can have different stock objects; neither is an authored LOGFONT.
//! VGA and 8514 System FNT character tables supply these ASCII advances. Their
//! complete 95-character ranges agree with GetTextExtentPoint32W at both strikes.
//! Keep device widths until cumulative DPtoLP conversion, including zero Dx.

use super::{WmfTextFont, wmf_logical_advances};
use crate::common::SdkEnumValue;

/// Initial System font in the conversion and playback device contexts.
///
/// This describes the player's environment, not an inferred document font.
/// Supported bitmap strikes are 16 and 20 pixels in the GB2312 environment.
/// The optional profile affects
/// only the initial font; selecting an authored font replaces it, and RestoreDC
/// restores it. Its ASCII device advances do not scale like a TrueType LOGFONT.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WmfSystemFontProfile {
  pub conversion_height_px: u16,
  pub playback_height_px: u16,
  pub char_set: u8,
}

#[derive(Clone, Copy)]
pub(super) struct SystemMetrics {
  pub height: u16,
  pub ascent: u16,
  widths: &'static [u8; 95],
}

impl WmfSystemFontProfile {
  pub(super) fn is_supported(self) -> bool {
    metrics(self.conversion_height_px).is_some()
      && metrics(self.playback_height_px).is_some()
      && self.char_set == 134
  }

  pub(super) fn playback_metrics(self) -> Option<SystemMetrics> {
    metrics(self.playback_height_px)
  }

  pub(super) fn playback_font(self) -> WmfTextFont {
    // A bitmap System font cannot be embedded as a TrueType program. The
    // Windows GB2312 fixed-output mapper uses its SimSun outline substitute;
    // retain the System metrics rather than measuring that substitute's widths.
    WmfTextFont {
      width: 0,
      height: i32::from(self.playback_height_px),
      escapement: 0,
      family: Some("SimSun".to_string()),
      char_set: self.char_set,
      weight: 700,
      italic: false,
      quality: crate::wmf::WmfFontQuality::Proof.raw(),
    }
  }

  pub(super) fn converted_advances(self, text: &str, device_scale: f32) -> Option<Vec<i16>> {
    let metrics = metrics(self.conversion_height_px)?;
    let widths = text
      .chars()
      .map(|c| {
        let index = usize::try_from(u32::from(c).checked_sub(32)?).ok()?;
        Some(f32::from(*metrics.widths.get(index)?))
      })
      .collect::<Option<Vec<_>>>()?;
    wmf_logical_advances(&widths, device_scale.abs())
  }
}

fn metrics(height: u16) -> Option<SystemMetrics> {
  let (ascent, widths) = match height {
    16 => (13, &SYSTEM_16_WIDTHS),
    20 => (16, &SYSTEM_20_WIDTHS),
    _ => return None,
  };
  Some(SystemMetrics {
    height,
    ascent,
    widths,
  })
}

const SYSTEM_16_WIDTHS: [u8; 95] = [
  4, 4, 6, 8, 8, 11, 9, 4, 4, 4, 6, 8, 4, 4, 4, 4, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 4, 4, 8, 8, 8, 8,
  14, 8, 10, 9, 10, 9, 8, 10, 10, 4, 7, 9, 8, 12, 10, 10, 9, 10, 10, 9, 8, 10, 8, 14, 9, 10, 9, 4,
  4, 4, 5, 8, 5, 8, 8, 7, 8, 8, 4, 8, 8, 4, 4, 7, 4, 12, 8, 8, 8, 8, 5, 8, 4, 8, 8, 10, 8, 8, 8, 5,
  4, 5, 5,
];

const SYSTEM_20_WIDTHS: [u8; 95] = [
  4, 5, 8, 9, 9, 13, 11, 4, 5, 5, 9, 9, 4, 5, 4, 4, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 4, 4, 9, 9, 9,
  10, 16, 12, 12, 12, 12, 11, 10, 13, 12, 5, 9, 11, 10, 14, 12, 13, 11, 13, 12, 11, 10, 12, 11, 15,
  11, 11, 10, 5, 4, 5, 5, 9, 5, 10, 10, 9, 10, 10, 6, 10, 10, 4, 4, 9, 4, 14, 10, 10, 10, 10, 7, 9,
  6, 10, 9, 14, 8, 9, 8, 6, 6, 6, 5,
];
