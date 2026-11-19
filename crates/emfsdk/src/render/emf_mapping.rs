//! EMF page-space extents. The recording device in EMR_HEADER owns physical
//! units; the destination rectangle is a separate outer playback transform.

use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct Extents {
  pub mode: EmrMapMode,
  pub window: [i32; 2],
  pub viewport: [i32; 2],
}

impl Extents {
  pub fn record(self, data: &[u8], pos: usize, kind: u32, size: usize) -> Self {
    let Some(payload) = data.get(pos + 8..pos + size) else {
      return self;
    };
    self.record_payload(data, kind, payload)
  }

  pub fn record_payload(mut self, header: &[u8], kind: u32, payload: &[u8]) -> Self {
    let value = |offset: usize| read_i32(payload, offset - 8).ok();
    if kind == EMR_SET_MAP_MODE {
      if let Some(mode) = value(8).and_then(|value| EmrMapMode::from_raw(value as u32)) {
        self.set_mode(header, mode);
      }
      return self;
    }
    if !emf_mapping_extents_are_variable(self.mode) {
      return self;
    }
    let scaled = matches!(kind, EMR_SCALE_WINDOW_EXT_EX | EMR_SCALE_VIEWPORT_EXT_EX);
    let window = matches!(kind, EMR_SET_WINDOW_EXT_EX | EMR_SCALE_WINDOW_EXT_EX);
    let old = if window { self.window } else { self.viewport };
    let next = if scaled {
      let scale = |axis: usize| {
        let numerator = value(8 + axis * 8)?;
        let denominator = value(12 + axis * 8)?;
        if numerator == 0 || denominator == 0 {
          return None;
        }
        scale_emf_extent(old[axis], numerator, denominator)
      };
      scale(0).zip(scale(1)).map(|(x, y)| [x, y])
    } else {
      value(8).zip(value(12)).map(|(x, y)| [x, y])
    };
    // SetWindow/ViewportExtEx rejects zero dimensions as one operation.
    if let Some(next) = next.filter(|next| next.iter().all(|v| *v != 0)) {
      if window {
        self.window = next;
      } else {
        self.viewport = next;
      }
      if self.mode == EmrMapMode::Isotropic {
        self.fix_isotropic(header);
      }
    }
    self
  }

  fn set_mode(&mut self, data: &[u8], mode: EmrMapMode) {
    // Repeating either variable mapping mode preserves its authored extents.
    if mode == self.mode && emf_mapping_extents_are_variable(mode) {
      return;
    }
    self.mode = mode;
    if mode == EmrMapMode::Text {
      self.window = [1; 2];
      self.viewport = [1; 2];
      return;
    }
    if mode == EmrMapMode::Anisotropic {
      return;
    }
    let Some((device, millimeters)) = reference_device(data) else {
      return;
    };
    let (numerator, denominator) = match mode {
      EmrMapMode::LoMetric | EmrMapMode::Isotropic => (10i64, 1i64),
      EmrMapMode::HiMetric => (100, 1),
      EmrMapMode::LoEnglish => (1000, 254),
      EmrMapMode::HiEnglish => (10000, 254),
      EmrMapMode::Twips => (14400, 254),
      _ => return,
    };
    let extent = |axis| {
      i32::try_from((i64::from(millimeters[axis]) * numerator + denominator / 2) / denominator).ok()
    };
    let Some((x, y)) = extent(0).zip(extent(1)) else {
      return;
    };
    self.window = [x.max(1), y.max(1)];
    self.viewport = [device[0], -device[1]];
  }

  fn fix_isotropic(&mut self, data: &[u8]) {
    let Some((device, millimeters)) = reference_device(data) else {
      return;
    };
    // MS-EMF 2.3.11.19 and native PlayEnhMetaFileRecord: preserve physical
    // aspect ratio by shrinking one viewport axis, including its sign.
    // Wine's EMF_FixIsotropic uses the same nearest-integer adjustment.
    let unit = |axis: usize| {
      (f64::from(self.viewport[axis]) * f64::from(millimeters[axis])
        / (f64::from(device[axis]) * f64::from(self.window[axis])))
      .abs()
    };
    let x = unit(0);
    let y = unit(1);
    if !(x > 0.0 && y > 0.0) {
      return;
    }
    let (axis, factor) = if x > y { (0, y / x) } else { (1, x / y) };
    let sign = self.viewport[axis].signum();
    let adjusted = (f64::from(self.viewport[axis]) * factor + 0.5).floor() as i32;
    self.viewport[axis] = if adjusted == 0 { sign } else { adjusted };
  }
}

fn reference_device(data: &[u8]) -> Option<([i32; 2], [i32; 2])> {
  let device = [
    read_i32(data, EMF_DEVICE_WIDTH_OFFSET).ok()?,
    read_i32(data, EMF_DEVICE_HEIGHT_OFFSET).ok()?,
  ];
  let millimeters = [
    read_i32(data, EMF_MILLIMETERS_WIDTH_OFFSET).ok()?,
    read_i32(data, EMF_MILLIMETERS_HEIGHT_OFFSET).ok()?,
  ];
  device
    .into_iter()
    .chain(millimeters)
    .all(|value| value > 0)
    .then_some((device, millimeters))
}

/// RGN_COPY replaces the selected region. RegionData is in recording-device
/// units (MS-EMF 2.3.2.2), unlike IntersectClipRect's logical coordinates.
/// A nonrectangular region must retain the completeness fallback.
pub(super) fn copied_rectangle(
  value: &crate::emf::EmrExtSelectClipRgn,
) -> Option<Option<crate::types::RectL>> {
  if value.region_mode_kind()? != crate::emf::EmrRegionMode::Copy {
    return None;
  }
  if value.region_data.is_empty() {
    return Some(None);
  }
  let region = value.typed_region_data().ok()?;
  match region.rectangles.as_slice() {
    [] => Some(Some(crate::types::RectL {
      left: 0,
      top: 0,
      right: 0,
      bottom: 0,
    })),
    [rect] if rect.left <= rect.right && rect.top <= rect.bottom => Some(Some(*rect)),
    _ => None,
  }
}

pub(super) fn device_edges(
  rect: crate::types::RectL,
  origin: [f32; 2],
  scale: [f32; 2],
) -> [f32; 4] {
  [
    (rect.left as f32 - origin[0]) * scale[0],
    (rect.top as f32 - origin[1]) * scale[1],
    (rect.right as f32 - origin[0]) * scale[0],
    (rect.bottom as f32 - origin[1]) * scale[1],
  ]
}

#[cfg(test)]
mod tests {
  use super::*;
  fn header() -> Vec<u8> {
    let mut data = vec![0; 88];
    for (offset, value) in [
      (EMF_DEVICE_WIDTH_OFFSET, 1024i32),
      (EMF_DEVICE_HEIGHT_OFFSET, 768),
      (EMF_MILLIMETERS_WIDTH_OFFSET, 320),
      (EMF_MILLIMETERS_HEIGHT_OFFSET, 240),
    ] {
      data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    data
  }
  fn apply(state: Extents, kind: u32, fields: &[i32]) -> Extents {
    let mut data = header();
    let pos = data.len();
    data.extend_from_slice(&kind.to_le_bytes());
    data.extend_from_slice(&(8 + fields.len() as u32 * 4).to_le_bytes());
    for value in fields {
      data.extend_from_slice(&value.to_le_bytes());
    }
    state.record(&data, pos, kind, 8 + fields.len() * 4)
  }
  fn initial() -> Extents {
    Extents {
      mode: EmrMapMode::Text,
      window: [1; 2],
      viewport: [1; 2],
    }
  }
  #[test]
  fn native_isotropic_initialization_and_repeated_mode_preserve_orientation() {
    let mut state = apply(initial(), EMR_SET_MAP_MODE, &[7]);
    assert_eq!(state.window, [3200, 2400]);
    assert_eq!(state.viewport, [1024, -768]);
    state = apply(state, EMR_SET_WINDOW_EXT_EX, &[12471, 12471]);
    assert_eq!(state.viewport, [768, -768]);
    state = apply(state, EMR_SET_MAP_MODE, &[7]);
    assert_eq!(state.window, [12471; 2]);
    assert_eq!(state.viewport, [768, -768]);
    state = apply(state, EMR_SET_MAP_MODE, &[8]);
    assert_eq!(state.viewport, [768, -768]);
    state = apply(state, EMR_SET_VIEWPORT_EXT_EX, &[321, -123]);
    assert_eq!(state.viewport, [321, -123]);
    state = apply(state, EMR_SET_MAP_MODE, &[1]);
    assert_eq!(state.window, [1; 2]);
    assert_eq!(state.viewport, [1; 2]);
  }
  #[test]
  fn native_fixed_modes_and_invalid_extents_keep_their_contract() {
    for (mode, window) in [
      (2, [3200, 2400]),
      (3, [32000, 24000]),
      (4, [1260, 945]),
      (5, [12598, 9449]),
      (6, [18142, 13606]),
    ] {
      let state = apply(initial(), EMR_SET_MAP_MODE, &[mode]);
      assert_eq!(state.window, window);
      assert_eq!(state.viewport, [1024, -768]);
      let ignored = apply(state, EMR_SET_WINDOW_EXT_EX, &[12471, 12471]);
      assert_eq!(ignored.window, window);
    }
    let state = apply(initial(), EMR_SET_MAP_MODE, &[7]);
    let invalid = apply(state, EMR_SET_WINDOW_EXT_EX, &[0, 400]);
    assert_eq!(invalid.window, state.window);
    assert_eq!(invalid.viewport, state.viewport);
    let scaled = apply(state, EMR_SCALE_WINDOW_EXT_EX, &[2, 1, 2, 1]);
    assert_eq!(scaled.window, [6400, 4800]);
    assert_eq!(scaled.viewport, [1024, -768]);
  }
}
