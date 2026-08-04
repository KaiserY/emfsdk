use fontique::{
  Attributes as FontAttributes, Collection as FontCollection,
  CollectionOptions as FontCollectionOptions, FontStyle, FontWeight, FontWidth, GenericFamily,
  QueryFamily, QueryStatus, SourceCache,
};
use image::codecs::png::PngEncoder;
use image::{ColorType, ImageEncoder};
use skrifa::outline::{
  DrawSettings, HintingInstance, HintingOptions, OutlinePen, SmoothMode, Target,
};
use skrifa::prelude::{FontRef, LocationRef, MetadataProvider, Size as FontSize};
use std::collections::HashMap;
use thiserror::Error;
use zeno::{
  Command as ZenoCommand, Format as ZenoMaskFormat, Mask as ZenoMask, Origin as ZenoOrigin,
  Point as ZenoPoint, Scratch as ZenoScratch, Transform as ZenoTransform, Vector as ZenoVector,
};

use crate::bitmap::{
  BitmapCompression, DeviceIndependentBitmap, DibColorTable, DibColorUsage, DibHeader,
};
use crate::common::{Reader, SdkEnumValue};
use crate::emf::EmrPenLineStyle;
use crate::emfplus::{
  EmfPlusBitmapPayload, EmfPlusBrushData, EmfPlusBrushRef, EmfPlusDrawArcData,
  EmfPlusDrawImageData, EmfPlusDrawImagePointsData, EmfPlusDrawPointsData,
  EmfPlusDrawRectShapeData, EmfPlusDrawStringData, EmfPlusFillPieData, EmfPlusFillRectShapeData,
  EmfPlusFontObject, EmfPlusHatchStyle, EmfPlusImageData, EmfPlusImageObject,
  EmfPlusObjectAssembler, EmfPlusObjectData, EmfPlusObjectRecordData, EmfPlusPathObject,
  EmfPlusPathPointType, EmfPlusPathPointTypeFlags, EmfPlusPathPointTypeValue,
  EmfPlusPathPointTypes, EmfPlusPenObject, EmfPlusPointData, EmfPlusRecord, EmfPlusRecordData,
  EmfPlusRecordType, EmfPlusRotateWorldTransformData, EmfPlusScaleWorldTransformData,
  EmfPlusTranslateWorldTransformData, EmfPlusUnitType,
};
use crate::wmf::{
  WmfBinaryRasterOperation, WmfBrushStyle, WmfEscapeData, WmfExtTextOutOptions, WmfMetafileRef,
  WmfPenLineStyle, WmfRecordData, WmfTernaryRasterOperationCode, WmfTextAlignmentModeFlags,
};

// record ids. The byte offsets below are record-relative, including the
// 8-byte EMR header, as specified by [MS-EMF].
const EMF_HEADER_SIZE: usize = 108;
const EMF_RECORD_HEADER_SIZE: usize = 8;
const EMF_BOUNDS_LEFT_OFFSET: usize = 8;
const EMF_BOUNDS_TOP_OFFSET: usize = 12;
const EMF_BOUNDS_RIGHT_OFFSET: usize = 16;
const EMF_BOUNDS_BOTTOM_OFFSET: usize = 20;
const EMF_FRAME_LEFT_OFFSET: usize = 24;
const EMF_FRAME_TOP_OFFSET: usize = 28;
const EMF_FRAME_RIGHT_OFFSET: usize = 32;
const EMF_FRAME_BOTTOM_OFFSET: usize = 36;
const EMF_DEVICE_WIDTH_OFFSET: usize = 72;
const EMF_DEVICE_HEIGHT_OFFSET: usize = 76;
const EMF_MILLIMETERS_WIDTH_OFFSET: usize = 80;
const EMF_MILLIMETERS_HEIGHT_OFFSET: usize = 84;
const EMR_EOF: u32 = 14;
const EMR_POLYBEZIER: u32 = 2;
const EMR_POLYGON: u32 = 3;
const EMR_POLYLINE: u32 = 4;
const EMR_POLYBEZIER_TO: u32 = 5;
const EMR_POLYLINE_TO: u32 = 6;
const EMR_POLYPOLYLINE: u32 = 7;
const EMR_POLYPOLYGON: u32 = 8;
const EMR_SET_WINDOW_EXT_EX: u32 = 9;
const EMR_SET_WINDOW_ORG_EX: u32 = 10;
const EMR_SET_VIEWPORT_EXT_EX: u32 = 11;
const EMR_SET_VIEWPORT_ORG_EX: u32 = 12;
const EMR_SET_PIXEL_V: u32 = 15;
const EMR_SET_ROP_2: u32 = 20;
const EMR_SET_TEXT_ALIGN: u32 = 22;
const EMR_SET_TEXT_COLOR: u32 = 24;
const EMR_MOVE_TO_EX: u32 = 27;
const EMR_SAVE_DC: u32 = 33;
const EMR_RESTORE_DC: u32 = 34;
const EMR_SET_WORLD_TRANSFORM: u32 = 35;
const EMR_MODIFY_WORLD_TRANSFORM: u32 = 36;
const EMR_SELECT_OBJECT: u32 = 37;
const EMR_CREATE_PEN: u32 = 38;
const EMR_CREATE_BRUSH_INDIRECT: u32 = 39;
const EMR_DELETE_OBJECT: u32 = 40;
const EMR_ELLIPSE: u32 = 42;
const EMR_RECTANGLE: u32 = 43;
const EMR_ROUND_RECT: u32 = 44;
const EMR_ARC: u32 = 45;
const EMR_CHORD: u32 = 46;
const EMR_PIE: u32 = 47;
const EMR_LINE_TO: u32 = 54;
const EMR_BIT_BLT: u32 = 76;
const EMR_STRETCH_BLT: u32 = 77;
const EMR_SET_DIBITS_TO_DEVICE: u32 = 80;
const EMR_STRETCH_DIBITS: u32 = 81;
const EMR_EXT_CREATE_FONT_INDIRECT_W: u32 = 82;
const EMR_EXT_TEXTOUT_A: u32 = 83;
const EMR_EXT_TEXTOUT_W: u32 = 84;
const EMR_POLYBEZIER16: u32 = 85;
const EMR_POLYGON16: u32 = 86;
const EMR_POLYLINE16: u32 = 87;
const EMR_POLYBEZIER_TO16: u32 = 88;
const EMR_POLYLINE_TO16: u32 = 89;
const EMR_POLYPOLYLINE16: u32 = 90;
const EMR_POLYPOLYGON16: u32 = 91;
const EMR_EXT_CREATE_PEN: u32 = 95;
const EMR_BITMAP_DEST_X_OFFSET: usize = 24;
const EMR_BITMAP_DEST_Y_OFFSET: usize = 28;
const EMR_BITMAP_SOURCE_WIDTH_OFFSET: usize = 40;
const EMR_BITMAP_SOURCE_HEIGHT_OFFSET: usize = 44;
const EMR_BITMAP_INFO_OFFSET_OFFSET: usize = 48;
const EMR_BITMAP_INFO_SIZE_OFFSET: usize = 52;
const EMR_BITMAP_BITS_OFFSET_OFFSET: usize = 56;
const EMR_BITMAP_BITS_SIZE_OFFSET: usize = 60;
const EMR_BITMAP_COLOR_USAGE_OFFSET: usize = 64;
const EMR_STRETCH_DIBITS_ROP_OFFSET: usize = 68;
const EMR_STRETCH_DIBITS_DEST_WIDTH_OFFSET: usize = 72;
const EMR_STRETCH_DIBITS_DEST_HEIGHT_OFFSET: usize = 76;
const EMR_BLT_DEST_WIDTH_OFFSET: usize = 32;
const EMR_BLT_DEST_HEIGHT_OFFSET: usize = 36;
const EMR_BLT_ROP_OFFSET: usize = 40;
const EMR_BLT_SOURCE_X_OFFSET: usize = 44;
const EMR_BLT_SOURCE_Y_OFFSET: usize = 48;
const EMR_BLT_COLOR_USAGE_OFFSET: usize = 80;
const EMR_BLT_INFO_OFFSET_OFFSET: usize = 84;
const EMR_BLT_INFO_SIZE_OFFSET: usize = 88;
const EMR_BLT_BITS_OFFSET_OFFSET: usize = 92;
const EMR_BLT_BITS_SIZE_OFFSET: usize = 96;
const EMR_STRETCH_BLT_SOURCE_WIDTH_OFFSET: usize = 100;
const EMR_STRETCH_BLT_SOURCE_HEIGHT_OFFSET: usize = 104;
const ENHMETA_STOCK_OBJECT: u32 = 0x8000_0000;
const WHITE_BRUSH: u32 = ENHMETA_STOCK_OBJECT;
const BLACK_BRUSH: u32 = ENHMETA_STOCK_OBJECT | 4;
const NULL_BRUSH: u32 = ENHMETA_STOCK_OBJECT | 5;
const WHITE_PEN: u32 = ENHMETA_STOCK_OBJECT | 6;
const BLACK_PEN: u32 = ENHMETA_STOCK_OBJECT | 7;
const NULL_PEN: u32 = ENHMETA_STOCK_OBJECT | 8;
const MWT_IDENTITY: u32 = 1;
const MWT_LEFTMULTIPLY: u32 = 2;
const MWT_RIGHTMULTIPLY: u32 = 3;
const MWT_SET: u32 = 4;
const EMR_COMMENT: u32 = 70;
const EMR_COMMENT_EMFPLUS: u32 = 0x2B46_4D45;
const LOGFONT_FACE_NAME_CHARS: usize = 32;
// values and keeps DIB scanlines aligned to four bytes.
const RGB_BYTES_PER_PIXEL: usize = 3;
const BGRA_BYTES_PER_PIXEL: usize = 4;
#[cfg(test)]
const BI_RGB: u32 = 0;
#[cfg(test)]
const BI_PNG: u32 = 5;
const DEFAULT_RENDER_WIDTH: usize = 1024;
const DEFAULT_RENDER_HEIGHT: usize = 768;
const DEFAULT_MAX_PIXELS: usize = 16_000_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedMetafile {
  pub data: Vec<u8>,
  pub content_type: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MetafilePhysicalSize {
  pub width_pt: f32,
  pub height_pt: f32,
  pub natural_width_px: u32,
  pub natural_height_px: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderOptions {
  pub target_width_px: Option<u32>,
  pub target_height_px: Option<u32>,
  pub max_pixels: Option<u32>,
  /// Preserve an unpainted destination as transparent output.
  ///
  /// GDI raster operations still require concrete destination samples. The
  /// renderer therefore replays against black and white destinations and
  /// reconstructs straight color plus coverage from the two results. This is
  /// the same black/white-background technique used by metafile consumers
  /// when a host application composites the preview over its own fill.
  pub transparent_background: bool,
  /// Existing destination surface color used when replaying raster operations.
  ///
  /// Metafiles embedded in a filled host shape paint onto that shape instead
  /// of an implicitly white page. Callers that do not supply a destination
  /// retain the standalone white-canvas behavior.
  pub background_color: Option<[u8; 3]>,
  /// Caller-specific realization palette for one-bit DIB pattern brushes.
  ///
  /// This is intentionally opt-in: color-output GDI playback otherwise
  /// preserves the DIB's embedded color table.
  pub monochrome_dib_palette_override: Option<[[u8; 3]; 2]>,
  /// Box-filter one-pixel checkerboard pattern brushes before a fixed output
  /// rescales the raster. This matches filtered GDI+/Cairo image playback and
  /// prevents phase-biased moire in PDF consumers.
  pub filter_high_frequency_pattern_brushes: bool,
}

impl RenderOptions {
  fn resolved_canvas_size(self, natural_width: usize, natural_height: usize) -> (usize, usize) {
    let natural_width = natural_width.max(1);
    let natural_height = natural_height.max(1);
    // [MS-WMF] 3.1.3 assigns the window to the metafile and the viewport to
    // the player. A requested output size is therefore the playback viewport,
    // even when it is larger than the metafile's logical extent.
    let resolve_axis =
      |target: Option<u32>, natural: usize| target.map_or(natural, |value| value.max(1) as usize);
    let width = resolve_axis(self.target_width_px, natural_width);
    let height = resolve_axis(self.target_height_px, natural_height);
    clamp_canvas_size(width, height, self.max_pixels)
  }
}

#[derive(Debug, Error)]
pub enum RenderError {
  #[error("{0}")]
  Invalid(String),
}

impl From<String> for RenderError {
  fn from(value: String) -> Self {
    Self::Invalid(value)
  }
}

pub type RenderResult<T> = std::result::Result<T, RenderError>;

pub fn decode_metafile_as_raster(
  data: &[u8],
  content_type: Option<&str>,
) -> RenderResult<Option<DecodedMetafile>> {
  decode_metafile_as_raster_with_options(data, content_type, RenderOptions::default())
}

pub fn decode_metafile_as_raster_with_options(
  data: &[u8],
  content_type: Option<&str>,
  options: RenderOptions,
) -> RenderResult<Option<DecodedMetafile>> {
  if !looks_like_metafile(data, content_type) {
    return Ok(None);
  }

  if options.transparent_background {
    return decode_transparent_metafile_as_raster(data, content_type, options).map_err(Into::into);
  }

  decode_opaque_metafile_as_raster(data, content_type, options, false).map_err(Into::into)
}

/// Returns the physical playback frame recorded by an EMF header.
///
/// `[MS-EMF]` defines `Frame` in 0.01 millimeter units. The natural pixel
/// dimensions are recovered from the same frame plus the reference
/// `Device`/`Millimeters` fields, matching raster playback.
pub fn metafile_physical_size(
  data: &[u8],
  content_type: Option<&str>,
) -> Option<MetafilePhysicalSize> {
  if !looks_like_metafile(data, content_type) || !is_emf(data) {
    return None;
  }
  emf_physical_size(data)
}

fn decode_opaque_metafile_as_raster(
  data: &[u8],
  _content_type: Option<&str>,
  options: RenderOptions,
  force_vector_replay: bool,
) -> Result<Option<DecodedMetafile>, String> {
  if let Some(raster) = decode_emf_as_raster(data, options, force_vector_replay)? {
    return Ok(Some(raster));
  }

  if let Some(raster) = decode_wmf_as_raster(data, options)? {
    return Ok(Some(raster));
  }

  Ok(None)
}

fn decode_transparent_metafile_as_raster(
  data: &[u8],
  content_type: Option<&str>,
  options: RenderOptions,
) -> Result<Option<DecodedMetafile>, String> {
  let mut black_options = options;
  black_options.transparent_background = false;
  black_options.background_color = Some([0; 3]);
  let mut white_options = options;
  white_options.transparent_background = false;
  white_options.background_color = Some([255; 3]);

  let Some(black) = decode_opaque_metafile_as_raster(data, content_type, black_options, true)?
  else {
    return Ok(None);
  };
  let white = decode_opaque_metafile_as_raster(data, content_type, white_options, true)?
    .ok_or_else(|| "metafile white-background replay produced no raster".to_string())?;
  let black = decoded_png_to_rgb(&black)?;
  let white = decoded_png_to_rgb(&white)?;
  if black.width != white.width || black.height != white.height {
    return Err("metafile black/white replays have different dimensions".to_string());
  }

  let rgba = straight_rgba_from_black_white(&black.rgb, &white.rgb)?;
  Ok(Some(DecodedMetafile {
    data: rgba_to_png(&rgba, black.width as u32, black.height as u32)?,
    content_type: "image/png",
  }))
}

#[derive(Clone, Debug)]
pub struct MetafileTextRun {
  pub text: String,
  pub x: f32,
  pub y: f32,
  pub font_size: Option<f32>,
  pub font_family: Option<String>,
  pub bold: bool,
  pub italic: bool,
  pub width: Option<f32>,
}

pub fn extract_metafile_text_runs(data: &[u8], content_type: Option<&str>) -> Vec<MetafileTextRun> {
  if !looks_like_metafile(data, content_type) {
    return Vec::new();
  }
  if is_emf(data) && data.len() >= EMF_HEADER_SIZE {
    return extract_emf_text_runs(data);
  }
  if crate::wmf::looks_like_wmf(data) {
    return extract_wmf_text_runs(data);
  }
  Vec::new()
}

fn extract_emf_text_runs(data: &[u8]) -> Vec<MetafileTextRun> {
  let mut state = match EmfTextState::new(data) {
    Ok(state) => state,
    Err(_) => return Vec::new(),
  };
  let mut runs = Vec::new();
  let mut pos = EMF_HEADER_SIZE;
  while pos + EMF_RECORD_HEADER_SIZE <= data.len() {
    let Ok(record_type) = read_u32(data, pos) else {
      break;
    };
    let Ok(record_size) = read_u32(data, pos + 4) else {
      break;
    };
    let record_size = record_size as usize;
    if record_size < EMF_RECORD_HEADER_SIZE || pos + record_size > data.len() {
      break;
    }

    match record_type {
      EMR_SET_WINDOW_ORG_EX if record_size >= 16 => {
        state.window_org_x = read_i32(data, pos + 8).unwrap_or(state.window_org_x);
        state.window_org_y = read_i32(data, pos + 12).unwrap_or(state.window_org_y);
      }
      EMR_SET_WINDOW_EXT_EX if record_size >= 16 => {
        state.window_ext_x = read_i32(data, pos + 8)
          .unwrap_or(state.window_ext_x)
          .abs()
          .max(1);
        state.window_ext_y = read_i32(data, pos + 12)
          .unwrap_or(state.window_ext_y)
          .abs()
          .max(1);
      }
      EMR_SET_VIEWPORT_ORG_EX if record_size >= 16 => {
        state.viewport_org_x = read_i32(data, pos + 8).unwrap_or(state.viewport_org_x);
        state.viewport_org_y = read_i32(data, pos + 12).unwrap_or(state.viewport_org_y);
      }
      EMR_SET_VIEWPORT_EXT_EX if record_size >= 16 => {
        state.viewport_ext_x = read_i32(data, pos + 8).unwrap_or(state.viewport_ext_x);
        state.viewport_ext_y = read_i32(data, pos + 12).unwrap_or(state.viewport_ext_y);
      }
      EMR_SET_TEXT_ALIGN if record_size >= 12 => {
        state.text_alignment = WmfTextAlignmentModeFlags::from_bits_retain(
          read_u32(data, pos + 8).unwrap_or_default() as u16,
        );
      }
      EMR_SET_WORLD_TRANSFORM if record_size >= 32 => {
        if let Ok(transform) = read_xform(data, pos + 8) {
          state.world_transform = transform;
        }
      }
      EMR_MODIFY_WORLD_TRANSFORM if record_size >= 36 => {
        if let (Ok(transform), Ok(mode)) = (read_xform(data, pos + 8), read_u32(data, pos + 32)) {
          state.world_transform = match mode {
            MWT_IDENTITY => EmfTransform::identity(),
            MWT_LEFTMULTIPLY => transform.multiply(state.world_transform),
            MWT_RIGHTMULTIPLY => state.world_transform.multiply(transform),
            MWT_SET => transform,
            _ => state.world_transform,
          };
        }
      }
      EMR_EXT_CREATE_FONT_INDIRECT_W if record_size >= 104 => {
        if let Some((object_id, font)) = read_logfont_object(data, pos, record_size)
          && object_id & ENHMETA_STOCK_OBJECT == 0
        {
          state.fonts.insert(object_id, font);
        }
      }
      EMR_SELECT_OBJECT if record_size >= 12 => {
        let object_id = read_u32(data, pos + 8).unwrap_or(0);
        if state.fonts.contains_key(&object_id) {
          state.current_font = Some(object_id);
        }
      }
      EMR_DELETE_OBJECT if record_size >= 12 => {
        let object_id = read_u32(data, pos + 8).unwrap_or(0);
        state.fonts.remove(&object_id);
        if state.current_font == Some(object_id) {
          state.current_font = None;
        }
      }
      EMR_EXT_TEXTOUT_W => {
        if let Some(text) = extract_semantic_emr_ext_text_out_w(data, pos, record_size)
          && !text.trim().is_empty()
          && let Some(run) = state.text_run(data, pos, record_size, text)
        {
          runs.push(run);
        }
      }
      EMR_EXT_TEXTOUT_A => {
        if let Some(text) = extract_emr_ext_text_out_a(data, pos, record_size)
          && !text.trim().is_empty()
          && let Some(run) = state.text_run(data, pos, record_size, text)
        {
          runs.push(run);
        }
      }
      EMR_EOF => break,
      _ => {}
    }

    pos += record_size;
  }

  runs
}

#[derive(Clone)]
struct WmfTextSnapshot {
  window_org_x: i32,
  window_org_y: i32,
  window_ext_x: i32,
  window_ext_y: i32,
  viewport_org_x: i32,
  viewport_org_y: i32,
  viewport_ext_x: i32,
  viewport_ext_y: i32,
  current_font_height: i32,
  current_font_family: Option<String>,
  current_font_bold: bool,
  current_font_italic: bool,
  text_alignment: WmfTextAlignmentModeFlags,
}

#[derive(Clone, Debug)]
struct WmfTextFont {
  height: i32,
  family: Option<String>,
  weight: u16,
  italic: bool,
}

struct WmfTextState {
  natural_width: f32,
  natural_height: f32,
  window_org_x: i32,
  window_org_y: i32,
  window_ext_x: i32,
  window_ext_y: i32,
  viewport_org_x: i32,
  viewport_org_y: i32,
  viewport_ext_x: i32,
  viewport_ext_y: i32,
  objects: Vec<Option<WmfTextFont>>,
  current_font_height: i32,
  current_font_family: Option<String>,
  current_font_bold: bool,
  current_font_italic: bool,
  text_alignment: WmfTextAlignmentModeFlags,
  saved: Vec<WmfTextSnapshot>,
}

impl WmfTextState {
  fn new(metafile: &WmfMetafileRef<'_>) -> Self {
    let (window_org_x, window_org_y, window_ext_x, window_ext_y) = wmf_initial_window(metafile);
    Self {
      natural_width: window_ext_x.unsigned_abs().max(1) as f32,
      natural_height: window_ext_y.unsigned_abs().max(1) as f32,
      window_org_x,
      window_org_y,
      window_ext_x: nonzero_mapping_extent(window_ext_x),
      window_ext_y: nonzero_mapping_extent(window_ext_y),
      viewport_org_x: 0,
      viewport_org_y: 0,
      viewport_ext_x: mapping_extent_magnitude(window_ext_x),
      viewport_ext_y: mapping_extent_magnitude(window_ext_y),
      objects: vec![None; metafile.header.number_of_objects as usize],
      current_font_height: 12,
      current_font_family: None,
      current_font_bold: false,
      current_font_italic: false,
      text_alignment: WmfTextAlignmentModeFlags::empty(),
      saved: Vec::new(),
    }
  }

  fn insert_object(&mut self, font: Option<WmfTextFont>) {
    let object = font.unwrap_or(WmfTextFont {
      height: 0,
      family: None,
      weight: 400,
      italic: false,
    });
    if let Some(slot) = self.objects.iter_mut().find(|slot| slot.is_none()) {
      *slot = Some(object);
    } else {
      self.objects.push(Some(object));
    }
  }

  fn select_object(&mut self, index: u16) {
    if let Some(Some(font)) = self.objects.get(index as usize)
      && font.height != 0
    {
      self.current_font_height = font.height.abs().max(7);
      self.current_font_family = font.family.clone();
      self.current_font_bold = font.weight > 400;
      self.current_font_italic = font.italic;
    }
  }

  fn save(&mut self) {
    self.saved.push(WmfTextSnapshot {
      window_org_x: self.window_org_x,
      window_org_y: self.window_org_y,
      window_ext_x: self.window_ext_x,
      window_ext_y: self.window_ext_y,
      viewport_org_x: self.viewport_org_x,
      viewport_org_y: self.viewport_org_y,
      viewport_ext_x: self.viewport_ext_x,
      viewport_ext_y: self.viewport_ext_y,
      current_font_height: self.current_font_height,
      current_font_family: self.current_font_family.clone(),
      current_font_bold: self.current_font_bold,
      current_font_italic: self.current_font_italic,
      text_alignment: self.text_alignment,
    });
  }

  fn restore(&mut self) {
    let Some(snapshot) = self.saved.pop() else {
      return;
    };
    self.window_org_x = snapshot.window_org_x;
    self.window_org_y = snapshot.window_org_y;
    self.window_ext_x = snapshot.window_ext_x;
    self.window_ext_y = snapshot.window_ext_y;
    self.viewport_org_x = snapshot.viewport_org_x;
    self.viewport_org_y = snapshot.viewport_org_y;
    self.viewport_ext_x = snapshot.viewport_ext_x;
    self.viewport_ext_y = snapshot.viewport_ext_y;
    self.current_font_height = snapshot.current_font_height;
    self.current_font_family = snapshot.current_font_family;
    self.current_font_bold = snapshot.current_font_bold;
    self.current_font_italic = snapshot.current_font_italic;
    self.text_alignment = snapshot.text_alignment;
  }

  fn scale_window(&mut self, value: crate::wmf::WmfScaleExtRecord) {
    self.window_ext_x = scale_wmf_extent(self.window_ext_x, value.x_num, value.x_denom);
    self.window_ext_y = scale_wmf_extent(self.window_ext_y, value.y_num, value.y_denom);
  }

  fn scale_viewport(&mut self, value: crate::wmf::WmfScaleExtRecord) {
    self.viewport_ext_x = scale_wmf_extent(self.viewport_ext_x, value.x_num, value.x_denom);
    self.viewport_ext_y = scale_wmf_extent(self.viewport_ext_y, value.y_num, value.y_denom);
  }

  fn text_run(
    &self,
    text: String,
    x: i16,
    y: i16,
    logical_width: Option<f32>,
  ) -> Option<MetafileTextRun> {
    if text.is_empty() {
      return None;
    }
    let scale_x = self.viewport_ext_x as f32 / self.window_ext_x as f32;
    let scale_y = self.viewport_ext_y as f32 / self.window_ext_y as f32;
    let mapped_x = self.viewport_org_x as f32 + (i32::from(x) - self.window_org_x) as f32 * scale_x;
    let baseline_y = if self
      .text_alignment
      .intersects(WmfTextAlignmentModeFlags::BASELINE | WmfTextAlignmentModeFlags::BOTTOM)
    {
      f32::from(y)
    } else {
      // TA_TOP aligns the reference point to the top of the character
      // cell. Without an installed-face dependency here, LOGFONT height is
      // the bounded GDI-compatible baseline advance.
      f32::from(y) + self.current_font_height.abs() as f32
    };
    let mapped_y = self.viewport_org_y as f32 + (baseline_y - self.window_org_y as f32) * scale_y;
    Some(MetafileTextRun {
      text,
      x: mapped_x / self.natural_width,
      y: mapped_y / self.natural_height,
      font_size: Some(self.current_font_height.abs() as f32 * scale_y.abs() / self.natural_height),
      font_family: self.current_font_family.clone(),
      bold: self.current_font_bold,
      italic: self.current_font_italic,
      width: logical_width
        .map(|width| width * scale_x.abs() / self.natural_width)
        .filter(|width| width.is_finite() && *width > 0.0),
    })
  }
}

fn scale_wmf_extent(extent: i32, numerator: i16, denominator: i16) -> i32 {
  if denominator == 0 {
    return extent;
  }
  ((i64::from(extent) * i64::from(numerator)) / i64::from(denominator))
    .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn nonzero_mapping_extent(extent: i32) -> i32 {
  // [MS-WMF] §§2.3.5.28/30 store viewport and window extents as signed
  // integers. Their sign participates in VExt/WExt and can reverse an axis;
  // it is not a canvas-size magnitude.
  if extent == 0 { 1 } else { extent }
}

fn mapping_extent_magnitude(extent: i32) -> i32 {
  extent.saturating_abs().max(1)
}

fn wmf_text_font(value: &crate::wmf::WmfFontObject) -> WmfTextFont {
  let face_name = &value.face_name[..usize::from(value.face_name_bytes)];
  let face_name = &face_name[..face_name
    .iter()
    .position(|byte| *byte == 0)
    .unwrap_or(face_name.len())];
  let family = crate::string::SdkEncoding::WmfCharset(value.char_set)
    .decode(face_name)
    .or_else(|_| crate::string::SdkEncoding::Windows1252.decode(face_name))
    .ok()
    .map(|family| family.trim().to_string())
    .filter(|family| !family.is_empty());
  WmfTextFont {
    height: i32::from(value.height),
    family,
    weight: value.weight.max(0) as u16,
    italic: value.italic != 0,
  }
}

fn extract_wmf_text_runs(data: &[u8]) -> Vec<MetafileTextRun> {
  let Ok(metafile) = WmfMetafileRef::from_bytes(data) else {
    return Vec::new();
  };
  let mut state = WmfTextState::new(&metafile);
  let mut runs = Vec::new();
  for record in metafile.records() {
    let Ok(record) = record.parse_data() else {
      continue;
    };
    match record {
      WmfRecordData::Eof(_) => break,
      WmfRecordData::SaveDc => state.save(),
      WmfRecordData::RestoreDc(_) => state.restore(),
      WmfRecordData::SetWindowOrg(value) => {
        state.window_org_x = i32::from(value.x);
        state.window_org_y = i32::from(value.y);
      }
      WmfRecordData::SetWindowExt(value) => {
        state.window_ext_x = nonzero_mapping_extent(i32::from(value.x));
        state.window_ext_y = nonzero_mapping_extent(i32::from(value.y));
      }
      WmfRecordData::SetViewportOrg(value) => {
        state.viewport_org_x = i32::from(value.x);
        state.viewport_org_y = i32::from(value.y);
      }
      WmfRecordData::SetViewportExt(value) => {
        state.viewport_ext_x = nonzero_mapping_extent(i32::from(value.x));
        state.viewport_ext_y = nonzero_mapping_extent(i32::from(value.y));
      }
      WmfRecordData::OffsetWindowOrg(value) => {
        state.window_org_x += i32::from(value.x);
        state.window_org_y += i32::from(value.y);
      }
      WmfRecordData::OffsetViewportOrg(value) => {
        state.viewport_org_x += i32::from(value.x);
        state.viewport_org_y += i32::from(value.y);
      }
      WmfRecordData::ScaleWindowExt(value) => state.scale_window(value),
      WmfRecordData::ScaleViewportExt(value) => state.scale_viewport(value),
      WmfRecordData::SetTextAlign(value) => {
        state.text_alignment = value.text_alignment_flags();
      }
      WmfRecordData::CreateFontIndirect(value) => {
        state.insert_object(Some(wmf_text_font(&value)));
      }
      WmfRecordData::CreatePenIndirect(_)
      | WmfRecordData::CreateBrushIndirect(_)
      | WmfRecordData::CreatePalette(_)
      | WmfRecordData::CreatePatternBrush(_)
      | WmfRecordData::CreateRegion(_)
      | WmfRecordData::DibCreatePatternBrush(_) => state.insert_object(None),
      WmfRecordData::SelectObject(value) => state.select_object(value.index),
      WmfRecordData::DeleteObject(value) => {
        if let Some(slot) = state.objects.get_mut(value.index as usize) {
          *slot = None;
        }
      }
      WmfRecordData::TextOut(value) => {
        if let Some(run) = state.text_run(
          single_byte_text(&value.string),
          value.x_start,
          value.y_start,
          None,
        ) {
          runs.push(run);
        }
      }
      WmfRecordData::ExtTextOut(value) => {
        let width = (!value.dx.is_empty())
          .then(|| value.dx.iter().map(|advance| f32::from(*advance)).sum())
          .filter(|width| *width > 0.0);
        if let Some(run) = state.text_run(single_byte_text(&value.string), value.x, value.y, width)
        {
          runs.push(run);
        }
      }
      _ => {}
    }
  }
  runs
}

pub fn looks_like_metafile(data: &[u8], content_type: Option<&str>) -> bool {
  matches!(
    content_type,
    Some("image/x-wmf" | "image/wmf" | "image/x-emf" | "image/emf" | "application/x-msmetafile")
  ) || is_emf(data)
    || crate::wmf::looks_like_wmf(data)
}

fn decode_emf_as_raster(
  data: &[u8],
  options: RenderOptions,
  force_vector_replay: bool,
) -> Result<Option<DecodedMetafile>, String> {
  if !is_emf(data) {
    return Ok(None);
  }
  if data.len() < EMF_HEADER_SIZE {
    return Err("EMF header is truncated".into());
  }

  let mut pos = EMF_HEADER_SIZE;
  let mut bitmap_record = None;
  let mut bitmap_count = 0usize;
  let mut needs_vector_replay = false;

  while pos + EMF_RECORD_HEADER_SIZE <= data.len() {
    let record_type = read_u32(data, pos)?;
    let record_size = read_u32(data, pos + 4)? as usize;
    if record_size < EMF_RECORD_HEADER_SIZE || pos + record_size > data.len() {
      return Err(format!(
        "invalid EMF record at offset {pos}: type=0x{record_type:08x} size={record_size}"
      ));
    }
    if matches!(
      record_type,
      EMR_BIT_BLT | EMR_STRETCH_BLT | EMR_SET_DIBITS_TO_DEVICE | EMR_STRETCH_DIBITS
    ) {
      bitmap_count += 1;
      bitmap_record = Some((record_type, pos, record_size));
      // BITBLT and STRETCHBLT can depend on the existing destination through
      // their ternary raster operation, even when they are the only bitmap
      // record in the metafile.
      if bitmap_count > 1 || matches!(record_type, EMR_BIT_BLT | EMR_STRETCH_BLT) {
        needs_vector_replay = true;
      }
    } else if emf_record_needs_vector_replay(record_type) {
      needs_vector_replay = true;
    }

    pos += record_size;
    if record_type == EMR_EOF {
      break;
    }
  }

  if needs_vector_replay || force_vector_replay {
    return decode_vector_emf_as_png(data, options).map(Some);
  }

  let (record_type, record_offset, record_size) = match bitmap_record {
    Some(record) => record,
    None => return decode_vector_emf_as_png(data, options).map(Some),
  };
  decode_bitmap_record_as_raster(data, record_type, record_offset, record_size).map(Some)
}

fn emf_record_needs_vector_replay(record_type: u32) -> bool {
  matches!(
    record_type,
    EMR_POLYBEZIER
      | EMR_POLYGON
      | EMR_POLYLINE
      | EMR_POLYBEZIER_TO
      | EMR_POLYLINE_TO
      | EMR_POLYPOLYLINE
      | EMR_POLYPOLYGON
      | EMR_SET_PIXEL_V
      | EMR_MOVE_TO_EX
      | EMR_ELLIPSE
      | EMR_RECTANGLE
      | EMR_ROUND_RECT
      | EMR_ARC
      | EMR_CHORD
      | EMR_PIE
      | EMR_LINE_TO
      | EMR_COMMENT
      | EMR_EXT_TEXTOUT_A
      | EMR_EXT_TEXTOUT_W
      | EMR_POLYBEZIER16
      | EMR_POLYGON16
      | EMR_POLYLINE16
      | EMR_POLYBEZIER_TO16
      | EMR_POLYLINE_TO16
      | EMR_POLYPOLYLINE16
      | EMR_POLYPOLYGON16
  )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EmfColor {
  r: u8,
  g: u8,
  b: u8,
}

impl EmfColor {
  fn not(self) -> Self {
    Self {
      r: !self.r,
      g: !self.g,
      b: !self.b,
    }
  }

  fn and(self, other: Self) -> Self {
    Self {
      r: self.r & other.r,
      g: self.g & other.g,
      b: self.b & other.b,
    }
  }

  fn or(self, other: Self) -> Self {
    Self {
      r: self.r | other.r,
      g: self.g | other.g,
      b: self.b | other.b,
    }
  }

  fn xor(self, other: Self) -> Self {
    Self {
      r: self.r ^ other.r,
      g: self.g ^ other.g,
      b: self.b ^ other.b,
    }
  }
}

#[derive(Clone, Copy, Debug)]
struct EmfPoint {
  x: i32,
  y: i32,
}

#[derive(Clone, Copy, Debug)]
struct EmfTransform {
  m11: f32,
  m12: f32,
  m21: f32,
  m22: f32,
  dx: f32,
  dy: f32,
}

impl EmfTransform {
  fn identity() -> Self {
    Self {
      m11: 1.0,
      m12: 0.0,
      m21: 0.0,
      m22: 1.0,
      dx: 0.0,
      dy: 0.0,
    }
  }

  fn apply(self, point: EmfPoint) -> (f32, f32) {
    let x = point.x as f32;
    let y = point.y as f32;
    (
      x * self.m11 + y * self.m21 + self.dx,
      x * self.m12 + y * self.m22 + self.dy,
    )
  }

  fn multiply(self, other: Self) -> Self {
    Self {
      m11: self.m11 * other.m11 + self.m12 * other.m21,
      m12: self.m11 * other.m12 + self.m12 * other.m22,
      m21: self.m21 * other.m11 + self.m22 * other.m21,
      m22: self.m21 * other.m12 + self.m22 * other.m22,
      dx: self.dx * other.m11 + self.dy * other.m21 + other.dx,
      dy: self.dx * other.m12 + self.dy * other.m22 + other.dy,
    }
  }
}

#[derive(Clone, Copy, Debug)]
struct EmfPen {
  color: EmfColor,
  width: usize,
  transform_width: bool,
}

fn emf_pen_from_style(style: u32, pen: EmfPen) -> Option<EmfPen> {
  (EmrPenLineStyle::from_raw(style & 0x0000_000F) != Some(EmrPenLineStyle::Null)).then_some(pen)
}

#[derive(Clone, Debug)]
struct EmfFont {
  height: i32,
  family: Option<String>,
  weight: u16,
  italic: bool,
}

struct EmfTextState {
  width: usize,
  height: usize,
  window_org_x: i32,
  window_org_y: i32,
  window_ext_x: i32,
  window_ext_y: i32,
  viewport_org_x: i32,
  viewport_org_y: i32,
  viewport_ext_x: i32,
  viewport_ext_y: i32,
  world_transform: EmfTransform,
  fonts: std::collections::HashMap<u32, EmfFont>,
  current_font: Option<u32>,
  text_alignment: WmfTextAlignmentModeFlags,
  font_cache: RenderFontCache,
}

impl EmfTextState {
  fn new(data: &[u8]) -> Result<Self, String> {
    let (width, height) = emf_natural_canvas_size(data)?;

    Ok(Self {
      width,
      height,
      window_org_x: 0,
      window_org_y: 0,
      window_ext_x: width as i32,
      window_ext_y: height as i32,
      viewport_org_x: 0,
      viewport_org_y: 0,
      viewport_ext_x: width as i32,
      viewport_ext_y: height as i32,
      world_transform: EmfTransform::identity(),
      fonts: std::collections::HashMap::new(),
      current_font: None,
      text_alignment: WmfTextAlignmentModeFlags::empty(),
      font_cache: RenderFontCache::load(),
    })
  }

  fn map_point(&self, point: EmfPoint) -> (f32, f32) {
    let (x, y) = self.world_transform.apply(point);
    let scale_x = self.viewport_ext_x as f32 / nonzero_mapping_extent(self.window_ext_x) as f32;
    let scale_y = self.viewport_ext_y as f32 / nonzero_mapping_extent(self.window_ext_y) as f32;
    (
      self.viewport_org_x as f32 + (x - self.window_org_x as f32) * scale_x,
      self.viewport_org_y as f32 + (y - self.window_org_y as f32) * scale_y,
    )
  }

  fn map_height(&self, height: i32) -> f32 {
    let (_, y0) = self.map_point(EmfPoint { x: 0, y: 0 });
    let (_, y1) = self.map_point(EmfPoint {
      x: 0,
      y: height.abs(),
    });
    (y1 - y0).abs()
  }

  fn map_horizontal_distance(&self, logical_width: i64) -> f32 {
    let width = logical_width as f32;
    let scale_x = self.viewport_ext_x as f32 / nonzero_mapping_extent(self.window_ext_x) as f32;
    let scale_y = self.viewport_ext_y as f32 / nonzero_mapping_extent(self.window_ext_y) as f32;
    let x = width * self.world_transform.m11 * scale_x;
    let y = width * self.world_transform.m12 * scale_y;
    x.hypot(y)
  }

  fn text_run(
    &mut self,
    data: &[u8],
    record_offset: usize,
    record_size: usize,
    text: String,
  ) -> Option<MetafileTextRun> {
    let text_record = ext_text_record(data, record_offset, record_size)?;
    let logical_width = ext_text_advances(data, record_offset, record_size, text_record)
      .map(|values| values.iter().copied().sum::<i32>());
    let aligned_x = if self
      .text_alignment
      .contains(WmfTextAlignmentModeFlags::CENTER)
    {
      text_record
        .x
        .saturating_sub(logical_width.unwrap_or_default() / 2)
    } else if self
      .text_alignment
      .contains(WmfTextAlignmentModeFlags::RIGHT)
    {
      text_record
        .x
        .saturating_sub(logical_width.unwrap_or_default())
    } else {
      text_record.x
    };
    let (x, reference_y) = self.map_point(EmfPoint {
      x: aligned_x,
      y: text_record.y,
    });
    let selected_font = self
      .current_font
      .and_then(|id| self.fonts.get(&id))
      .cloned();
    let current_font = selected_font
      .as_ref()
      .map(|font| WmfTextFont {
        height: font.height,
        family: font.family.clone(),
        weight: font.weight,
        italic: font.italic,
      })
      .unwrap_or(WmfTextFont {
        height: 12,
        family: None,
        weight: 400,
        italic: false,
      });
    let font_size = self.map_height(current_font.height);
    // [MS-EMF] 2.3.11.25 and 2.3.5 define these as reference coordinates.
    // Their meaning comes from EMR_SETTEXTALIGN, so semantic text must use
    // the same aligned origin and realized-font baseline as vector replay.
    let y = self.font_cache.baseline_for_alignment(
      &current_font,
      font_size.round().max(1.0),
      reference_y.round(),
      self.text_alignment,
    );
    Some(MetafileTextRun {
      text,
      x: x / self.width.max(1) as f32,
      y: y / self.height.max(1) as f32,
      font_size: selected_font
        .as_ref()
        .map(|_| font_size / self.height.max(1) as f32),
      font_family: self
        .current_font
        .and_then(|id| self.fonts.get(&id))
        .and_then(|font| font.family.clone()),
      bold: self
        .current_font
        .and_then(|id| self.fonts.get(&id))
        .is_some_and(|font| font.weight > 400),
      italic: self
        .current_font
        .and_then(|id| self.fonts.get(&id))
        .is_some_and(|font| font.italic),
      // [MS-EMF] §2.2.5 defines Dx as the logical spacing between
      // consecutive character-cell origins. Map that logical distance
      // through the current page/world transform, then normalize it against
      // Header.Frame's playback surface. Header.Bounds encloses only marks;
      // using it as the canvas makes identical text wider whenever a
      // metafile happens to have tighter ink bounds.
      width: logical_width
        .map(|width| self.map_horizontal_distance(i64::from(width)) / self.width.max(1) as f32)
        .filter(|width| width.is_finite() && *width > 0.0),
    })
  }
}

struct EmfVectorState {
  width: usize,
  height: usize,
  output_scale_x: f32,
  output_scale_y: f32,
  window_org_x: i32,
  window_org_y: i32,
  window_ext_x: i32,
  window_ext_y: i32,
  viewport_org_x: i32,
  viewport_org_y: i32,
  viewport_ext_x: i32,
  viewport_ext_y: i32,
  world_transform: EmfTransform,
  brush_colors: std::collections::HashMap<u32, EmfColor>,
  pens: std::collections::HashMap<u32, Option<EmfPen>>,
  fonts: std::collections::HashMap<u32, EmfFont>,
  current_brush: Option<EmfColor>,
  current_pen: Option<EmfPen>,
  current_font: Option<u32>,
  current_pos: EmfPoint,
  text_color: EmfColor,
  binary_raster_operation: WmfBinaryRasterOperation,
  text_alignment: WmfTextAlignmentModeFlags,
  clip_rect: Option<(i32, i32, i32, i32)>,
  clip_mask: Option<Vec<bool>>,
  saved_states: Vec<EmfVectorSnapshot>,
  emf_plus_objects: Vec<Option<EmfPlusRenderObject>>,
  emf_plus_object_assembler: EmfPlusObjectAssembler,
  font_cache: RenderFontCache,
  rgb: Vec<u8>,
}

#[derive(Clone, Debug)]
struct EmfVectorSnapshot {
  window_org_x: i32,
  window_org_y: i32,
  window_ext_x: i32,
  window_ext_y: i32,
  viewport_org_x: i32,
  viewport_org_y: i32,
  viewport_ext_x: i32,
  viewport_ext_y: i32,
  world_transform: EmfTransform,
  current_brush: Option<EmfColor>,
  current_pen: Option<EmfPen>,
  current_font: Option<u32>,
  current_pos: EmfPoint,
  text_color: EmfColor,
  binary_raster_operation: WmfBinaryRasterOperation,
  text_alignment: WmfTextAlignmentModeFlags,
  clip_rect: Option<(i32, i32, i32, i32)>,
  clip_mask: Option<Vec<bool>>,
}

#[derive(Clone, Debug)]
enum EmfPlusRenderObject {
  Brush(Option<EmfPlusRenderBrush>),
  Pen(Option<EmfPen>),
  Path(Vec<EmfPoint>),
  Region(Vec<EmfPoint>),
  Image(RasterPixels),
  Font(EmfPlusFontObject),
  Unsupported,
}

#[derive(Clone, Debug)]
enum EmfPlusRenderBrush {
  Solid(EmfColor),
  Hatch {
    fore: EmfColor,
    back: EmfColor,
    style: u32,
  },
  LinearGradient {
    rect: (f32, f32, f32, f32),
    start: EmfColor,
    end: EmfColor,
  },
  PathGradient {
    center: (f32, f32),
    center_color: EmfColor,
    surround: EmfColor,
  },
  Texture(RasterPixels),
}

impl EmfPlusRenderBrush {
  fn representative_color(&self) -> EmfColor {
    match self {
      Self::Solid(color) => *color,
      Self::Hatch { fore, .. } => *fore,
      Self::LinearGradient { start, end, .. } => average_color(*start, *end),
      Self::PathGradient {
        center_color,
        surround,
        ..
      } => average_color(*center_color, *surround),
      Self::Texture(image) => average_image_color(image),
    }
  }

  fn color_at(&self, x: i32, y: i32) -> EmfColor {
    match self {
      Self::Solid(color) => *color,
      Self::Hatch { fore, back, style } => {
        if EmfPlusHatchStyle::from_raw(*style).is_some_and(|style| style.is_foreground(x, y)) {
          *fore
        } else {
          *back
        }
      }
      Self::LinearGradient { rect, start, end } => {
        let span = (rect.2 - rect.0).abs().max(1.0);
        let t = ((x as f32 - rect.0) / span).clamp(0.0, 1.0);
        lerp_color(*start, *end, t)
      }
      Self::PathGradient {
        center,
        center_color,
        surround,
      } => {
        let distance = ((x as f32 - center.0).hypot(y as f32 - center.1) / 256.0).clamp(0.0, 1.0);
        lerp_color(*center_color, *surround, distance)
      }
      Self::Texture(image) => {
        if image.width == 0 || image.height == 0 {
          return EmfColor { r: 0, g: 0, b: 0 };
        }
        let tx = x.rem_euclid(image.width as i32) as usize;
        let ty = y.rem_euclid(image.height as i32) as usize;
        let offset = (ty * image.width + tx) * RGB_BYTES_PER_PIXEL;
        EmfColor {
          r: image.rgb[offset],
          g: image.rgb[offset + 1],
          b: image.rgb[offset + 2],
        }
      }
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct RenderFontKey {
  family: Option<String>,
  weight: u16,
  italic: bool,
}

#[derive(Clone, Debug)]
struct RenderFontFace {
  font_data: fontique::Blob<u8>,
  face_index: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct RenderHintingKey {
  font: RenderFontKey,
  pixel_height_bits: u32,
}

struct RenderFontCache {
  collection: FontCollection,
  source_cache: SourceCache,
  faces: HashMap<RenderFontKey, Option<RenderFontFace>>,
  hinting_instances: HashMap<RenderHintingKey, HintingInstance>,
  raster_scratch: ZenoScratch,
}

#[derive(Clone, Debug)]
struct RenderedSubpixelGlyph {
  left: i32,
  top: i32,
  width: usize,
  height: usize,
  coverage: Vec<[u8; 3]>,
}

struct TextRenderRequest<'a> {
  font: &'a WmfTextFont,
  text: &'a str,
  x: f32,
  baseline_y: f32,
  height: f32,
  horizontal_scale: f32,
  advances: Option<&'a [f32]>,
}

impl RenderFontCache {
  fn load() -> Self {
    Self {
      collection: FontCollection::new(FontCollectionOptions {
        shared: false,
        system_fonts: true,
      }),
      source_cache: SourceCache::default(),
      faces: HashMap::new(),
      hinting_instances: HashMap::new(),
      raster_scratch: ZenoScratch::default(),
    }
  }

  fn resolve_face(&mut self, font: &WmfTextFont) -> Option<&RenderFontFace> {
    let key = RenderFontKey {
      family: font.family.clone(),
      weight: font.weight,
      italic: font.italic,
    };
    if !self.faces.contains_key(&key) {
      let mut families = Vec::with_capacity(2);
      if let Some(family) = key.family.as_deref() {
        families.push(QueryFamily::Named(family));
      }
      families.push(QueryFamily::Generic(GenericFamily::SansSerif));
      let weight = if key.weight == 0 {
        FontWeight::NORMAL
      } else {
        FontWeight::new(f32::from(key.weight.min(1000)))
      };
      let style = if key.italic {
        FontStyle::Italic
      } else {
        FontStyle::Normal
      };
      let mut query = self.collection.query(&mut self.source_cache);
      query.set_families(families);
      query.set_attributes(FontAttributes::new(FontWidth::NORMAL, style, weight));
      let mut face = None;
      query.matches_with(|font| {
        face = Some(RenderFontFace {
          font_data: font.blob.clone(),
          face_index: font.index,
        });
        QueryStatus::Stop
      });
      self.faces.insert(key.clone(), face);
    }
    self.faces.get(&key).and_then(Option::as_ref)
  }

  fn baseline_for_alignment(
    &mut self,
    font: &WmfTextFont,
    height: f32,
    reference_y: f32,
    alignment: WmfTextAlignmentModeFlags,
  ) -> f32 {
    if alignment.contains(WmfTextAlignmentModeFlags::BASELINE) {
      return reference_y;
    }
    let metrics = self.resolve_face(font).and_then(|face_data| {
      let face = FontRef::from_index(face_data.font_data.as_ref(), face_data.face_index).ok()?;
      Some(face.metrics(FontSize::new(height.max(1.0)), LocationRef::default()))
    });
    if alignment.contains(WmfTextAlignmentModeFlags::BOTTOM) {
      reference_y + metrics.map_or(0.0, |metrics| metrics.descent)
    } else {
      // [MS-WMF] 2.1.2.3 defines the all-zero vertical mode as TA_TOP.
      // Its reference point is the top of the font alignment box, so the
      // baseline is one font ascent below it. `lfHeight` is not the ascent:
      // substituting the character-cell height loses the hhea/OS/2 metrics
      // used by the GDI font mapper.
      reference_y + metrics.map_or(height, |metrics| metrics.ascent)
    }
  }

  fn render_text(&mut self, request: &TextRenderRequest<'_>) -> Option<Vec<RenderedSubpixelGlyph>> {
    let face_data = self.resolve_face(request.font)?.clone();
    let data = face_data.font_data.as_ref();
    let face = FontRef::from_index(data, face_data.face_index).ok()?;
    let size = FontSize::new(request.height.max(1.0));
    let location = LocationRef::new(&[]);
    let outlines = face.outline_glyphs();
    let charmap = face.charmap();
    let metrics = face.glyph_metrics(size, location);
    let hinting_key = RenderHintingKey {
      font: RenderFontKey {
        family: request.font.family.clone(),
        weight: request.font.weight,
        italic: request.font.italic,
      },
      pixel_height_bits: request.height.max(1.0).to_bits(),
    };
    if !self.hinting_instances.contains_key(&hinting_key) {
      // ExtTextOut's Dx array owns inter-glyph placement. Use the linear-metric
      // LCD mode from the Swash/Skrifa text pipeline so hinting cannot alter
      // the externally supplied spacing.
      let hinting = HintingInstance::new(
        &outlines,
        size,
        location,
        HintingOptions {
          engine: Default::default(),
          target: Target::Smooth {
            mode: SmoothMode::Lcd,
            symmetric_rendering: false,
            preserve_linear_metrics: true,
          },
        },
      )
      .ok()?;
      self.hinting_instances.insert(hinting_key.clone(), hinting);
    }
    let (hinting_instances, raster_scratch) = (&self.hinting_instances, &mut self.raster_scratch);
    let hinting = hinting_instances.get(&hinting_key)?;
    let mut cursor_x = request.x;
    let mut glyphs = Vec::with_capacity(request.text.chars().count());
    for (index, ch) in request.text.chars().enumerate() {
      if ch == '\n' || ch == '\r' {
        continue;
      }
      if ch.is_whitespace() {
        cursor_x += request
          .advances
          .and_then(|values| values.get(index))
          .copied()
          .unwrap_or(request.height * 0.35);
        continue;
      }
      let glyph_id = charmap.map(ch)?;
      let outline = outlines.get(glyph_id)?;
      let mut commands = Vec::new();
      let mut collector = ZenoGlyphPathCollector {
        commands: &mut commands,
      };
      let adjusted_metrics = outline
        .draw(DrawSettings::hinted(hinting, false), &mut collector)
        .ok()?;
      let mut mask = ZenoMask::with_scratch(&commands, raster_scratch);
      // Microsoft ClearType consumes a bitmap oversampled by at least six in
      // the horizontal direction, then applies one-pixel-wide displaced box
      // filters at the RGB stripe locations. Rasterize that producer input
      // directly instead of treating three shifted whole-pixel masks as its
      // final coverage.
      const CLEARTYPE_X_SCALE: f32 = 6.0;
      let fractional_offset = ZenoVector::new(
        cursor_x.fract() * CLEARTYPE_X_SCALE,
        request.baseline_y.fract(),
      );
      mask
        .format(ZenoMaskFormat::Alpha)
        .origin(ZenoOrigin::BottomLeft)
        .offset(fractional_offset)
        .render_offset(fractional_offset);
      mask.transform(Some(ZenoTransform::scale(
        request.horizontal_scale * CLEARTYPE_X_SCALE,
        1.0,
      )));
      // Zeno's BottomLeft placement includes the computed mask height. Match
      // Swash's render path by materializing that size before render_into;
      // calling Mask::render directly leaves placement.top without it.
      let mut data = Vec::new();
      mask.inspect(|format, width, height| {
        data.resize(format.buffer_size(width, height), 0);
      });
      let placement = mask.render_into(&mut data, None);
      if data.len() != placement.width as usize * placement.height as usize {
        return None;
      }
      let high_resolution_left = (cursor_x.floor() as i32)
        .saturating_mul(CLEARTYPE_X_SCALE as i32)
        .saturating_add(placement.left);
      let (left, width, coverage) = cleartype_box_decimate(
        &data,
        placement.width as usize,
        placement.height as usize,
        high_resolution_left,
      );
      glyphs.push(RenderedSubpixelGlyph {
        left,
        top: request.baseline_y.floor() as i32 - placement.top,
        width,
        height: placement.height as usize,
        coverage,
      });
      let advance = adjusted_metrics
        .advance_width
        .or_else(|| metrics.advance_width(glyph_id))
        .unwrap_or(request.height * 0.5);
      cursor_x += request
        .advances
        .and_then(|values| values.get(index))
        .copied()
        .unwrap_or(advance);
    }
    Some(glyphs)
  }
}

/// Applies the one-pixel-wide displaced box filters described by Microsoft's
/// ClearType RGB-decimation paper to a six-times-horizontal alpha raster.
fn cleartype_box_decimate(
  high_resolution: &[u8],
  high_resolution_width: usize,
  height: usize,
  high_resolution_left: i32,
) -> (i32, usize, Vec<[u8; 3]>) {
  const SAMPLES_PER_PIXEL: i32 = 6;
  if high_resolution_width == 0 || height == 0 {
    return (0, 0, Vec::new());
  }
  let high_resolution_right = high_resolution_left.saturating_add(high_resolution_width as i32);
  let left = high_resolution_left.div_euclid(SAMPLES_PER_PIXEL) - 1;
  let right = (high_resolution_right - 1).div_euclid(SAMPLES_PER_PIXEL) + 2;
  let width = (right - left).max(0) as usize;
  let mut output = vec![[0; 3]; width * height];

  for y in 0..height {
    let row = &high_resolution[y * high_resolution_width..(y + 1) * high_resolution_width];
    for output_x in left..right {
      let mut channels = [0_u8; 3];
      for (channel, window_offset) in [-2_i32, 0, 2].into_iter().enumerate() {
        let window_start = output_x
          .saturating_mul(SAMPLES_PER_PIXEL)
          .saturating_add(window_offset);
        let mut sum = 0_u16;
        for sample_x in window_start..window_start + SAMPLES_PER_PIXEL {
          let source_x = sample_x - high_resolution_left;
          if let Ok(source_x) = usize::try_from(source_x)
            && let Some(sample) = row.get(source_x)
          {
            sum += u16::from(*sample);
          }
        }
        channels[channel] = ((sum + 3) / SAMPLES_PER_PIXEL as u16) as u8;
      }
      output[y * width + (output_x - left) as usize] = channels;
    }
  }
  (left, width, output)
}

struct ZenoGlyphPathCollector<'a> {
  commands: &'a mut Vec<ZenoCommand>,
}

impl OutlinePen for ZenoGlyphPathCollector<'_> {
  fn move_to(&mut self, x: f32, y: f32) {
    self
      .commands
      .push(ZenoCommand::MoveTo(ZenoPoint::new(x, y)));
  }

  fn line_to(&mut self, x: f32, y: f32) {
    self
      .commands
      .push(ZenoCommand::LineTo(ZenoPoint::new(x, y)));
  }

  fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
    self.commands.push(ZenoCommand::QuadTo(
      ZenoPoint::new(x1, y1),
      ZenoPoint::new(x, y),
    ));
  }

  fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
    self.commands.push(ZenoCommand::CurveTo(
      ZenoPoint::new(x1, y1),
      ZenoPoint::new(x2, y2),
      ZenoPoint::new(x, y),
    ));
  }

  fn close(&mut self) {
    self.commands.push(ZenoCommand::Close);
  }
}

impl EmfVectorState {
  fn new_with_options(data: &[u8], options: RenderOptions) -> Result<Self, String> {
    let (natural_width, natural_height) = emf_natural_canvas_size(data)?;
    let (width, height) = options.resolved_canvas_size(natural_width, natural_height);
    let output_scale_x = width as f32 / natural_width.max(1) as f32;
    let output_scale_y = height as f32 / natural_height.max(1) as f32;
    let background_color = options.background_color.unwrap_or([255; 3]);
    let mut rgb = vec![0; width * height * RGB_BYTES_PER_PIXEL];
    for pixel in rgb.chunks_exact_mut(RGB_BYTES_PER_PIXEL) {
      pixel.copy_from_slice(&background_color);
    }

    Ok(Self {
      width,
      height,
      output_scale_x,
      output_scale_y,
      window_org_x: 0,
      window_org_y: 0,
      window_ext_x: natural_width as i32,
      window_ext_y: natural_height as i32,
      viewport_org_x: 0,
      viewport_org_y: 0,
      viewport_ext_x: natural_width as i32,
      viewport_ext_y: natural_height as i32,
      world_transform: EmfTransform::identity(),
      brush_colors: std::collections::HashMap::new(),
      pens: std::collections::HashMap::new(),
      fonts: std::collections::HashMap::new(),
      current_brush: None,
      current_pen: Some(EmfPen {
        color: EmfColor { r: 0, g: 0, b: 0 },
        width: 1,
        transform_width: false,
      }),
      current_font: None,
      current_pos: EmfPoint { x: 0, y: 0 },
      text_color: EmfColor { r: 0, g: 0, b: 0 },
      binary_raster_operation: WmfBinaryRasterOperation::CopyPen,
      text_alignment: WmfTextAlignmentModeFlags::empty(),
      clip_rect: None,
      clip_mask: None,
      saved_states: Vec::new(),
      emf_plus_objects: Vec::new(),
      emf_plus_object_assembler: EmfPlusObjectAssembler::default(),
      font_cache: RenderFontCache::load(),
      rgb,
    })
  }

  fn map_point(&self, point: EmfPoint) -> (f32, f32) {
    let (x, y) = self.world_transform.apply(point);
    let scale_x = self.viewport_ext_x as f32 / nonzero_mapping_extent(self.window_ext_x) as f32;
    let scale_y = self.viewport_ext_y as f32 / nonzero_mapping_extent(self.window_ext_y) as f32;
    (
      (self.viewport_org_x as f32 + (x - self.window_org_x as f32) * scale_x) * self.output_scale_x,
      (self.viewport_org_y as f32 + (y - self.window_org_y as f32) * scale_y) * self.output_scale_y,
    )
  }

  fn resolve_pen(&self, mut pen: EmfPen) -> EmfPen {
    if !pen.transform_width {
      return pen;
    }
    let width = pen.width as f32;
    let scale_x = self.viewport_ext_x as f32 / nonzero_mapping_extent(self.window_ext_x) as f32
      * self.output_scale_x;
    let scale_y = self.viewport_ext_y as f32 / nonzero_mapping_extent(self.window_ext_y) as f32
      * self.output_scale_y;
    let x_axis = (
      width * self.world_transform.m11 * scale_x,
      width * self.world_transform.m12 * scale_y,
    );
    let y_axis = (
      width * self.world_transform.m21 * scale_x,
      width * self.world_transform.m22 * scale_y,
    );
    let width = x_axis.0.hypot(x_axis.1).max(y_axis.0.hypot(y_axis.1));
    pen.width = if width.is_finite() {
      width.round().max(1.0) as usize
    } else {
      1
    };
    pen.transform_width = false;
    pen
  }

  fn set_pixel(&mut self, x: i32, y: i32, color: EmfColor) {
    if x < 0 || y < 0 {
      return;
    }
    if let Some((left, top, right, bottom)) = self.clip_rect
      && (x < left || x >= right || y < top || y >= bottom)
    {
      return;
    }
    let (x, y) = (x as usize, y as usize);
    if x >= self.width || y >= self.height {
      return;
    }
    if let Some(mask) = &self.clip_mask
      && !mask[y * self.width + x]
    {
      return;
    }
    let offset = (y * self.width + x) * RGB_BYTES_PER_PIXEL;
    self.rgb[offset] = color.r;
    self.rgb[offset + 1] = color.g;
    self.rgb[offset + 2] = color.b;
  }

  fn set_vector_pixel(&mut self, x: i32, y: i32, color: EmfColor) {
    let Some(destination) = self.pixel(x, y) else {
      return;
    };
    self.set_pixel(
      x,
      y,
      apply_binary_raster_operation(color, destination, self.binary_raster_operation),
    );
  }

  fn pixel(&self, x: i32, y: i32) -> Option<EmfColor> {
    let (x, y) = (usize::try_from(x).ok()?, usize::try_from(y).ok()?);
    if x >= self.width || y >= self.height {
      return None;
    }
    let offset = (y * self.width + x) * RGB_BYTES_PER_PIXEL;
    Some(EmfColor {
      r: self.rgb[offset],
      g: self.rgb[offset + 1],
      b: self.rgb[offset + 2],
    })
  }

  fn draw_rgb_image(
    &mut self,
    dest_x: i32,
    dest_y: i32,
    dest_width: i32,
    dest_height: i32,
    image: &RasterPixels,
  ) {
    let (mapped_left, mapped_top) = self.map_point(EmfPoint {
      x: dest_x,
      y: dest_y,
    });
    let (mapped_right, mapped_bottom) = self.map_point(EmfPoint {
      x: dest_x + dest_width,
      y: dest_y + dest_height,
    });
    let left = mapped_left.min(mapped_right).round() as i32;
    let top = mapped_top.min(mapped_bottom).round() as i32;
    // StretchBlt's destination extent is half-open. Office/GDI maps the
    // leading edge to the nearest device pixel and truncates the exclusive
    // trailing edge; rounding both edges makes a half-pixel bottom grow by
    // one row (as in the 32-unit preview bitmap in tdf135653.docx).
    let right = mapped_left.max(mapped_right).floor() as i32;
    let bottom = mapped_top.max(mapped_bottom).floor() as i32;
    let width = (right - left).max(1);
    let height = (bottom - top).max(1);
    let interpolate = width as usize != image.width || height as usize != image.height;

    for y in 0..height {
      for x in 0..width {
        let color = if interpolate {
          bilinear_raster_color(
            image,
            x as usize,
            y as usize,
            width as usize,
            height as usize,
          )
        } else {
          raster_color(image, x as usize, y as usize)
        };
        self.set_pixel(left + x, top + y, color);
      }
    }
  }

  fn draw_rgb_image_with_rop(
    &mut self,
    dest_x: i32,
    dest_y: i32,
    dest_width: i32,
    dest_height: i32,
    image: &RasterPixels,
    rop: WmfTernaryRasterOperationCode,
  ) {
    let (mapped_left, mapped_top) = self.map_point(EmfPoint {
      x: dest_x,
      y: dest_y,
    });
    let (mapped_right, mapped_bottom) = self.map_point(EmfPoint {
      x: dest_x + dest_width,
      y: dest_y + dest_height,
    });
    let left = mapped_left.min(mapped_right).round() as i32;
    let top = mapped_top.min(mapped_bottom).round() as i32;
    let right = mapped_left.max(mapped_right).floor() as i32;
    let bottom = mapped_top.max(mapped_bottom).floor() as i32;
    let width = (right - left).max(1);
    let height = (bottom - top).max(1);
    // A one-bit mask in the canonical SRCAND/SRCINVERT transparency pair must
    // keep its boolean samples. Color sources follow the filtered StretchBlt
    // path used by Office/GDI+ when the destination viewport changes size.
    let interpolate = (width as usize != image.width || height as usize != image.height)
      && !is_discrete_two_color_raster(image);

    for y in 0..height {
      for x in 0..width {
        let dest_x = left + x;
        let dest_y = top + y;
        let src = if interpolate {
          bilinear_raster_color(
            image,
            x as usize,
            y as usize,
            width as usize,
            height as usize,
          )
        } else {
          let src_x = nearest_raster_index(x as usize, width as usize, image.width);
          let src_y = nearest_raster_index(y as usize, height as usize, image.height);
          raster_color(image, src_x, src_y)
        };
        if let Some(color) = self.apply_raster_op(dest_x, dest_y, src, rop) {
          self.set_pixel(dest_x, dest_y, color);
        }
      }
    }
  }

  fn draw_masked_rgb_image(
    &mut self,
    dest_x: i32,
    dest_y: i32,
    dest_width: i32,
    dest_height: i32,
    image: &RasterPixels,
    mask: &RasterPixels,
  ) {
    let (mapped_left, mapped_top) = self.map_point(EmfPoint {
      x: dest_x,
      y: dest_y,
    });
    let (mapped_right, mapped_bottom) = self.map_point(EmfPoint {
      x: dest_x + dest_width,
      y: dest_y + dest_height,
    });
    let left = mapped_left.min(mapped_right).round() as i32;
    let top = mapped_top.min(mapped_bottom).round() as i32;
    let right = mapped_left.max(mapped_right).floor() as i32;
    let bottom = mapped_top.max(mapped_bottom).floor() as i32;
    let width = (right - left).max(1) as usize;
    let height = (bottom - top).max(1) as usize;
    let interpolate = width != image.width || height != image.height;

    for y in 0..height {
      let mask_y = nearest_raster_index(y, height, mask.height);
      for x in 0..width {
        let mask_x = nearest_raster_index(x, width, mask.width);
        let mask_color = raster_color(mask, mask_x, mask_y);
        let color = if interpolate {
          gdi_plus_bilinear_raster_color(image, x, y, width, height)
        } else {
          raster_color(image, x, y)
        };
        // The canonical SRCAND mask uses black for covered source pixels and
        // white for the transparent destination. GDI+ filters the paired
        // SRCINVERT color bitmap independently, and a nonblack filtered
        // sample is consequently part of the pair's opaque output even when
        // its nearest one-bit mask sample is white. Keeping that color fringe
        // is required before black/white destination reconstruction; masking
        // first contracts icon edges.
        if u16::from(mask_color.r) + u16::from(mask_color.g) + u16::from(mask_color.b) >= 3 * 128
          && color == (EmfColor { r: 0, g: 0, b: 0 })
        {
          continue;
        }
        self.set_pixel(left + x as i32, top + y as i32, color);
      }
    }
  }

  fn fill_rect_with_rop(
    &mut self,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    rop: WmfTernaryRasterOperationCode,
  ) {
    let Some(brush) = self.current_brush else {
      return;
    };
    let (mapped_left, mapped_top) = self.map_point(EmfPoint { x: left, y: top });
    let (mapped_right, mapped_bottom) = self.map_point(EmfPoint {
      x: right,
      y: bottom,
    });
    let left = mapped_left.min(mapped_right).round().max(0.0) as i32;
    let top = mapped_top.min(mapped_bottom).round().max(0.0) as i32;
    let right = mapped_left.max(mapped_right).round().min(self.width as f32) as i32;
    let bottom = mapped_top
      .max(mapped_bottom)
      .round()
      .min(self.height as f32) as i32;
    for y in top..bottom {
      for x in left..right {
        if let Some(color) = self.apply_raster_op(x, y, brush, rop) {
          self.set_pixel(x, y, color);
        }
      }
    }
  }

  fn apply_raster_op(
    &self,
    x: i32,
    y: i32,
    src: EmfColor,
    rop: WmfTernaryRasterOperationCode,
  ) -> Option<EmfColor> {
    self.apply_raster_op_with_pattern(x, y, src, self.current_brush.unwrap_or(src), rop)
  }

  fn apply_raster_op_with_pattern(
    &self,
    x: i32,
    y: i32,
    src: EmfColor,
    pattern: EmfColor,
    rop: WmfTernaryRasterOperationCode,
  ) -> Option<EmfColor> {
    let dest = self.pixel_color(x, y).unwrap_or(EmfColor {
      r: 255,
      g: 255,
      b: 255,
    });
    let color = match rop {
      WmfTernaryRasterOperationCode::BLACKNESS => EmfColor { r: 0, g: 0, b: 0 },
      WmfTernaryRasterOperationCode::WHITENESS => EmfColor {
        r: 255,
        g: 255,
        b: 255,
      },
      WmfTernaryRasterOperationCode::DSTINVERT => dest.not(),
      WmfTernaryRasterOperationCode::NOTSRCCOPY => src.not(),
      WmfTernaryRasterOperationCode::SRCCOPY => src,
      WmfTernaryRasterOperationCode::SRCPAINT => src.or(dest),
      WmfTernaryRasterOperationCode::SRCAND => src.and(dest),
      WmfTernaryRasterOperationCode::SRCINVERT => src.xor(dest),
      WmfTernaryRasterOperationCode::SRCERASE => src.and(dest.not()),
      WmfTernaryRasterOperationCode::MERGECOPY => src.and(pattern),
      WmfTernaryRasterOperationCode::MERGEPAINT => src.not().or(dest),
      WmfTernaryRasterOperationCode::PATCOPY => pattern,
      WmfTernaryRasterOperationCode::PATINVERT => pattern.xor(dest),
      WmfTernaryRasterOperationCode::PATPAINT => pattern.or(src.not()).or(dest),
      WmfTernaryRasterOperationCode::D => return None,
      _ => return None,
    };
    Some(color)
  }

  fn pixel_color(&self, x: i32, y: i32) -> Option<EmfColor> {
    if x < 0 || y < 0 {
      return None;
    }
    let (x, y) = (x as usize, y as usize);
    if x >= self.width || y >= self.height {
      return None;
    }
    let offset = (y * self.width + x) * RGB_BYTES_PER_PIXEL;
    Some(EmfColor {
      r: self.rgb[offset],
      g: self.rgb[offset + 1],
      b: self.rgb[offset + 2],
    })
  }

  fn mapped_vertical_length(&self, logical_height: i32) -> f32 {
    let height = logical_height.unsigned_abs().max(1) as f32;
    let scale_x = self.viewport_ext_x as f32 / nonzero_mapping_extent(self.window_ext_x) as f32
      * self.output_scale_x;
    let scale_y = self.viewport_ext_y as f32 / nonzero_mapping_extent(self.window_ext_y) as f32
      * self.output_scale_y;
    let x = height * self.world_transform.m21 * scale_x;
    let y = height * self.world_transform.m22 * scale_y;
    x.hypot(y).max(1.0)
  }

  fn mapped_horizontal_distance(&self, logical_width: i64) -> f32 {
    let width = logical_width as f32;
    let scale_x = self.viewport_ext_x as f32 / nonzero_mapping_extent(self.window_ext_x) as f32
      * self.output_scale_x;
    let scale_y = self.viewport_ext_y as f32 / nonzero_mapping_extent(self.window_ext_y) as f32
      * self.output_scale_y;
    let x = (width * self.world_transform.m11 * scale_x).round();
    let y = (width * self.world_transform.m12 * scale_y).round();
    x.hypot(y).copysign(width)
  }

  fn draw_text(&mut self, x: i32, y: i32, text: &str, color: EmfColor, height: i32) {
    self.draw_text_with_font(
      x,
      y,
      text,
      color,
      &WmfTextFont {
        height,
        family: None,
        weight: 400,
        italic: false,
      },
    );
  }

  fn draw_text_with_font(
    &mut self,
    x: i32,
    y: i32,
    text: &str,
    color: EmfColor,
    font: &WmfTextFont,
  ) {
    let (mapped_x, mapped_y) = self.map_point(EmfPoint { x, y });
    let height = self
      .mapped_vertical_length(if font.height == 0 { 12 } else { font.height })
      .round()
      .max(1.0);
    self.draw_text_at_device(
      color,
      TextRenderRequest {
        font,
        text,
        x: mapped_x.round(),
        baseline_y: mapped_y.round(),
        height,
        horizontal_scale: 1.0,
        advances: None,
      },
    );
  }

  fn draw_emf_text(
    &mut self,
    text_record: ExtTextRecord,
    text: &str,
    color: EmfColor,
    font: &WmfTextFont,
    logical_advances: Option<&[i32]>,
  ) {
    let font_height = if font.height == 0 { 12 } else { font.height };
    let logical_width = logical_advances.map(|values| values.iter().copied().sum::<i32>());
    let aligned_x = if self
      .text_alignment
      .contains(WmfTextAlignmentModeFlags::CENTER)
    {
      text_record
        .x
        .saturating_sub(logical_width.unwrap_or_default() / 2)
    } else if self
      .text_alignment
      .contains(WmfTextAlignmentModeFlags::RIGHT)
    {
      text_record
        .x
        .saturating_sub(logical_width.unwrap_or_default())
    } else {
      text_record.x
    };
    let (mapped_x, reference_y) = self.map_point(EmfPoint {
      x: aligned_x,
      y: text_record.y,
    });
    // GDI maps the logical text reference point and LOGFONT character height
    // to device units before selecting and rasterizing the realized font.
    let mapped_x = mapped_x.round();
    let reference_y = reference_y.round();
    // GDI realizes a transformed LOGFONT height in whole device pixels by
    // truncating the positive magnitude. For example, the -11 logical Segoe
    // UI font in tdf135653 maps to 22.83 pixels and Windows uses a 22-pixel
    // realization; rounding to 23 grows every glyph one row above TA_TOP.
    let height = self.mapped_vertical_length(font_height).floor().max(1.0);
    let mapped_y =
      self
        .font_cache
        .baseline_for_alignment(font, height, reference_y, self.text_alignment);
    let advances = logical_advances.map(|values| {
      cumulative_mapped_advances(values, |logical_cumulative| {
        self.mapped_horizontal_distance(logical_cumulative)
      })
    });
    // [MS-EMF] 2.3.5.8 defines exScale/eyScale for GM_COMPATIBLE
    // text. LibreOffice's EMF reader applies their absolute ratio to the
    // realized font width while mapping the supplied Dx positions separately.
    // A controlled Windows GDI+ replay with exScale changed to equal eyScale
    // also widens the glyphs without changing their supplied origins.
    let horizontal_scale = if text_record.graphics_mode == 1
      && text_record.x_scale.is_finite()
      && text_record.y_scale.is_finite()
      && text_record.x_scale != 0.0
    {
      (text_record.y_scale / text_record.x_scale).abs()
    } else {
      1.0
    };
    self.draw_text_at_device(
      color,
      TextRenderRequest {
        font,
        text,
        x: mapped_x,
        baseline_y: mapped_y,
        height,
        horizontal_scale,
        advances: advances.as_deref(),
      },
    );
  }

  fn draw_text_at_device(&mut self, color: EmfColor, request: TextRenderRequest<'_>) {
    if let Some(glyphs) = self.font_cache.render_text(&request) {
      for glyph in &glyphs {
        self.draw_subpixel_glyph(glyph, color);
      }
      return;
    }

    let scale = ((request.height as usize).max(7) / 7).max(1);
    let mut cursor_x = request.x.round() as i32;
    let baseline_y = request.baseline_y.round() as i32;
    for (index, ch) in request.text.chars().enumerate() {
      let advance = request
        .advances
        .and_then(|values| values.get(index))
        .copied()
        .unwrap_or((6 * scale) as f32)
        .round() as i32;
      if ch.is_whitespace() {
        cursor_x += request
          .advances
          .and_then(|values| values.get(index))
          .copied()
          .unwrap_or((4 * scale) as f32)
          .round() as i32;
        continue;
      }
      draw_glyph_5x7(
        self,
        cursor_x,
        baseline_y - (7 * scale) as i32,
        ch,
        color,
        scale,
      );
      cursor_x += advance;
    }
  }

  fn draw_subpixel_glyph(&mut self, glyph: &RenderedSubpixelGlyph, color: EmfColor) {
    // DEFAULT_QUALITY GDI text uses the system smoothing target. During
    // metafile playback its covered edge samples are written as destination
    // colors. Office's GDI+ playback preserves the union of the three
    // ClearType coverage channels as opaque text-color pixels in the embedded
    // image, rather than exporting fractional-alpha or white-blended edges.
    for y in 0..glyph.height {
      for x in 0..glyph.width {
        let coverage = glyph.coverage[y * glyph.width + x];
        if coverage != [0; 3] {
          self.set_vector_pixel(glyph.left + x as i32, glyph.top + y as i32, color);
        }
      }
    }
  }

  fn fill_arc_segment(
    &mut self,
    rect: (i32, i32, i32, i32),
    start_angle: f32,
    sweep_angle: f32,
    pie: bool,
  ) {
    let (left, top, right, bottom) = rect;
    let points = arc_segment_points(left, top, right, bottom, start_angle, sweep_angle, pie);
    if pie {
      self.fill_polygon(&points);
      self.draw_polyline(&points, true);
    } else {
      self.draw_polyline(&points, false);
    }
  }

  fn save_state(&mut self) {
    self.saved_states.push(EmfVectorSnapshot {
      window_org_x: self.window_org_x,
      window_org_y: self.window_org_y,
      window_ext_x: self.window_ext_x,
      window_ext_y: self.window_ext_y,
      viewport_org_x: self.viewport_org_x,
      viewport_org_y: self.viewport_org_y,
      viewport_ext_x: self.viewport_ext_x,
      viewport_ext_y: self.viewport_ext_y,
      world_transform: self.world_transform,
      current_brush: self.current_brush,
      current_pen: self.current_pen,
      current_font: self.current_font,
      current_pos: self.current_pos,
      text_color: self.text_color,
      binary_raster_operation: self.binary_raster_operation,
      text_alignment: self.text_alignment,
      clip_rect: self.clip_rect,
      clip_mask: self.clip_mask.clone(),
    });
  }

  fn restore_state(&mut self) {
    let Some(saved) = self.saved_states.pop() else {
      return;
    };
    self.window_org_x = saved.window_org_x;
    self.window_org_y = saved.window_org_y;
    self.window_ext_x = saved.window_ext_x;
    self.window_ext_y = saved.window_ext_y;
    self.viewport_org_x = saved.viewport_org_x;
    self.viewport_org_y = saved.viewport_org_y;
    self.viewport_ext_x = saved.viewport_ext_x;
    self.viewport_ext_y = saved.viewport_ext_y;
    self.world_transform = saved.world_transform;
    self.current_brush = saved.current_brush;
    self.current_pen = saved.current_pen;
    self.current_font = saved.current_font;
    self.current_pos = saved.current_pos;
    self.text_color = saved.text_color;
    self.binary_raster_operation = saved.binary_raster_operation;
    self.text_alignment = saved.text_alignment;
    self.clip_rect = saved.clip_rect;
    self.clip_mask = saved.clip_mask;
  }

  fn set_clip_rect_logical(&mut self, left: i32, top: i32, right: i32, bottom: i32) {
    let (x1, y1) = self.map_point(EmfPoint { x: left, y: top });
    let (x2, y2) = self.map_point(EmfPoint {
      x: right,
      y: bottom,
    });
    let rect = (
      x1.min(x2).floor().max(0.0) as i32,
      y1.min(y2).floor().max(0.0) as i32,
      x1.max(x2).ceil().min(self.width as f32) as i32,
      y1.max(y2).ceil().min(self.height as f32) as i32,
    );
    self.set_clip_rect_device(rect, 0);
  }

  fn set_clip_rect_device(&mut self, rect: (i32, i32, i32, i32), combine_mode: u8) {
    let next = (
      rect.0.clamp(0, self.width as i32),
      rect.1.clamp(0, self.height as i32),
      rect.2.clamp(0, self.width as i32),
      rect.3.clamp(0, self.height as i32),
    );
    if combine_mode == 0 {
      self.clip_rect = Some(next);
      self.clip_mask = None;
      return;
    }
    if combine_mode == 1 && self.clip_mask.is_none() {
      self.clip_rect = Some(match self.clip_rect {
        Some(current) => intersect_rects(current, next),
        None => next,
      });
      return;
    }
    let mask = self.rect_clip_mask(next);
    self.combine_clip_mask(mask, combine_mode);
  }

  fn set_clip_points_logical(&mut self, points: &[EmfPoint], combine_mode: u8) {
    let mapped = points
      .iter()
      .map(|point| self.map_point(*point))
      .collect::<Vec<_>>();
    if let Some(rect) = axis_aligned_clip_rect(&mapped, self.width, self.height) {
      self.set_clip_rect_device(rect, combine_mode);
      return;
    }
    let mask = self.polygon_mask(&mapped);
    self.combine_clip_mask(mask, combine_mode);
  }

  fn offset_clip(&mut self, dx: f32, dy: f32) {
    if let Some((left, top, right, bottom)) = self.clip_rect {
      self.clip_rect = Some((
        (left as f32 + dx).round() as i32,
        (top as f32 + dy).round() as i32,
        (right as f32 + dx).round() as i32,
        (bottom as f32 + dy).round() as i32,
      ));
    }
    if let Some(mask) = self.clip_mask.take() {
      let mut shifted = vec![false; mask.len()];
      let dx = dx.round() as i32;
      let dy = dy.round() as i32;
      for y in 0..self.height {
        for x in 0..self.width {
          if !mask[y * self.width + x] {
            continue;
          }
          let nx = x as i32 + dx;
          let ny = y as i32 + dy;
          if nx >= 0 && ny >= 0 && nx < self.width as i32 && ny < self.height as i32 {
            shifted[ny as usize * self.width + nx as usize] = true;
          }
        }
      }
      self.clip_mask = Some(shifted);
      self.update_clip_rect_from_mask();
    }
  }

  fn combine_clip_mask(&mut self, next: Vec<bool>, combine_mode: u8) {
    let current = match self.clip_mask.take() {
      Some(mask) => Some(mask),
      None => self.clip_rect.map(|rect| self.rect_clip_mask(rect)),
    };
    let mask = match (current, combine_mode) {
      (_, 0) => Some(next),
      (None, 1) => Some(next),
      (Some(current), 1) => Some(
        current
          .into_iter()
          .zip(next)
          .map(|(left, right)| left && right)
          .collect(),
      ),
      (None, 2) => None,
      (Some(current), 2) => Some(
        current
          .into_iter()
          .zip(next)
          .map(|(left, right)| left || right)
          .collect(),
      ),
      (None, 3) => Some(next),
      (Some(current), 3) => Some(
        current
          .into_iter()
          .zip(next)
          .map(|(left, right)| left ^ right)
          .collect(),
      ),
      (None, 4) => None,
      (Some(current), 4) => Some(
        current
          .into_iter()
          .zip(next)
          .map(|(left, right)| left && !right)
          .collect(),
      ),
      (None, 5) => Some(next.into_iter().map(|value| !value).collect()),
      (Some(current), 5) => Some(
        current
          .into_iter()
          .zip(next)
          .map(|(left, right)| right && !left)
          .collect(),
      ),
      (_, _) => Some(next),
    };
    self.clip_mask = mask;
    self.update_clip_rect_from_mask();
  }

  fn rect_clip_mask(&self, rect: (i32, i32, i32, i32)) -> Vec<bool> {
    let mut mask = vec![false; self.width * self.height];
    for y in rect.1.max(0) as usize..rect.3.max(0) as usize {
      let row = y * self.width;
      for x in rect.0.max(0) as usize..rect.2.max(0) as usize {
        mask[row + x] = true;
      }
    }
    mask
  }

  fn update_clip_rect_from_mask(&mut self) {
    let Some(mask) = &self.clip_mask else {
      self.clip_rect = None;
      return;
    };
    let mut left = self.width;
    let mut top = self.height;
    let mut right = 0usize;
    let mut bottom = 0usize;
    for y in 0..self.height {
      for x in 0..self.width {
        if !mask[y * self.width + x] {
          continue;
        }
        left = left.min(x);
        top = top.min(y);
        right = right.max(x + 1);
        bottom = bottom.max(y + 1);
      }
    }
    self.clip_rect = (right > left && bottom > top).then_some((
      left as i32,
      top as i32,
      right as i32,
      bottom as i32,
    ));
  }

  fn polygon_mask(&self, mapped: &[(f32, f32)]) -> Vec<bool> {
    let mut mask = vec![false; self.width * self.height];
    if mapped.len() < 3 {
      return mask;
    }
    visit_polygon_scanline_spans(mapped, self.width, self.height, |y, start, end| {
      for x in start..end {
        mask[y * self.width + x] = true;
      }
    });
    mask
  }

  fn fill_polygon(&mut self, points: &[EmfPoint]) {
    let Some(color) = self.current_brush else {
      return;
    };
    if points.len() < 3 {
      return;
    }

    let mapped = points
      .iter()
      .map(|point| self.map_point(*point))
      .collect::<Vec<_>>();
    let width = self.width;
    let height = self.height;
    visit_polygon_scanline_spans(&mapped, width, height, |y, start, end| {
      for x in start..end {
        self.set_vector_pixel(x as i32, y as i32, color);
      }
    });
  }

  fn fill_polygon_with_emf_plus_brush(&mut self, points: &[EmfPoint], brush: &EmfPlusRenderBrush) {
    if points.len() < 3 {
      return;
    }

    let mapped = points
      .iter()
      .map(|point| self.map_point(*point))
      .collect::<Vec<_>>();
    let width = self.width;
    let height = self.height;
    visit_polygon_scanline_spans(&mapped, width, height, |y, start, end| {
      for x in start..end {
        self.set_pixel(x as i32, y as i32, brush.color_at(x as i32, y as i32));
      }
    });
  }

  fn draw_polyline(&mut self, points: &[EmfPoint], closed: bool) {
    let Some(pen) = self.current_pen else {
      return;
    };
    if points.len() < 2 {
      return;
    }
    for pair in points.windows(2) {
      self.draw_line(pair[0], pair[1], pen);
    }
    if closed {
      self.draw_line(points[points.len() - 1], points[0], pen);
    }
  }

  fn draw_line(&mut self, a: EmfPoint, b: EmfPoint, pen: EmfPen) {
    if self.width == 0 || self.height == 0 {
      return;
    }
    let radius = (pen.width.max(1) / 2) as f64;
    let canvas = (0, 0, self.width as i32, self.height as i32);
    let (left, top, right, bottom) = self
      .clip_rect
      .map_or(canvas, |clip_rect| intersect_rects(canvas, clip_rect));
    if right <= left || bottom <= top {
      return;
    }
    let (x0, y0) = self.map_point(a);
    let (x1, y1) = self.map_point(b);
    let Some(((x0, y0), (x1, y1))) = clip_line_to_rect(
      (f64::from(x0), f64::from(y0)),
      (f64::from(x1), f64::from(y1)),
      (
        f64::from(left) - radius,
        f64::from(top) - radius,
        f64::from(right - 1) + radius,
        f64::from(bottom - 1) + radius,
      ),
    ) else {
      return;
    };
    let mut x0 = x0.round() as i32;
    let mut y0 = y0.round() as i32;
    let x1 = x1.round() as i32;
    let y1 = y1.round() as i32;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut error = dx + dy;
    loop {
      self.set_pen_pixel(x0, y0, pen);
      if x0 == x1 && y0 == y1 {
        break;
      }
      let e2 = 2 * error;
      if e2 >= dy {
        error += dy;
        x0 += sx;
      }
      if e2 <= dx {
        error += dx;
        y0 += sy;
      }
    }
  }

  fn set_pen_pixel(&mut self, x: i32, y: i32, pen: EmfPen) {
    let radius = (pen.width.max(1) / 2) as i32;
    for yy in y - radius..=y + radius {
      for xx in x - radius..=x + radius {
        self.set_vector_pixel(xx, yy, pen.color);
      }
    }
  }

  fn fill_rect(&mut self, left: i32, top: i32, right: i32, bottom: i32) {
    let points = [
      EmfPoint { x: left, y: top },
      EmfPoint { x: right, y: top },
      EmfPoint {
        x: right,
        y: bottom,
      },
      EmfPoint { x: left, y: bottom },
    ];
    self.fill_polygon(&points);
    self.draw_polyline(&points, true);
  }

  fn fill_solid_rect(&mut self, left: i32, top: i32, right: i32, bottom: i32, color: EmfColor) {
    let (mapped_left, mapped_top) = self.map_point(EmfPoint { x: left, y: top });
    let (mapped_right, mapped_bottom) = self.map_point(EmfPoint {
      x: right,
      y: bottom,
    });
    let left = mapped_left.min(mapped_right).floor().max(0.0) as i32;
    let top = mapped_top.min(mapped_bottom).floor().max(0.0) as i32;
    let right = mapped_left.max(mapped_right).ceil().min(self.width as f32) as i32;
    let bottom = mapped_top.max(mapped_bottom).ceil().min(self.height as f32) as i32;
    for y in top..bottom {
      for x in left..right {
        self.set_vector_pixel(x, y, color);
      }
    }
  }

  fn fill_ellipse(&mut self, left: i32, top: i32, right: i32, bottom: i32) {
    let steps = 72;
    let cx = (left + right) as f32 / 2.0;
    let cy = (top + bottom) as f32 / 2.0;
    let rx = (right - left).abs() as f32 / 2.0;
    let ry = (bottom - top).abs() as f32 / 2.0;
    let mut points = Vec::with_capacity(steps);
    for index in 0..steps {
      let theta = index as f32 * std::f32::consts::TAU / steps as f32;
      points.push(EmfPoint {
        x: (cx + theta.cos() * rx).round() as i32,
        y: (cy + theta.sin() * ry).round() as i32,
      });
    }
    self.fill_polygon(&points);
    self.draw_polyline(&points, true);
  }

  fn fill_ellipse_with_emf_plus_brush(
    &mut self,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    brush: &EmfPlusRenderBrush,
  ) {
    let steps = 72;
    let cx = (left + right) as f32 / 2.0;
    let cy = (top + bottom) as f32 / 2.0;
    let rx = (right - left).abs() as f32 / 2.0;
    let ry = (bottom - top).abs() as f32 / 2.0;
    let mut points = Vec::with_capacity(steps);
    for index in 0..steps {
      let theta = index as f32 * std::f32::consts::TAU / steps as f32;
      points.push(EmfPoint {
        x: (cx + theta.cos() * rx).round() as i32,
        y: (cy + theta.sin() * ry).round() as i32,
      });
    }
    self.fill_polygon_with_emf_plus_brush(&points, brush);
    self.draw_polyline(&points, true);
  }

  fn select_object(&mut self, object_id: u32) {
    match object_id {
      WHITE_BRUSH => {
        self.current_brush = Some(EmfColor {
          r: 255,
          g: 255,
          b: 255,
        })
      }
      BLACK_BRUSH => self.current_brush = Some(EmfColor { r: 0, g: 0, b: 0 }),
      NULL_BRUSH => self.current_brush = None,
      WHITE_PEN => {
        self.current_pen = Some(EmfPen {
          color: EmfColor {
            r: 255,
            g: 255,
            b: 255,
          },
          width: 1,
          transform_width: false,
        })
      }
      BLACK_PEN => {
        self.current_pen = Some(EmfPen {
          color: EmfColor { r: 0, g: 0, b: 0 },
          width: 1,
          transform_width: false,
        })
      }
      NULL_PEN => self.current_pen = None,
      _ => {
        if let Some(brush) = self.brush_colors.get(&object_id).copied() {
          self.current_brush = Some(brush);
        }
        if let Some(pen) = self.pens.get(&object_id).copied() {
          self.current_pen = pen;
        }
        if self.fonts.contains_key(&object_id) {
          self.current_font = Some(object_id);
        }
      }
    }
  }
}

fn decode_vector_emf_as_png(
  data: &[u8],
  options: RenderOptions,
) -> Result<DecodedMetafile, String> {
  let mut state = EmfVectorState::new_with_options(data, options)?;
  let mut pos = EMF_HEADER_SIZE;

  while pos + EMF_RECORD_HEADER_SIZE <= data.len() {
    let record_type = read_u32(data, pos)?;
    let record_size = read_u32(data, pos + 4)? as usize;
    if record_size < EMF_RECORD_HEADER_SIZE || pos + record_size > data.len() {
      return Err(format!(
        "invalid EMF record at offset {pos}: type=0x{record_type:08x} size={record_size}"
      ));
    }
    let mut consumed_following_record_size = 0usize;

    match record_type {
      EMR_SET_WINDOW_ORG_EX if record_size >= 16 => {
        state.window_org_x = read_i32(data, pos + 8)?;
        state.window_org_y = read_i32(data, pos + 12)?;
      }
      EMR_SET_WINDOW_EXT_EX if record_size >= 16 => {
        state.window_ext_x = read_i32(data, pos + 8)?.abs().max(1);
        state.window_ext_y = read_i32(data, pos + 12)?.abs().max(1);
      }
      EMR_SET_VIEWPORT_ORG_EX if record_size >= 16 => {
        state.viewport_org_x = read_i32(data, pos + 8)?;
        state.viewport_org_y = read_i32(data, pos + 12)?;
      }
      EMR_SET_VIEWPORT_EXT_EX if record_size >= 16 => {
        state.viewport_ext_x = read_i32(data, pos + 8)?;
        state.viewport_ext_y = read_i32(data, pos + 12)?;
      }
      EMR_SET_PIXEL_V if record_size >= 20 => {
        let (x, y) = state.map_point(EmfPoint {
          x: read_i32(data, pos + 8)?,
          y: read_i32(data, pos + 12)?,
        });
        state.set_pixel(
          x.round() as i32,
          y.round() as i32,
          read_color_ref(data, pos + 16)?,
        );
      }
      EMR_SET_ROP_2 if record_size >= 12 => {
        if let Some(operation) = u16::try_from(read_u32(data, pos + 8)?)
          .ok()
          .and_then(WmfBinaryRasterOperation::from_raw)
        {
          state.binary_raster_operation = operation;
        }
      }
      EMR_MOVE_TO_EX if record_size >= 16 => {
        state.current_pos = EmfPoint {
          x: read_i32(data, pos + 8)?,
          y: read_i32(data, pos + 12)?,
        };
      }
      EMR_LINE_TO if record_size >= 16 => {
        let next = EmfPoint {
          x: read_i32(data, pos + 8)?,
          y: read_i32(data, pos + 12)?,
        };
        state.draw_polyline(&[state.current_pos, next], false);
        state.current_pos = next;
      }
      EMR_SET_TEXT_COLOR if record_size >= 12 => {
        state.text_color = read_color_ref(data, pos + 8)?;
      }
      EMR_SET_TEXT_ALIGN if record_size >= 12 => {
        state.text_alignment =
          WmfTextAlignmentModeFlags::from_bits_retain(read_u32(data, pos + 8)? as u16);
      }
      EMR_SAVE_DC => state.save_state(),
      EMR_RESTORE_DC => state.restore_state(),
      EMR_SET_WORLD_TRANSFORM if record_size >= 32 => {
        state.world_transform = read_xform(data, pos + 8)?;
      }
      EMR_MODIFY_WORLD_TRANSFORM if record_size >= 36 => {
        let transform = read_xform(data, pos + 8)?;
        let mode = read_u32(data, pos + 32)?;
        state.world_transform = match mode {
          MWT_IDENTITY => EmfTransform::identity(),
          MWT_LEFTMULTIPLY => transform.multiply(state.world_transform),
          MWT_RIGHTMULTIPLY => state.world_transform.multiply(transform),
          MWT_SET => transform,
          _ => state.world_transform,
        };
      }
      EMR_CREATE_PEN if record_size >= 28 => {
        let object_id = read_u32(data, pos + 8)?;
        if object_id & ENHMETA_STOCK_OBJECT == 0 {
          let style = read_u32(data, pos + 12)?;
          let width = read_i32(data, pos + 16)?.unsigned_abs().max(1) as usize;
          state.pens.insert(
            object_id,
            emf_pen_from_style(
              style,
              EmfPen {
                color: read_color_ref(data, pos + 24)?,
                width,
                transform_width: false,
              },
            ),
          );
        }
      }
      EMR_CREATE_BRUSH_INDIRECT if record_size >= 24 => {
        let object_id = read_u32(data, pos + 8)?;
        state
          .brush_colors
          .insert(object_id, read_color_ref(data, pos + 16)?);
      }
      EMR_EXT_CREATE_PEN if record_size >= 56 => {
        let object_id = read_u32(data, pos + 8)?;
        if object_id & ENHMETA_STOCK_OBJECT == 0 {
          let style = read_u32(data, pos + 28)?;
          let width = read_u32(data, pos + 32)?.max(1) as usize;
          state.pens.insert(
            object_id,
            emf_pen_from_style(
              style,
              EmfPen {
                color: read_color_ref(data, pos + 40)?,
                width,
                transform_width: false,
              },
            ),
          );
        }
      }
      EMR_EXT_CREATE_FONT_INDIRECT_W if record_size >= 104 => {
        if let Some((object_id, font)) = read_logfont_object(data, pos, record_size)
          && object_id & ENHMETA_STOCK_OBJECT == 0
        {
          state.fonts.insert(object_id, font);
        }
      }
      EMR_SELECT_OBJECT if record_size >= 12 => {
        state.select_object(read_u32(data, pos + 8)?);
      }
      EMR_DELETE_OBJECT if record_size >= 12 => {
        let object_id = read_u32(data, pos + 8)?;
        state.brush_colors.remove(&object_id);
        state.pens.remove(&object_id);
        state.fonts.remove(&object_id);
        if state.current_font == Some(object_id) {
          state.current_font = None;
        }
      }
      EMR_POLYGON if record_size >= 28 => {
        if let Some(points) = read_points_i32(data, pos + 28, read_u32(data, pos + 24)? as usize) {
          state.fill_polygon(&points);
          state.draw_polyline(&points, true);
        }
      }
      EMR_POLYBEZIER if record_size >= 28 => {
        if let Some(points) = read_points_i32(data, pos + 28, read_u32(data, pos + 24)? as usize) {
          state.draw_polyline(&flatten_bezier_sequence(&points), false);
        }
      }
      EMR_POLYBEZIER_TO if record_size >= 28 => {
        if let Some(points) = read_points_i32(data, pos + 28, read_u32(data, pos + 24)? as usize) {
          let mut sequence = Vec::with_capacity(points.len() + 1);
          sequence.push(state.current_pos);
          sequence.extend_from_slice(&points);
          state.draw_polyline(&flatten_bezier_sequence(&sequence), false);
          if let Some(last) = points.last().copied() {
            state.current_pos = last;
          }
        }
      }
      EMR_POLYGON16 if record_size >= 28 => {
        if let Some(points) = read_points_i16(data, pos + 28, read_u32(data, pos + 24)? as usize) {
          state.fill_polygon(&points);
          state.draw_polyline(&points, true);
        }
      }
      EMR_POLYBEZIER16 if record_size >= 28 => {
        if let Some(points) = read_points_i16(data, pos + 28, read_u32(data, pos + 24)? as usize) {
          state.draw_polyline(&flatten_bezier_sequence(&points), false);
        }
      }
      EMR_POLYBEZIER_TO16 if record_size >= 28 => {
        if let Some(points) = read_points_i16(data, pos + 28, read_u32(data, pos + 24)? as usize) {
          let mut sequence = Vec::with_capacity(points.len() + 1);
          sequence.push(state.current_pos);
          sequence.extend_from_slice(&points);
          state.draw_polyline(&flatten_bezier_sequence(&sequence), false);
          if let Some(last) = points.last().copied() {
            state.current_pos = last;
          }
        }
      }
      EMR_POLYLINE if record_size >= 28 => {
        if let Some(points) = read_points_i32(data, pos + 28, read_u32(data, pos + 24)? as usize) {
          state.draw_polyline(&points, false);
        }
      }
      EMR_POLYLINE_TO if record_size >= 28 => {
        if let Some(points) = read_points_i32(data, pos + 28, read_u32(data, pos + 24)? as usize) {
          let mut sequence = Vec::with_capacity(points.len() + 1);
          sequence.push(state.current_pos);
          sequence.extend_from_slice(&points);
          state.draw_polyline(&sequence, false);
          if let Some(last) = points.last().copied() {
            state.current_pos = last;
          }
        }
      }
      EMR_POLYLINE16 if record_size >= 28 => {
        if let Some(points) = read_points_i16(data, pos + 28, read_u32(data, pos + 24)? as usize) {
          state.draw_polyline(&points, false);
        }
      }
      EMR_POLYLINE_TO16 if record_size >= 28 => {
        if let Some(points) = read_points_i16(data, pos + 28, read_u32(data, pos + 24)? as usize) {
          let mut sequence = Vec::with_capacity(points.len() + 1);
          sequence.push(state.current_pos);
          sequence.extend_from_slice(&points);
          state.draw_polyline(&sequence, false);
          if let Some(last) = points.last().copied() {
            state.current_pos = last;
          }
        }
      }
      EMR_POLYPOLYLINE if record_size >= 36 => {
        for points in read_poly_polygons_i32(data, pos, record_size)? {
          state.draw_polyline(&points, false);
        }
      }
      EMR_POLYPOLYGON if record_size >= 36 => {
        for points in read_poly_polygons_i32(data, pos, record_size)? {
          state.fill_polygon(&points);
          state.draw_polyline(&points, true);
        }
      }
      EMR_POLYPOLYLINE16 if record_size >= 36 => {
        for points in read_poly_polygons_i16(data, pos, record_size)? {
          state.draw_polyline(&points, false);
        }
      }
      EMR_POLYPOLYGON16 if record_size >= 36 => {
        for points in read_poly_polygons_i16(data, pos, record_size)? {
          state.fill_polygon(&points);
          state.draw_polyline(&points, true);
        }
      }
      EMR_RECTANGLE if record_size >= 24 => {
        state.fill_rect(
          read_i32(data, pos + 8)?,
          read_i32(data, pos + 12)?,
          read_i32(data, pos + 16)?,
          read_i32(data, pos + 20)?,
        );
      }
      EMR_ROUND_RECT if record_size >= 32 => {
        state.fill_rect(
          read_i32(data, pos + 8)?,
          read_i32(data, pos + 12)?,
          read_i32(data, pos + 16)?,
          read_i32(data, pos + 20)?,
        );
      }
      EMR_ELLIPSE if record_size >= 24 => {
        state.fill_ellipse(
          read_i32(data, pos + 8)?,
          read_i32(data, pos + 12)?,
          read_i32(data, pos + 16)?,
          read_i32(data, pos + 20)?,
        );
      }
      EMR_ARC if record_size >= 40 => {
        let rect = emf_arc_rect(data, pos)?;
        state.fill_arc_segment(
          rect,
          angle_from_emf_arc_point(rect, read_i32(data, pos + 24)?, read_i32(data, pos + 28)?),
          sweep_from_emf_arc_points(data, pos, rect)?,
          false,
        );
      }
      EMR_CHORD | EMR_PIE if record_size >= 40 => {
        let rect = emf_arc_rect(data, pos)?;
        state.fill_arc_segment(
          rect,
          angle_from_emf_arc_point(rect, read_i32(data, pos + 24)?, read_i32(data, pos + 28)?),
          sweep_from_emf_arc_points(data, pos, rect)?,
          true,
        );
      }
      EMR_EXT_TEXTOUT_W => {
        if let Some(text) = extract_emr_ext_text_out_w(data, pos, record_size)
          && let Some(text_record) = ext_text_record(data, pos, record_size)
        {
          let font = emf_current_font(&state);
          let advances = ext_text_advances(data, pos, record_size, text_record);
          state.draw_emf_text(
            text_record,
            &text,
            state.text_color,
            &font,
            advances.as_deref(),
          );
        }
      }
      EMR_EXT_TEXTOUT_A => {
        if let Some(text) = extract_emr_ext_text_out_a(data, pos, record_size)
          && let Some(text_record) = ext_text_record(data, pos, record_size)
        {
          let font = emf_current_font(&state);
          let advances = ext_text_advances(data, pos, record_size, text_record);
          state.draw_emf_text(
            text_record,
            &text,
            state.text_color,
            &font,
            advances.as_deref(),
          );
        }
      }
      EMR_BIT_BLT | EMR_STRETCH_BLT | EMR_SET_DIBITS_TO_DEVICE | EMR_STRETCH_DIBITS => {
        if let Some(next_record_size) =
          replay_masked_blt_pair(data, pos, record_type, record_size, &mut state)?
        {
          consumed_following_record_size = next_record_size;
        } else if let Some(target) = emf_bitmap_draw_target(data, pos, record_type, record_size)?
          && let Some(image) = cropped_emf_bitmap(data, pos, record_type, record_size, target)?
        {
          if let Some(rop) = target.raster_operation {
            state.draw_rgb_image_with_rop(
              target.dest_x,
              target.dest_y,
              target.dest_width,
              target.dest_height,
              &image,
              rop,
            );
          } else {
            state.draw_rgb_image(
              target.dest_x,
              target.dest_y,
              target.dest_width,
              target.dest_height,
              &image,
            );
          }
        }
      }
      EMR_COMMENT if record_size >= 16 => {
        process_emf_plus_comment(data, pos, record_size, &mut state)?;
      }
      EMR_EOF => break,
      _ => {}
    }

    pos += record_size + consumed_following_record_size;
  }

  Ok(DecodedMetafile {
    data: rgb_to_png(&state.rgb, state.width as u32, state.height as u32)?,
    content_type: "image/png",
  })
}

fn cropped_emf_bitmap(
  data: &[u8],
  record_offset: usize,
  record_type: u32,
  record_size: usize,
  target: EmfBitmapDrawTarget,
) -> Result<Option<RasterPixels>, String> {
  let Some(image) = decode_bitmap_record_as_rgb(data, record_type, record_offset, record_size)?
  else {
    return Ok(None);
  };
  Ok(Some(
    target
      .source_rect
      .and_then(|source_rect| crop_raster_pixels(&image, source_rect))
      .unwrap_or(image),
  ))
}

fn replay_masked_blt_pair(
  data: &[u8],
  record_offset: usize,
  record_type: u32,
  record_size: usize,
  state: &mut EmfVectorState,
) -> Result<Option<usize>, String> {
  if !matches!(record_type, EMR_BIT_BLT | EMR_STRETCH_BLT) {
    return Ok(None);
  }
  let Some(mask_target) = emf_bitmap_draw_target(data, record_offset, record_type, record_size)?
  else {
    return Ok(None);
  };
  if mask_target.raster_operation != Some(WmfTernaryRasterOperationCode::SRCAND) {
    return Ok(None);
  }

  let source_offset = record_offset + record_size;
  if source_offset + EMF_RECORD_HEADER_SIZE > data.len() {
    return Ok(None);
  }
  let source_type = read_u32(data, source_offset)?;
  if !matches!(source_type, EMR_BIT_BLT | EMR_STRETCH_BLT) {
    return Ok(None);
  }
  let source_record_size = read_u32(data, source_offset + 4)? as usize;
  if source_record_size < EMF_RECORD_HEADER_SIZE || source_offset + source_record_size > data.len()
  {
    return Ok(None);
  }
  let Some(source_target) =
    emf_bitmap_draw_target(data, source_offset, source_type, source_record_size)?
  else {
    return Ok(None);
  };
  if source_target.raster_operation != Some(WmfTernaryRasterOperationCode::SRCINVERT)
    || !same_bitmap_destination(mask_target, source_target)
  {
    return Ok(None);
  }
  let Some(mask) = cropped_emf_bitmap(data, record_offset, record_type, record_size, mask_target)?
  else {
    return Ok(None);
  };
  if !is_binary_monochrome_raster(&mask) {
    return Ok(None);
  }
  let Some(source) = cropped_emf_bitmap(
    data,
    source_offset,
    source_type,
    source_record_size,
    source_target,
  )?
  else {
    return Ok(None);
  };
  if mask.width != source.width || mask.height != source.height {
    return Ok(None);
  }

  state.draw_masked_rgb_image(
    mask_target.dest_x,
    mask_target.dest_y,
    mask_target.dest_width,
    mask_target.dest_height,
    &source,
    &mask,
  );
  Ok(Some(source_record_size))
}

#[derive(Clone, Debug)]
struct RasterPixels {
  width: usize,
  height: usize,
  rgb: Vec<u8>,
}

fn raster_color(image: &RasterPixels, x: usize, y: usize) -> EmfColor {
  let x = x.min(image.width.saturating_sub(1));
  let y = y.min(image.height.saturating_sub(1));
  let offset = (y * image.width + x) * RGB_BYTES_PER_PIXEL;
  EmfColor {
    r: image.rgb[offset],
    g: image.rgb[offset + 1],
    b: image.rgb[offset + 2],
  }
}

fn nearest_raster_index(destination: usize, destination_size: usize, source_size: usize) -> usize {
  if destination_size == 0 || source_size <= 1 {
    return 0;
  }
  // Sample at destination pixel centers, as GDI's COLORONCOLOR/nearest
  // StretchBlt mode does. Sampling from the leading edge biases duplicated
  // source columns toward the trailing side.
  let numerator = (destination as u128 * 2 + 1) * source_size as u128;
  (numerator / (destination_size as u128 * 2)).min((source_size - 1) as u128) as usize
}

fn is_binary_monochrome_raster(image: &RasterPixels) -> bool {
  image
    .rgb
    .chunks_exact(RGB_BYTES_PER_PIXEL)
    .all(|pixel| pixel[0] == pixel[1] && pixel[1] == pixel[2] && matches!(pixel[0], 0 | u8::MAX))
}

fn is_discrete_two_color_raster(image: &RasterPixels) -> bool {
  let mut colors = [[0u8; RGB_BYTES_PER_PIXEL]; 2];
  let mut color_count = 0;
  for pixel in image.rgb.chunks_exact(RGB_BYTES_PER_PIXEL) {
    let color = [pixel[0], pixel[1], pixel[2]];
    if colors[..color_count].contains(&color) {
      continue;
    }
    if color_count == colors.len() {
      return false;
    }
    colors[color_count] = color;
    color_count += 1;
  }
  true
}

fn bilinear_raster_color(
  image: &RasterPixels,
  x: usize,
  y: usize,
  target_width: usize,
  target_height: usize,
) -> EmfColor {
  if image.width == 0 || image.height == 0 || target_width == 0 || target_height == 0 {
    return EmfColor { r: 0, g: 0, b: 0 };
  }
  let source_coordinate = |target: usize, source_extent: usize, target_extent: usize| {
    ((target as f32 + 0.5) * source_extent as f32 / target_extent as f32 - 0.5)
      .clamp(0.0, source_extent.saturating_sub(1) as f32)
  };
  let source_x = source_coordinate(x, image.width, target_width);
  let source_y = source_coordinate(y, image.height, target_height);
  let x0 = source_x.floor() as usize;
  let y0 = source_y.floor() as usize;
  let x1 = (x0 + 1).min(image.width - 1);
  let y1 = (y0 + 1).min(image.height - 1);
  let fraction_x = source_x - x0 as f32;
  let fraction_y = source_y - y0 as f32;
  let top_left = raster_color(image, x0, y0);
  let top_right = raster_color(image, x1, y0);
  let bottom_left = raster_color(image, x0, y1);
  let bottom_right = raster_color(image, x1, y1);
  let channel = |top_left: u8, top_right: u8, bottom_left: u8, bottom_right: u8| {
    let top = f32::from(top_left) + (f32::from(top_right) - f32::from(top_left)) * fraction_x;
    let bottom =
      f32::from(bottom_left) + (f32::from(bottom_right) - f32::from(bottom_left)) * fraction_x;
    (top + (bottom - top) * fraction_y)
      .round()
      .clamp(0.0, f32::from(u8::MAX)) as u8
  };
  EmfColor {
    r: channel(top_left.r, top_right.r, bottom_left.r, bottom_right.r),
    g: channel(top_left.g, top_right.g, bottom_left.g, bottom_right.g),
    b: channel(top_left.b, top_right.b, bottom_left.b, bottom_right.b),
  }
}

/// Samples the independently filtered color branch of a GDI+ metafile blit.
///
/// `Graphics::DrawImage(Metafile, destination)` maps the first destination
/// sample to the first source sample and advances by `(source - 1) / target`.
/// This differs from the half-pixel convention used by ordinary decoded
/// images. A 32-sample 0,8,..,248 ramp stretched to 66 samples therefore
/// begins `0,4,8,11,15` and ends at 244 in Windows GDI+ playback.
fn gdi_plus_bilinear_raster_color(
  image: &RasterPixels,
  x: usize,
  y: usize,
  target_width: usize,
  target_height: usize,
) -> EmfColor {
  if image.width == 0 || image.height == 0 || target_width == 0 || target_height == 0 {
    return EmfColor { r: 0, g: 0, b: 0 };
  }
  let source_coordinate = |target: usize, source_extent: usize, target_extent: usize| {
    target as f32 * source_extent.saturating_sub(1) as f32 / target_extent as f32
  };
  let source_x = source_coordinate(x, image.width, target_width);
  let source_y = source_coordinate(y, image.height, target_height);
  let x0 = source_x.floor() as usize;
  let y0 = source_y.floor() as usize;
  let x1 = (x0 + 1).min(image.width - 1);
  let y1 = (y0 + 1).min(image.height - 1);
  let fraction_x = source_x - x0 as f32;
  let fraction_y = source_y - y0 as f32;
  let top_left = raster_color(image, x0, y0);
  let top_right = raster_color(image, x1, y0);
  let bottom_left = raster_color(image, x0, y1);
  let bottom_right = raster_color(image, x1, y1);
  let channel = |top_left: u8, top_right: u8, bottom_left: u8, bottom_right: u8| {
    let top = f32::from(top_left) + (f32::from(top_right) - f32::from(top_left)) * fraction_x;
    let bottom =
      f32::from(bottom_left) + (f32::from(bottom_right) - f32::from(bottom_left)) * fraction_x;
    (top + (bottom - top) * fraction_y)
      .round()
      .clamp(0.0, f32::from(u8::MAX)) as u8
  };
  EmfColor {
    r: channel(top_left.r, top_right.r, bottom_left.r, bottom_right.r),
    g: channel(top_left.g, top_right.g, bottom_left.g, bottom_right.g),
    b: channel(top_left.b, top_right.b, bottom_left.b, bottom_right.b),
  }
}

fn checkerboard_average_color(image: &RasterPixels) -> Option<EmfColor> {
  if image.width < 2 || image.height < 2 {
    return None;
  }
  let color_at = |x: usize, y: usize| {
    let offset = (y * image.width + x) * RGB_BYTES_PER_PIXEL;
    EmfColor {
      r: image.rgb[offset],
      g: image.rgb[offset + 1],
      b: image.rgb[offset + 2],
    }
  };
  let first = color_at(0, 0);
  let second = color_at(1, 0);
  if first == second {
    return None;
  }
  for y in 0..image.height {
    for x in 0..image.width {
      let expected = if (x + y).is_multiple_of(2) {
        first
      } else {
        second
      };
      if color_at(x, y) != expected {
        return None;
      }
    }
  }
  Some(EmfColor {
    r: ((u16::from(first.r) + u16::from(second.r)) / 2) as u8,
    g: ((u16::from(first.g) + u16::from(second.g)) / 2) as u8,
    b: ((u16::from(first.b) + u16::from(second.b)) / 2) as u8,
  })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EmfBitmapDrawTarget {
  dest_x: i32,
  dest_y: i32,
  dest_width: i32,
  dest_height: i32,
  raster_operation: Option<WmfTernaryRasterOperationCode>,
  source_rect: Option<(i32, i32, i32, i32)>,
}

fn same_bitmap_destination(first: EmfBitmapDrawTarget, second: EmfBitmapDrawTarget) -> bool {
  first.dest_x == second.dest_x
    && first.dest_y == second.dest_y
    && first.dest_width == second.dest_width
    && first.dest_height == second.dest_height
}

#[derive(Clone, Debug)]
struct WmfSavedState {
  window_org_x: i32,
  window_org_y: i32,
  window_ext_x: i32,
  window_ext_y: i32,
  viewport_org_x: i32,
  viewport_org_y: i32,
  viewport_ext_x: i32,
  viewport_ext_y: i32,
  current_brush: Option<EmfColor>,
  current_pen: Option<EmfPen>,
  current_pos: EmfPoint,
  text_color: EmfColor,
  binary_raster_operation: WmfBinaryRasterOperation,
  background_color: EmfColor,
  current_pattern_brush: Option<WmfPatternBrush>,
  current_font: WmfTextFont,
  text_alignment: WmfTextAlignmentModeFlags,
}

#[derive(Clone, Debug)]
struct WmfPatternBrush {
  image: RasterPixels,
  use_dc_colors: bool,
  filtered_color: Option<EmfColor>,
}

#[derive(Clone, Debug)]
enum WmfRenderObject {
  Pen(Option<EmfPen>),
  Brush(Option<EmfColor>),
  PatternBrush(WmfPatternBrush),
  Font(WmfTextFont),
  Unsupported,
}

struct WmfRenderState {
  canvas: EmfVectorState,
  objects: Vec<Option<WmfRenderObject>>,
  current_pos: EmfPoint,
  text_color: EmfColor,
  background_color: EmfColor,
  current_pattern_brush: Option<WmfPatternBrush>,
  current_font: WmfTextFont,
  text_alignment: WmfTextAlignmentModeFlags,
  saved: Vec<WmfSavedState>,
}

impl WmfRenderState {
  fn new(metafile: &WmfMetafileRef<'_>, options: RenderOptions) -> Result<Self, String> {
    let (window_org_x, window_org_y, window_ext_x, window_ext_y) = wmf_initial_window(metafile);
    let natural_width = window_ext_x.unsigned_abs().max(1) as usize;
    let natural_height = window_ext_y.unsigned_abs().max(1) as usize;
    let (width, height) = options.resolved_canvas_size(natural_width, natural_height);
    let output_scale_x = width as f32 / natural_width as f32;
    let output_scale_y = height as f32 / natural_height as f32;
    let object_count = metafile.header.number_of_objects as usize;
    let background_color = options.background_color.unwrap_or([255; 3]);
    let mut rgb = vec![0; width * height * RGB_BYTES_PER_PIXEL];
    for pixel in rgb.chunks_exact_mut(RGB_BYTES_PER_PIXEL) {
      pixel.copy_from_slice(&background_color);
    }

    Ok(Self {
      canvas: EmfVectorState {
        width,
        height,
        output_scale_x,
        output_scale_y,
        window_org_x,
        window_org_y,
        window_ext_x: nonzero_mapping_extent(window_ext_x),
        window_ext_y: nonzero_mapping_extent(window_ext_y),
        viewport_org_x: 0,
        viewport_org_y: 0,
        viewport_ext_x: natural_width as i32,
        viewport_ext_y: natural_height as i32,
        world_transform: EmfTransform::identity(),
        brush_colors: std::collections::HashMap::new(),
        pens: std::collections::HashMap::new(),
        fonts: std::collections::HashMap::new(),
        current_brush: Some(EmfColor {
          r: 255,
          g: 255,
          b: 255,
        }),
        current_pen: Some(EmfPen {
          color: EmfColor { r: 0, g: 0, b: 0 },
          width: 1,
          transform_width: false,
        }),
        current_font: None,
        current_pos: EmfPoint { x: 0, y: 0 },
        text_color: EmfColor { r: 0, g: 0, b: 0 },
        binary_raster_operation: WmfBinaryRasterOperation::CopyPen,
        text_alignment: WmfTextAlignmentModeFlags::empty(),
        clip_rect: None,
        clip_mask: None,
        saved_states: Vec::new(),
        emf_plus_objects: Vec::new(),
        emf_plus_object_assembler: EmfPlusObjectAssembler::default(),
        font_cache: RenderFontCache::load(),
        rgb,
      },
      objects: vec![None; object_count],
      current_pos: EmfPoint { x: 0, y: 0 },
      text_color: EmfColor { r: 0, g: 0, b: 0 },
      background_color: EmfColor {
        r: 255,
        g: 255,
        b: 255,
      },
      current_pattern_brush: None,
      current_font: WmfTextFont {
        height: 12,
        family: None,
        weight: 400,
        italic: false,
      },
      text_alignment: WmfTextAlignmentModeFlags::empty(),
      saved: Vec::new(),
    })
  }

  fn insert_object(&mut self, object: WmfRenderObject) {
    if let Some(slot) = self.objects.iter_mut().find(|slot| slot.is_none()) {
      *slot = Some(object);
    } else {
      self.objects.push(Some(object));
    }
  }

  fn save_dc(&mut self) {
    self.saved.push(WmfSavedState {
      window_org_x: self.canvas.window_org_x,
      window_org_y: self.canvas.window_org_y,
      window_ext_x: self.canvas.window_ext_x,
      window_ext_y: self.canvas.window_ext_y,
      viewport_org_x: self.canvas.viewport_org_x,
      viewport_org_y: self.canvas.viewport_org_y,
      viewport_ext_x: self.canvas.viewport_ext_x,
      viewport_ext_y: self.canvas.viewport_ext_y,
      current_brush: self.canvas.current_brush,
      current_pen: self.canvas.current_pen,
      current_pos: self.current_pos,
      text_color: self.text_color,
      binary_raster_operation: self.canvas.binary_raster_operation,
      background_color: self.background_color,
      current_pattern_brush: self.current_pattern_brush.clone(),
      current_font: self.current_font.clone(),
      text_alignment: self.text_alignment,
    });
  }

  fn restore_dc(&mut self) {
    let Some(saved) = self.saved.pop() else {
      return;
    };
    self.canvas.window_org_x = saved.window_org_x;
    self.canvas.window_org_y = saved.window_org_y;
    self.canvas.window_ext_x = saved.window_ext_x;
    self.canvas.window_ext_y = saved.window_ext_y;
    self.canvas.viewport_org_x = saved.viewport_org_x;
    self.canvas.viewport_org_y = saved.viewport_org_y;
    self.canvas.viewport_ext_x = saved.viewport_ext_x;
    self.canvas.viewport_ext_y = saved.viewport_ext_y;
    self.canvas.current_brush = saved.current_brush;
    self.canvas.current_pen = saved.current_pen;
    self.current_pos = saved.current_pos;
    self.text_color = saved.text_color;
    self.canvas.binary_raster_operation = saved.binary_raster_operation;
    self.background_color = saved.background_color;
    self.current_pattern_brush = saved.current_pattern_brush;
    self.current_font = saved.current_font;
    self.text_alignment = saved.text_alignment;
  }

  fn select_object(&mut self, index: u16) {
    let Some(Some(object)) = self.objects.get(index as usize).cloned() else {
      return;
    };
    match object {
      WmfRenderObject::Pen(pen) => self.canvas.current_pen = pen,
      WmfRenderObject::Brush(brush) => {
        self.canvas.current_brush = brush;
        self.current_pattern_brush = None;
      }
      WmfRenderObject::PatternBrush(pattern) => {
        self.current_pattern_brush = Some(pattern);
      }
      WmfRenderObject::Font(font) => self.current_font = font,
      WmfRenderObject::Unsupported => {}
    }
  }

  fn delete_object(&mut self, index: u16) {
    if let Some(slot) = self.objects.get_mut(index as usize) {
      *slot = None;
    }
  }

  fn text_baseline_y(&self, reference_y: i16) -> i32 {
    let reference_y = i32::from(reference_y);
    if self
      .text_alignment
      .contains(WmfTextAlignmentModeFlags::BASELINE)
      || self
        .text_alignment
        .contains(WmfTextAlignmentModeFlags::BOTTOM)
    {
      reference_y
    } else {
      // [MS-WMF] 2.1.2.3 defines the all-zero vertical mode as TA_TOP.
      // Our outline painter takes a baseline, so advance by the logical
      // character-cell height before applying the device mapping.
      reference_y.saturating_add(self.current_font.height.unsigned_abs() as i32)
    }
  }

  fn fill_pattern_rect(
    &mut self,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    rop: WmfTernaryRasterOperationCode,
  ) -> bool {
    let Some(pattern) = self.current_pattern_brush.as_ref() else {
      return false;
    };
    let (mapped_left, mapped_top) = self.canvas.map_point(EmfPoint { x: left, y: top });
    let (mapped_right, mapped_bottom) = self.canvas.map_point(EmfPoint {
      x: right,
      y: bottom,
    });
    let left = mapped_left.min(mapped_right).round().max(0.0) as i32;
    let top = mapped_top.min(mapped_bottom).round().max(0.0) as i32;
    let right = mapped_left
      .max(mapped_right)
      .round()
      .min(self.canvas.width as f32) as i32;
    let bottom = mapped_top
      .max(mapped_bottom)
      .round()
      .min(self.canvas.height as f32) as i32;
    for y in top..bottom {
      for x in left..right {
        let pattern_x = x.rem_euclid(pattern.image.width as i32) as usize;
        let pattern_y = y.rem_euclid(pattern.image.height as i32) as usize;
        let offset = (pattern_y * pattern.image.width + pattern_x) * RGB_BYTES_PER_PIXEL;
        let stored = pattern.filtered_color.unwrap_or(EmfColor {
          r: pattern.image.rgb[offset],
          g: pattern.image.rgb[offset + 1],
          b: pattern.image.rgb[offset + 2],
        });
        let brush = if pattern.use_dc_colors {
          if u16::from(stored.r) + u16::from(stored.g) + u16::from(stored.b) < 3 * 128 {
            self.text_color
          } else {
            self.background_color
          }
        } else {
          stored
        };
        if let Some(color) = self
          .canvas
          .apply_raster_op_with_pattern(x, y, brush, brush, rop)
        {
          self.canvas.set_pixel(x, y, color);
        }
      }
    }
    true
  }
}

fn decode_wmf_as_raster(
  data: &[u8],
  options: RenderOptions,
) -> Result<Option<DecodedMetafile>, String> {
  if !crate::wmf::looks_like_wmf(data) {
    return Ok(None);
  }

  let metafile = WmfMetafileRef::from_bytes(data).map_err(|err| err.to_string())?;
  let mut state = WmfRenderState::new(&metafile, options)?;

  for record in metafile.records() {
    // Compatibility-mode parsing preserves producer-specific and malformed
    // records so later valid drawing commands remain usable. Rendering must
    // follow the same recovery rule: one unsupported device escape must not
    // discard the entire preview that was already replayed.
    let Ok(parsed) = record.parse_data() else {
      continue;
    };
    match parsed {
      WmfRecordData::Eof(_) => break,
      WmfRecordData::SaveDc => state.save_dc(),
      WmfRecordData::RestoreDc(_) => state.restore_dc(),
      WmfRecordData::SetWindowOrg(value) => {
        state.canvas.window_org_x = i32::from(value.x);
        state.canvas.window_org_y = i32::from(value.y);
      }
      WmfRecordData::SetWindowExt(value) => {
        state.canvas.window_ext_x = nonzero_mapping_extent(i32::from(value.x));
        state.canvas.window_ext_y = nonzero_mapping_extent(i32::from(value.y));
      }
      WmfRecordData::SetViewportOrg(value) => {
        state.canvas.viewport_org_x = i32::from(value.x);
        state.canvas.viewport_org_y = i32::from(value.y);
      }
      WmfRecordData::SetViewportExt(value) => {
        state.canvas.viewport_ext_x = nonzero_mapping_extent(i32::from(value.x));
        state.canvas.viewport_ext_y = nonzero_mapping_extent(i32::from(value.y));
      }
      WmfRecordData::IntersectClipRect(value) => {
        state.canvas.set_clip_rect_logical(
          i32::from(value.left),
          i32::from(value.top),
          i32::from(value.right),
          i32::from(value.bottom),
        );
      }
      WmfRecordData::ExcludeClipRect(_) => {}
      WmfRecordData::SetTextColor(value) => {
        state.text_color = color_ref_to_emf(value.color);
      }
      WmfRecordData::SetRop2(value) => {
        if let Some(operation) = value.binary_raster_operation_kind() {
          state.canvas.binary_raster_operation = operation;
        }
      }
      WmfRecordData::SetTextAlign(value) => {
        state.text_alignment = value.text_alignment_flags();
      }
      WmfRecordData::SetBkColor(value) => {
        state.background_color = color_ref_to_emf(value.color);
      }
      WmfRecordData::OffsetWindowOrg(value) => {
        state.canvas.window_org_x += i32::from(value.x);
        state.canvas.window_org_y += i32::from(value.y);
      }
      WmfRecordData::OffsetViewportOrg(value) => {
        state.canvas.viewport_org_x += i32::from(value.x);
        state.canvas.viewport_org_y += i32::from(value.y);
      }
      WmfRecordData::CreatePenIndirect(value) => {
        let line_style = WmfPenLineStyle::from_raw(value.pen.pen_line_style_raw());
        let pen = if line_style == Some(WmfPenLineStyle::Null) {
          None
        } else {
          Some(EmfPen {
            color: color_ref_to_emf(value.pen.color_ref),
            width: i32::from(value.pen.width.x).unsigned_abs().max(1) as usize,
            transform_width: false,
          })
        };
        state.insert_object(WmfRenderObject::Pen(pen));
      }
      WmfRecordData::CreateBrushIndirect(value) => {
        let brush = match value.brush_style_kind() {
          Some(WmfBrushStyle::Null) => None,
          _ => Some(color_ref_to_emf(value.color_ref)),
        };
        state.insert_object(WmfRenderObject::Brush(brush));
      }
      WmfRecordData::CreateFontIndirect(value) => {
        state.insert_object(WmfRenderObject::Font(wmf_text_font(&value)));
      }
      WmfRecordData::CreatePatternBrush(value) => {
        let pattern = value
          .bitmap16()
          .ok()
          .and_then(|bitmap| bitmap.to_bytes().ok())
          .and_then(|bytes| bitmap16_to_rgb(&bytes).ok().flatten())
          .map(|image| {
            let filtered_color = options
              .filter_high_frequency_pattern_brushes
              .then(|| checkerboard_average_color(&image))
              .flatten();
            WmfPatternBrush {
              image,
              use_dc_colors: value.bitmap.bits_pixel == 1,
              filtered_color,
            }
          });
        state.insert_object(
          pattern
            .map(WmfRenderObject::PatternBrush)
            .unwrap_or(WmfRenderObject::Unsupported),
        );
      }
      WmfRecordData::DibCreatePatternBrush(value) => {
        let pattern = value
          .color_usage_kind()
          .and_then(|usage| {
            packed_dib_to_rgb_with_palette_override(
              &value.target,
              usage,
              options.monochrome_dib_palette_override,
            )
            .ok()
            .flatten()
          })
          .map(|image| {
            let filtered_color = options
              .filter_high_frequency_pattern_brushes
              .then(|| checkerboard_average_color(&image))
              .flatten();
            WmfPatternBrush {
              image,
              // DIB pattern brushes retain their color table on a color
              // playback surface. The DC text/background substitution applies
              // only when GDI realizes the brush into a monochrome target.
              use_dc_colors: false,
              filtered_color,
            }
          });
        state.insert_object(
          pattern
            .map(WmfRenderObject::PatternBrush)
            .unwrap_or(WmfRenderObject::Unsupported),
        );
      }
      WmfRecordData::CreatePalette(_) | WmfRecordData::CreateRegion(_) => {
        state.insert_object(WmfRenderObject::Unsupported);
      }
      WmfRecordData::SelectObject(value) => state.select_object(value.index),
      WmfRecordData::DeleteObject(value) => state.delete_object(value.index),
      WmfRecordData::MoveTo(value) => {
        state.current_pos = EmfPoint {
          x: i32::from(value.x),
          y: i32::from(value.y),
        };
      }
      WmfRecordData::LineTo(value) => {
        let next = EmfPoint {
          x: i32::from(value.x),
          y: i32::from(value.y),
        };
        state
          .canvas
          .draw_polyline(&[state.current_pos, next], false);
        state.current_pos = next;
      }
      WmfRecordData::SetPixel(value) => {
        let (x, y) = state.canvas.map_point(EmfPoint {
          x: i32::from(value.x),
          y: i32::from(value.y),
        });
        state.canvas.set_pixel(
          x.round() as i32,
          y.round() as i32,
          color_ref_to_emf(value.color),
        );
      }
      WmfRecordData::Polygon(value) => {
        let points = value
          .points
          .iter()
          .map(|point| EmfPoint {
            x: i32::from(point.x),
            y: i32::from(point.y),
          })
          .collect::<Vec<_>>();
        state.canvas.fill_polygon(&points);
        state.canvas.draw_polyline(&points, true);
      }
      WmfRecordData::Polyline(value) => {
        let points = value
          .points
          .iter()
          .map(|point| EmfPoint {
            x: i32::from(point.x),
            y: i32::from(point.y),
          })
          .collect::<Vec<_>>();
        state.canvas.draw_polyline(&points, false);
      }
      WmfRecordData::PolyPolygon(value) => {
        let mut cursor = 0usize;
        for count in value.points_per_polygon {
          let end = cursor
            .saturating_add(count as usize)
            .min(value.points.len());
          let points = value.points[cursor..end]
            .iter()
            .map(|point| EmfPoint {
              x: i32::from(point.x),
              y: i32::from(point.y),
            })
            .collect::<Vec<_>>();
          state.canvas.fill_polygon(&points);
          state.canvas.draw_polyline(&points, true);
          cursor = end;
        }
      }
      WmfRecordData::Rectangle(value) => state.canvas.fill_rect(
        i32::from(value.left),
        i32::from(value.top),
        i32::from(value.right),
        i32::from(value.bottom),
      ),
      WmfRecordData::RoundRect(value) => state.canvas.fill_rect(
        i32::from(value.left),
        i32::from(value.top),
        i32::from(value.right),
        i32::from(value.bottom),
      ),
      WmfRecordData::Ellipse(value) => state.canvas.fill_ellipse(
        i32::from(value.left),
        i32::from(value.top),
        i32::from(value.right),
        i32::from(value.bottom),
      ),
      WmfRecordData::Arc(value) => state.canvas.fill_arc_segment(
        (
          i32::from(value.left),
          i32::from(value.top),
          i32::from(value.right),
          i32::from(value.bottom),
        ),
        angle_from_arc_point(value, value.x_radial_1, value.y_radial_1),
        sweep_from_arc_points(value),
        false,
      ),
      WmfRecordData::Chord(value) | WmfRecordData::Pie(value) => state.canvas.fill_arc_segment(
        (
          i32::from(value.left),
          i32::from(value.top),
          i32::from(value.right),
          i32::from(value.bottom),
        ),
        angle_from_arc_point(value, value.x_radial_1, value.y_radial_1),
        sweep_from_arc_points(value),
        true,
      ),
      WmfRecordData::TextOut(value) => {
        let text = single_byte_text(&value.string);
        let baseline_y = state.text_baseline_y(value.y_start);
        state.canvas.draw_text_with_font(
          i32::from(value.x_start),
          baseline_y,
          &text,
          state.text_color,
          &state.current_font,
        );
      }
      WmfRecordData::ExtTextOut(value) => {
        if let Some(rectangle) = value.rectangle
          && value.options.contains(WmfExtTextOutOptions::OPAQUE)
        {
          // [MS-WMF] 2.1.2.2: ETO_OPAQUE fills the application-defined
          // rectangle with the playback DC's current background color.
          state.canvas.fill_solid_rect(
            i32::from(rectangle.left),
            i32::from(rectangle.top),
            i32::from(rectangle.right),
            i32::from(rectangle.bottom),
            state.background_color,
          );
        }

        let text = single_byte_text(&value.string);
        let baseline_y = state.text_baseline_y(value.y);
        let saved_clip = value
          .rectangle
          .filter(|_| value.options.contains(WmfExtTextOutOptions::CLIPPED))
          .map(|rectangle| {
            let saved = (state.canvas.clip_rect, state.canvas.clip_mask.clone());
            state.canvas.set_clip_rect_device(
              {
                let (left, top) = state.canvas.map_point(EmfPoint {
                  x: i32::from(rectangle.left),
                  y: i32::from(rectangle.top),
                });
                let (right, bottom) = state.canvas.map_point(EmfPoint {
                  x: i32::from(rectangle.right),
                  y: i32::from(rectangle.bottom),
                });
                (
                  left.min(right).floor() as i32,
                  top.min(bottom).floor() as i32,
                  left.max(right).ceil() as i32,
                  top.max(bottom).ceil() as i32,
                )
              },
              1,
            );
            saved
          });
        state.canvas.draw_text_with_font(
          i32::from(value.x),
          baseline_y,
          &text,
          state.text_color,
          &state.current_font,
        );
        if let Some((clip_rect, clip_mask)) = saved_clip {
          state.canvas.clip_rect = clip_rect;
          state.canvas.clip_mask = clip_mask;
        }
      }
      WmfRecordData::PatBlt(value) => {
        let left = i32::from(value.x_left);
        let top = i32::from(value.y_left);
        let right = left + i32::from(value.width);
        let bottom = top + i32::from(value.height);
        let rop = value.raster_operation_code();
        if !state.fill_pattern_rect(left, top, right, bottom, rop) {
          state
            .canvas
            .fill_rect_with_rop(left, top, right, bottom, rop);
        }
      }
      WmfRecordData::StretchDib(value) => {
        if let Some(color_usage) = value.color_usage_kind()
          && let Some(image) = packed_dib_to_rgb(&value.dib, color_usage)?
        {
          state.canvas.draw_rgb_image_with_rop(
            i32::from(value.x_dest),
            i32::from(value.y_dest),
            i32::from(value.dest_width),
            i32::from(value.dest_height),
            &image,
            value.raster_operation_code(),
          );
        }
      }
      WmfRecordData::SetDibToDev(value) => {
        if let Some(color_usage) = value.color_usage_kind()
          && let Some(image) = packed_dib_to_rgb(&value.dib, color_usage)?
        {
          state.canvas.draw_rgb_image(
            i32::from(value.x_dest),
            i32::from(value.y_dest),
            i32::from(value.width),
            i32::from(value.height),
            &image,
          );
        }
      }
      WmfRecordData::DibBitBlt(value) => {
        if let Some(bytes) = value.target.source_bytes()
          && let Some(image) = packed_dib_to_rgb(bytes, DibColorUsage::RgbColors)?
        {
          state.canvas.draw_rgb_image_with_rop(
            i32::from(value.x_dest),
            i32::from(value.y_dest),
            i32::from(value.width),
            i32::from(value.height),
            &image,
            value.raster_operation_code(),
          );
        }
      }
      WmfRecordData::DibStretchBlt(value) => {
        if let Some(bytes) = value.target.source_bytes()
          && let Some(image) = packed_dib_to_rgb(bytes, DibColorUsage::RgbColors)?
        {
          state.canvas.draw_rgb_image_with_rop(
            i32::from(value.x_dest),
            i32::from(value.y_dest),
            i32::from(value.dest_width),
            i32::from(value.dest_height),
            &image,
            value.raster_operation_code(),
          );
        }
      }
      WmfRecordData::BitBlt(value) => {
        if let Some(bytes) = value.target.source_bytes()
          && let Ok(Some(image)) = bitmap16_to_rgb(bytes)
        {
          state.canvas.draw_rgb_image_with_rop(
            i32::from(value.x_dest),
            i32::from(value.y_dest),
            i32::from(value.width),
            i32::from(value.height),
            &image,
            value.raster_operation_code(),
          );
        }
      }
      WmfRecordData::StretchBlt(value) => {
        if let Some(bytes) = value.target.source_bytes()
          && let Ok(Some(image)) = bitmap16_to_rgb(bytes)
        {
          state.canvas.draw_rgb_image_with_rop(
            i32::from(value.x_dest),
            i32::from(value.y_dest),
            i32::from(value.dest_width),
            i32::from(value.dest_height),
            &image,
            value.raster_operation_code(),
          );
        }
      }
      WmfRecordData::Escape(value) => {
        if let Ok(WmfEscapeData::EnhancedMetafile {
          enhanced_metafile_data,
          ..
        }) = value.typed_data()
          && let Some(raster) = decode_emf_as_raster(enhanced_metafile_data, options, false)?
          && let Some(image) = decoded_raster_to_rgb(&raster)?
        {
          state.canvas.draw_rgb_image(
            state.canvas.window_org_x,
            state.canvas.window_org_y,
            state.canvas.window_ext_x,
            state.canvas.window_ext_y,
            &image,
          );
        }
      }
      _ => {}
    }
  }

  Ok(Some(DecodedMetafile {
    data: rgb_to_png(
      &state.canvas.rgb,
      state.canvas.width as u32,
      state.canvas.height as u32,
    )?,
    content_type: "image/png",
  }))
}

fn wmf_initial_window(metafile: &WmfMetafileRef<'_>) -> (i32, i32, i32, i32) {
  if let Some(placeable) = &metafile.placeable_header {
    return (
      i32::from(placeable.left),
      i32::from(placeable.top),
      placeable.bounding_box_width().abs().max(1),
      placeable.bounding_box_height().abs().max(1),
    );
  }

  let mut org_x = 0;
  let mut org_y = 0;
  let mut ext_x = DEFAULT_RENDER_WIDTH as i32;
  let mut ext_y = DEFAULT_RENDER_HEIGHT as i32;
  for record in metafile.records() {
    match record.parse_data() {
      Ok(WmfRecordData::SetWindowOrg(value)) => {
        org_x = i32::from(value.x);
        org_y = i32::from(value.y);
      }
      Ok(WmfRecordData::SetWindowExt(value)) => {
        ext_x = nonzero_mapping_extent(i32::from(value.x));
        ext_y = nonzero_mapping_extent(i32::from(value.y));
        break;
      }
      Ok(WmfRecordData::Eof(_)) => break,
      _ => {}
    }
  }
  (org_x, org_y, ext_x, ext_y)
}

fn emf_bitmap_draw_target(
  data: &[u8],
  record_offset: usize,
  record_type: u32,
  record_size: usize,
) -> Result<Option<EmfBitmapDrawTarget>, String> {
  let min_size = match record_type {
    EMR_BIT_BLT => EMR_BLT_BITS_SIZE_OFFSET + 4,
    EMR_STRETCH_BLT => EMR_STRETCH_BLT_SOURCE_HEIGHT_OFFSET + 4,
    EMR_SET_DIBITS_TO_DEVICE => EMR_BITMAP_BITS_SIZE_OFFSET + 4,
    EMR_STRETCH_DIBITS => EMR_STRETCH_DIBITS_DEST_HEIGHT_OFFSET + 4,
    _ => return Ok(None),
  };
  if record_size < min_size {
    return Ok(None);
  }

  let dest_x = read_i32(data, record_offset + EMR_BITMAP_DEST_X_OFFSET)?;
  let dest_y = read_i32(data, record_offset + EMR_BITMAP_DEST_Y_OFFSET)?;
  let (dest_width, dest_height, raster_operation, source_rect) = match record_type {
    EMR_BIT_BLT => (
      read_i32(data, record_offset + EMR_BLT_DEST_WIDTH_OFFSET)?,
      read_i32(data, record_offset + EMR_BLT_DEST_HEIGHT_OFFSET)?,
      Some(emf_ternary_raster_operation(read_u32(
        data,
        record_offset + EMR_BLT_ROP_OFFSET,
      )?)),
      None,
    ),
    EMR_STRETCH_BLT => (
      read_i32(data, record_offset + EMR_BLT_DEST_WIDTH_OFFSET)?,
      read_i32(data, record_offset + EMR_BLT_DEST_HEIGHT_OFFSET)?,
      Some(emf_ternary_raster_operation(read_u32(
        data,
        record_offset + EMR_BLT_ROP_OFFSET,
      )?)),
      Some((
        read_i32(data, record_offset + EMR_BLT_SOURCE_X_OFFSET)?,
        read_i32(data, record_offset + EMR_BLT_SOURCE_Y_OFFSET)?,
        read_i32(data, record_offset + EMR_STRETCH_BLT_SOURCE_WIDTH_OFFSET)?,
        read_i32(data, record_offset + EMR_STRETCH_BLT_SOURCE_HEIGHT_OFFSET)?,
      )),
    ),
    EMR_SET_DIBITS_TO_DEVICE => (
      read_i32(data, record_offset + EMR_BITMAP_SOURCE_WIDTH_OFFSET)?,
      read_i32(data, record_offset + EMR_BITMAP_SOURCE_HEIGHT_OFFSET)?,
      None,
      None,
    ),
    EMR_STRETCH_DIBITS => (
      read_i32(data, record_offset + EMR_STRETCH_DIBITS_DEST_WIDTH_OFFSET)?,
      read_i32(data, record_offset + EMR_STRETCH_DIBITS_DEST_HEIGHT_OFFSET)?,
      Some(emf_ternary_raster_operation(read_u32(
        data,
        record_offset + EMR_STRETCH_DIBITS_ROP_OFFSET,
      )?)),
      Some((
        read_i32(data, record_offset + EMR_BITMAP_DEST_X_OFFSET + 8)?,
        read_i32(data, record_offset + EMR_BITMAP_DEST_Y_OFFSET + 8)?,
        read_i32(data, record_offset + EMR_BITMAP_SOURCE_WIDTH_OFFSET)?,
        read_i32(data, record_offset + EMR_BITMAP_SOURCE_HEIGHT_OFFSET)?,
      )),
    ),
    _ => unreachable!(),
  };
  if dest_width == 0 || dest_height == 0 {
    return Ok(None);
  }

  Ok(Some(EmfBitmapDrawTarget {
    dest_x,
    dest_y,
    dest_width,
    dest_height,
    raster_operation,
    source_rect,
  }))
}

fn emf_ternary_raster_operation(raw: u32) -> WmfTernaryRasterOperationCode {
  WmfTernaryRasterOperationCode::from_raw(((raw >> 16) & 0xff) as u8)
}

fn decode_bitmap_record_as_raster(
  data: &[u8],
  record_type: u32,
  record_offset: usize,
  record_size: usize,
) -> Result<DecodedMetafile, String> {
  let bitmap = emf_bitmap_record(data, record_type, record_offset, record_size)?
    .ok_or_else(|| "EMF bitmap record omits its source bitmap".to_string())?;
  let dib =
    DeviceIndependentBitmap::from_parts(bitmap.info, bitmap.bits).map_err(|err| err.to_string())?;
  if let Some(format) = dib.embedded_format() {
    return Ok(DecodedMetafile {
      data: dib.bits,
      content_type: format.content_type(),
    });
  }
  let pixels = device_independent_bitmap_to_rgb(&dib, bitmap.color_usage, None)?
    .ok_or_else(|| "unsupported EMF source bitmap format".to_string())?;
  Ok(DecodedMetafile {
    data: rgb_to_png(&pixels.rgb, pixels.width as u32, pixels.height as u32)?,
    content_type: "image/png",
  })
}

#[derive(Clone, Copy)]
struct EmfBitmapRecord<'a> {
  info: &'a [u8],
  bits: &'a [u8],
  color_usage: DibColorUsage,
}

fn emf_bitmap_record<'a>(
  data: &'a [u8],
  record_type: u32,
  record_offset: usize,
  record_size: usize,
) -> Result<Option<EmfBitmapRecord<'a>>, String> {
  let (info_offset_field, info_size_field, bits_offset_field, bits_size_field, usage_field) =
    match record_type {
      EMR_BIT_BLT | EMR_STRETCH_BLT => (
        EMR_BLT_INFO_OFFSET_OFFSET,
        EMR_BLT_INFO_SIZE_OFFSET,
        EMR_BLT_BITS_OFFSET_OFFSET,
        EMR_BLT_BITS_SIZE_OFFSET,
        EMR_BLT_COLOR_USAGE_OFFSET,
      ),
      EMR_SET_DIBITS_TO_DEVICE | EMR_STRETCH_DIBITS => (
        EMR_BITMAP_INFO_OFFSET_OFFSET,
        EMR_BITMAP_INFO_SIZE_OFFSET,
        EMR_BITMAP_BITS_OFFSET_OFFSET,
        EMR_BITMAP_BITS_SIZE_OFFSET,
        EMR_BITMAP_COLOR_USAGE_OFFSET,
      ),
      _ => {
        return Err(format!(
          "unsupported EMF bitmap record type 0x{record_type:08x}"
        ));
      }
    };
  let record_end = record_offset
    .checked_add(record_size)
    .ok_or_else(|| "EMF bitmap record range overflows".to_string())?;
  if record_end > data.len() {
    return Err("EMF bitmap record points outside the file".into());
  }
  let off_bmi = read_u32(data, record_offset + info_offset_field)? as usize;
  let cb_bmi = read_u32(data, record_offset + info_size_field)? as usize;
  let off_bits = read_u32(data, record_offset + bits_offset_field)? as usize;
  let cb_bits = read_u32(data, record_offset + bits_size_field)? as usize;
  if cb_bmi == 0 && cb_bits == 0 {
    return Ok(None);
  }
  let record_slice = |offset: usize, size: usize, description: &str| {
    let end = offset
      .checked_add(size)
      .ok_or_else(|| format!("{description} range overflows"))?;
    if end > record_size {
      return Err(format!("{description} points outside its EMF record"));
    }
    Ok(&data[record_offset + offset..record_offset + end])
  };
  let color_usage_raw = read_u32(data, record_offset + usage_field)?;
  let color_usage = DibColorUsage::from_raw(color_usage_raw)
    .ok_or_else(|| format!("unsupported EMF DIB color usage: {color_usage_raw}"))?;
  Ok(Some(EmfBitmapRecord {
    info: record_slice(off_bmi, cb_bmi, "bitmap info")?,
    bits: record_slice(off_bits, cb_bits, "bitmap bits")?,
    color_usage,
  }))
}

fn decode_bitmap_record_as_rgb(
  data: &[u8],
  record_type: u32,
  record_offset: usize,
  record_size: usize,
) -> Result<Option<RasterPixels>, String> {
  let Some(bitmap) = emf_bitmap_record(data, record_type, record_offset, record_size)? else {
    return Ok(None);
  };
  let dib =
    DeviceIndependentBitmap::from_parts(bitmap.info, bitmap.bits).map_err(|err| err.to_string())?;
  device_independent_bitmap_to_rgb(&dib, bitmap.color_usage, None)
}

fn crop_raster_pixels(
  image: &RasterPixels,
  (x, y, width, height): (i32, i32, i32, i32),
) -> Option<RasterPixels> {
  let (x, y, width, height) = (
    usize::try_from(x).ok()?,
    usize::try_from(y).ok()?,
    usize::try_from(width).ok()?,
    usize::try_from(height).ok()?,
  );
  if width == 0 || height == 0 {
    return None;
  }
  let right = x.checked_add(width)?;
  let bottom = y.checked_add(height)?;
  if right > image.width || bottom > image.height {
    return None;
  }
  if x == 0 && y == 0 && width == image.width && height == image.height {
    return None;
  }
  let mut rgb = Vec::with_capacity(width * height * RGB_BYTES_PER_PIXEL);
  for row in y..bottom {
    let start = (row * image.width + x) * RGB_BYTES_PER_PIXEL;
    let end = start + width * RGB_BYTES_PER_PIXEL;
    rgb.extend_from_slice(&image.rgb[start..end]);
  }
  Some(RasterPixels { width, height, rgb })
}

fn process_emf_plus_comment(
  data: &[u8],
  record_offset: usize,
  record_size: usize,
  state: &mut EmfVectorState,
) -> Result<(), String> {
  // EMR_COMMENT_EMFPLUS chunks as a stream of 12-byte EMF+ record headers.
  let data_size = read_u32(data, record_offset + 8)? as usize;
  let comment_identifier = read_u32(data, record_offset + 12)?;
  if comment_identifier != EMR_COMMENT_EMFPLUS || data_size < 4 {
    return Ok(());
  }
  let mut cursor = record_offset + 16;
  let end = record_offset
    .checked_add(12)
    .and_then(|offset| offset.checked_add(data_size))
    .map(|end| end.min(record_offset + record_size))
    .ok_or_else(|| "EMF+ comment range overflows".to_string())?;
  while cursor + 12 <= end {
    let size = read_u32(data, cursor + 4)? as usize;
    if size < 12 || cursor + size > end {
      break;
    }
    let record_bytes = &data[cursor..cursor + size];
    let mut reader = Reader::new(std::io::Cursor::new(record_bytes));
    if let Ok(record) = EmfPlusRecord::read_from(&mut reader, record_bytes.len() as u64) {
      if record.record_kind() == Some(EmfPlusRecordType::Object) {
        if let Ok(fragment) = record.into_object_fragment() {
          process_emf_plus_object(fragment, state)?;
        }
      } else if let Ok(parsed) = record.parse_data() {
        process_emf_plus_record(parsed, state)?;
      }
    }
    cursor += size;
  }
  Ok(())
}

fn process_emf_plus_record(
  record: EmfPlusRecordData<'_>,
  state: &mut EmfVectorState,
) -> Result<(), String> {
  match record {
    EmfPlusRecordData::Object(value) => process_emf_plus_object(value, state)?,
    EmfPlusRecordData::Clear(value) => {
      let color = emf_plus_argb_to_color(value.color);
      for y in 0..state.height {
        for x in 0..state.width {
          state.set_pixel(x as i32, y as i32, color);
        }
      }
    }
    EmfPlusRecordData::FillRects(value) => {
      let Some(brush) = emf_plus_brush_ref(value.brush, state) else {
        return Ok(());
      };
      for rect in value.rects {
        let (left, top, right, bottom) = emf_plus_rect_bounds(rect);
        state.fill_polygon_with_emf_plus_brush(&rect_points(left, top, right, bottom), &brush);
      }
    }
    EmfPlusRecordData::DrawRects(value) => {
      let old = state.current_pen;
      if let Some(pen) = emf_plus_pen(value.pen_id, state) {
        state.current_pen = Some(pen);
        for rect in value.rects {
          let (left, top, right, bottom) = emf_plus_rect_bounds(rect);
          state.fill_rect(left, top, right, bottom);
        }
      }
      state.current_pen = old;
    }
    EmfPlusRecordData::FillPolygon(value) => {
      if let Some(brush) = emf_plus_brush_ref(value.brush, state) {
        let points = emf_plus_points_to_emf_points(&value.points);
        state.fill_polygon_with_emf_plus_brush(&points, &brush);
      }
    }
    EmfPlusRecordData::DrawLines(value) => {
      if let Some(pen) = emf_plus_pen(value.pen_id, state) {
        let old = state.current_pen;
        state.current_pen = Some(pen);
        let points = emf_plus_points_to_emf_points(&value.points);
        state.draw_polyline(&points, value.close_shape);
        state.current_pen = old;
      }
    }
    EmfPlusRecordData::FillEllipse(value) => fill_emf_plus_rect_shape(value, state),
    EmfPlusRecordData::DrawEllipse(value) => draw_emf_plus_rect_shape(value, state),
    EmfPlusRecordData::FillPie(value) => fill_emf_plus_pie(value, state),
    EmfPlusRecordData::DrawPie(value) | EmfPlusRecordData::DrawArc(value) => {
      draw_emf_plus_arc(value, state)
    }
    EmfPlusRecordData::FillRegion(value) => {
      if let Some(brush) = emf_plus_brush_ref(value.brush, state)
        && let Some(points) = emf_plus_region_points(value.object_id, state)
      {
        state.fill_polygon_with_emf_plus_brush(&points, &brush);
      }
    }
    EmfPlusRecordData::FillPath(value) => {
      if let Some(brush) = emf_plus_brush_ref(value.brush, state)
        && let Some(points) = emf_plus_path_points(value.object_id, state)
      {
        state.fill_polygon_with_emf_plus_brush(&points, &brush);
      }
    }
    EmfPlusRecordData::DrawPath(value) => {
      if let Some(pen) = emf_plus_pen(value.pen_id, state)
        && let Some(points) = emf_plus_path_points(value.object_id, state)
      {
        let old = state.current_pen;
        state.current_pen = Some(pen);
        state.draw_polyline(&points, true);
        state.current_pen = old;
      }
    }
    EmfPlusRecordData::FillClosedCurve(value) => {
      if let Some(brush) = emf_plus_brush_ref(value.brush, state) {
        let points = emf_plus_points_to_emf_points(&value.points);
        let points = flatten_cardinal_curve(&points, value.tension, true);
        state.fill_polygon_with_emf_plus_brush(&points, &brush);
        state.draw_polyline(&points, true);
      }
    }
    EmfPlusRecordData::DrawClosedCurve(value) => {
      if let Some(pen) = emf_plus_pen(value.pen_id, state) {
        let old = state.current_pen;
        state.current_pen = Some(pen);
        let points = emf_plus_points_to_emf_points(&value.points);
        let points = flatten_cardinal_curve(&points, value.tension, true);
        state.draw_polyline(&points, true);
        state.current_pen = old;
      }
    }
    EmfPlusRecordData::DrawCurve(value) => {
      if let Some(pen) = emf_plus_pen(value.pen_id, state) {
        let old = state.current_pen;
        state.current_pen = Some(pen);
        let points = emf_plus_points_to_emf_points(&value.points);
        let start = value.offset as usize;
        let end = start
          .saturating_add(value.num_segments as usize + 1)
          .min(points.len());
        let points = if start < end {
          flatten_cardinal_curve(&points[start..end], value.tension, false)
        } else {
          points
        };
        state.draw_polyline(&points, false);
        state.current_pen = old;
      }
    }
    EmfPlusRecordData::StrokeFillPath => {
      let paths = state
        .emf_plus_objects
        .iter()
        .filter_map(|object| match object {
          Some(EmfPlusRenderObject::Path(points)) => Some(points.clone()),
          _ => None,
        })
        .collect::<Vec<_>>();
      for points in paths {
        state.fill_polygon(&points);
        state.draw_polyline(&points, true);
      }
    }
    EmfPlusRecordData::DrawBeziers(value) => draw_emf_plus_beziers(value, state),
    EmfPlusRecordData::DrawImage(value) => draw_emf_plus_image(value, state),
    EmfPlusRecordData::DrawImagePoints(value) => draw_emf_plus_image_points(value, state),
    EmfPlusRecordData::DrawString(value) => draw_emf_plus_string(value, state),
    EmfPlusRecordData::DrawDriverString(value) => {
      if let Some(color) = emf_plus_brush_ref_to_color(value.brush, state)
        && let Some(first) = value.glyph_positions.first()
      {
        let text = value
          .glyphs
          .iter()
          .filter_map(|glyph| char::from_u32(u32::from(*glyph)))
          .collect::<String>();
        state.draw_text(first.x as i32, first.y as i32, &text, color, 12);
      }
    }
    EmfPlusRecordData::Save(_)
    | EmfPlusRecordData::BeginContainer(_)
    | EmfPlusRecordData::BeginContainerNoParams(_) => state.save_state(),
    EmfPlusRecordData::Restore(_) | EmfPlusRecordData::EndContainer(_) => state.restore_state(),
    EmfPlusRecordData::SetClipRect(value) => {
      let (left, top, right, bottom) = emf_plus_rectf_bounds(value.clip_rect);
      let points = [
        EmfPoint { x: left, y: top },
        EmfPoint { x: right, y: top },
        EmfPoint {
          x: right,
          y: bottom,
        },
        EmfPoint { x: left, y: bottom },
      ];
      state.set_clip_points_logical(&points, value.combine_mode);
    }
    EmfPlusRecordData::SetClipPath(value) => {
      if let Some(points) = emf_plus_path_points(value.object_id, state) {
        state.set_clip_points_logical(&points, value.combine_mode);
      }
    }
    EmfPlusRecordData::SetClipRegion(value) => {
      if let Some(points) = emf_plus_region_points(value.object_id, state) {
        state.set_clip_points_logical(&points, value.combine_mode);
      }
    }
    EmfPlusRecordData::OffsetClip(value) => {
      state.offset_clip(value.dx, value.dy);
    }
    EmfPlusRecordData::ResetClip => {
      state.clip_rect = None;
      state.clip_mask = None;
    }
    EmfPlusRecordData::SetWorldTransform(value) => {
      state.world_transform = xform_to_transform(value);
    }
    EmfPlusRecordData::ResetWorldTransform => state.world_transform = EmfTransform::identity(),
    EmfPlusRecordData::MultiplyWorldTransform(value) => {
      multiply_emf_plus_transform(xform_to_transform(value.data), value.post_multiply, state);
    }
    EmfPlusRecordData::TranslateWorldTransform(value) => {
      multiply_emf_plus_transform(translate_transform(value.data), value.post_multiply, state);
    }
    EmfPlusRecordData::ScaleWorldTransform(value) => {
      multiply_emf_plus_transform(scale_transform(value.data), value.post_multiply, state);
    }
    EmfPlusRecordData::RotateWorldTransform(value) => {
      multiply_emf_plus_transform(rotate_transform(value.data), value.post_multiply, state);
    }
    EmfPlusRecordData::SetPageTransform(value)
      if value.page_scale.is_finite() && value.page_scale > 0.0 =>
    {
      multiply_emf_plus_transform(
        EmfTransform {
          m11: value.page_scale,
          m22: value.page_scale,
          ..EmfTransform::identity()
        },
        true,
        state,
      );
    }
    EmfPlusRecordData::SetTsGraphics(value) => {
      state.world_transform = xform_to_transform(value.world_to_device);
    }
    _ => {}
  }
  Ok(())
}

fn process_emf_plus_object(
  value: EmfPlusObjectRecordData,
  state: &mut EmfVectorState,
) -> Result<(), String> {
  match state.emf_plus_object_assembler.push(value) {
    Ok(Some(complete)) => process_complete_emf_plus_object(complete, state),
    Ok(None) => {}
    Err(_) => {
      state.emf_plus_object_assembler = EmfPlusObjectAssembler::default();
    }
  }
  Ok(())
}

fn process_complete_emf_plus_object(value: EmfPlusObjectRecordData, state: &mut EmfVectorState) {
  let object = match value.parse_object_data() {
    Ok(EmfPlusObjectData::Brush(brush)) => {
      EmfPlusRenderObject::Brush(emf_plus_brush_object(&brush))
    }
    Ok(EmfPlusObjectData::Pen(pen)) => EmfPlusRenderObject::Pen(emf_plus_pen_object(&pen)),
    Ok(EmfPlusObjectData::Path(path)) => {
      EmfPlusRenderObject::Path(emf_plus_path_object_points(&path))
    }
    Ok(EmfPlusObjectData::Region(region)) => {
      EmfPlusRenderObject::Region(emf_plus_region_object_points(&region))
    }
    Ok(EmfPlusObjectData::Image(image)) => match emf_plus_image_object_to_rgb(&image) {
      Ok(Some(image)) => EmfPlusRenderObject::Image(image),
      _ => EmfPlusRenderObject::Unsupported,
    },
    Ok(EmfPlusObjectData::Font(font)) => EmfPlusRenderObject::Font(font),
    _ => EmfPlusRenderObject::Unsupported,
  };
  let index = value.object_id as usize;
  if state.emf_plus_objects.len() <= index {
    state.emf_plus_objects.resize(index + 1, None);
  }
  state.emf_plus_objects[index] = Some(object);
}

fn emf_plus_brush_object(brush: &crate::emfplus::EmfPlusBrushObject) -> Option<EmfPlusRenderBrush> {
  match brush.parse_brush_data().ok()? {
    EmfPlusBrushData::Solid(value) => Some(EmfPlusRenderBrush::Solid(emf_plus_argb_to_color(
      value.solid_color,
    ))),
    EmfPlusBrushData::Hatch(value) => Some(EmfPlusRenderBrush::Hatch {
      fore: emf_plus_argb_to_color(value.fore_color),
      back: emf_plus_argb_to_color(value.back_color),
      style: value.hatch_style,
    }),
    EmfPlusBrushData::LinearGradient(value) => Some(EmfPlusRenderBrush::LinearGradient {
      rect: (
        value.rect.x,
        value.rect.y,
        value.rect.x + value.rect.width,
        value.rect.y + value.rect.height,
      ),
      start: emf_plus_argb_to_color(value.start_color),
      end: emf_plus_argb_to_color(value.end_color),
    }),
    EmfPlusBrushData::PathGradient(value) => Some(EmfPlusRenderBrush::PathGradient {
      center: (value.center_point.x, value.center_point.y),
      center_color: emf_plus_argb_to_color(value.center_color),
      surround: value
        .surrounding_colors
        .first()
        .copied()
        .map(emf_plus_argb_to_color)
        .unwrap_or_else(|| emf_plus_argb_to_color(value.center_color)),
    }),
    EmfPlusBrushData::Texture(value) => value
      .parse_optional_data()
      .ok()?
      .image_object
      .as_ref()
      .and_then(|image| emf_plus_image_object_to_rgb(image).ok().flatten())
      .map(EmfPlusRenderBrush::Texture),
    EmfPlusBrushData::Unknown { .. } => None,
  }
}

fn emf_plus_pen_object(pen: &EmfPlusPenObject) -> Option<EmfPen> {
  let payload = pen.parse_pen_payload().ok()?;
  let brush = payload.brush_object.as_ref()?;
  Some(EmfPen {
    color: emf_plus_brush_object(brush)?.representative_color(),
    width: payload.pen_data.pen_width.round().max(1.0) as usize,
    transform_width: payload.pen_data.pen_unit_kind() == Some(EmfPlusUnitType::World),
  })
}

fn emf_plus_brush_ref(
  brush: EmfPlusBrushRef,
  state: &EmfVectorState,
) -> Option<EmfPlusRenderBrush> {
  match brush {
    EmfPlusBrushRef::Color(color) => Some(EmfPlusRenderBrush::Solid(emf_plus_argb_to_color(color))),
    EmfPlusBrushRef::ObjectId(id) => match state.emf_plus_objects.get(id as usize)? {
      Some(EmfPlusRenderObject::Brush(brush)) => brush.clone(),
      Some(EmfPlusRenderObject::Pen(Some(pen))) => Some(EmfPlusRenderBrush::Solid(pen.color)),
      _ => None,
    },
  }
}

fn emf_plus_brush_ref_to_color(brush: EmfPlusBrushRef, state: &EmfVectorState) -> Option<EmfColor> {
  emf_plus_brush_ref(brush, state).map(|brush| brush.representative_color())
}

fn emf_plus_pen(id: u8, state: &EmfVectorState) -> Option<EmfPen> {
  match state.emf_plus_objects.get(id as usize)? {
    Some(EmfPlusRenderObject::Pen(pen)) => pen.map(|pen| state.resolve_pen(pen)),
    Some(EmfPlusRenderObject::Brush(Some(brush))) => Some(EmfPen {
      color: brush.representative_color(),
      width: 1,
      transform_width: false,
    }),
    _ => None,
  }
}

fn emf_plus_argb_to_color(color: crate::EmfPlusArgb) -> EmfColor {
  EmfColor {
    r: color.red,
    g: color.green,
    b: color.blue,
  }
}

fn lerp_color(start: EmfColor, end: EmfColor, t: f32) -> EmfColor {
  let t = t.clamp(0.0, 1.0);
  EmfColor {
    r: (start.r as f32 + (end.r as f32 - start.r as f32) * t).round() as u8,
    g: (start.g as f32 + (end.g as f32 - start.g as f32) * t).round() as u8,
    b: (start.b as f32 + (end.b as f32 - start.b as f32) * t).round() as u8,
  }
}

fn average_color(a: EmfColor, b: EmfColor) -> EmfColor {
  EmfColor {
    r: ((u16::from(a.r) + u16::from(b.r)) / 2) as u8,
    g: ((u16::from(a.g) + u16::from(b.g)) / 2) as u8,
    b: ((u16::from(a.b) + u16::from(b.b)) / 2) as u8,
  }
}

fn average_image_color(image: &RasterPixels) -> EmfColor {
  if image.rgb.is_empty() {
    return EmfColor { r: 0, g: 0, b: 0 };
  }
  let mut r = 0u64;
  let mut g = 0u64;
  let mut b = 0u64;
  let mut count = 0u64;
  for pixel in image.rgb.chunks_exact(RGB_BYTES_PER_PIXEL) {
    r += u64::from(pixel[0]);
    g += u64::from(pixel[1]);
    b += u64::from(pixel[2]);
    count += 1;
  }
  EmfColor {
    r: (r / count) as u8,
    g: (g / count) as u8,
    b: (b / count) as u8,
  }
}

fn emf_plus_rect_bounds(rect: crate::EmfPlusRect) -> (i32, i32, i32, i32) {
  match rect {
    crate::EmfPlusRect::Compressed(rect) => (
      i32::from(rect.x),
      i32::from(rect.y),
      i32::from(rect.x) + i32::from(rect.width),
      i32::from(rect.y) + i32::from(rect.height),
    ),
    crate::EmfPlusRect::Float(rect) => emf_plus_rectf_bounds(rect),
  }
}

fn emf_plus_rectf_bounds(rect: crate::RectF) -> (i32, i32, i32, i32) {
  (
    rect.x.round() as i32,
    rect.y.round() as i32,
    (rect.x + rect.width).round() as i32,
    (rect.y + rect.height).round() as i32,
  )
}

fn fill_emf_plus_rect_shape(value: EmfPlusFillRectShapeData, state: &mut EmfVectorState) {
  if let Some(brush) = emf_plus_brush_ref(value.brush, state) {
    let (left, top, right, bottom) = emf_plus_rect_bounds(value.rect);
    state.fill_ellipse_with_emf_plus_brush(left, top, right, bottom, &brush);
  }
}

fn draw_emf_plus_rect_shape(value: EmfPlusDrawRectShapeData, state: &mut EmfVectorState) {
  if let Some(pen) = emf_plus_pen(value.pen_id, state) {
    let old_brush = state.current_brush;
    let old_pen = state.current_pen;
    state.current_brush = None;
    state.current_pen = Some(pen);
    let (left, top, right, bottom) = emf_plus_rect_bounds(value.rect);
    state.fill_ellipse(left, top, right, bottom);
    state.current_brush = old_brush;
    state.current_pen = old_pen;
  }
}

fn fill_emf_plus_pie(value: EmfPlusFillPieData, state: &mut EmfVectorState) {
  if let Some(brush) = emf_plus_brush_ref(value.brush, state) {
    let (left, top, right, bottom) = emf_plus_rect_bounds(value.rect);
    let points = arc_segment_points(
      left,
      top,
      right,
      bottom,
      value.start_angle,
      value.sweep_angle,
      true,
    );
    state.fill_polygon_with_emf_plus_brush(&points, &brush);
    state.draw_polyline(&points, true);
  }
}

fn draw_emf_plus_arc(value: EmfPlusDrawArcData, state: &mut EmfVectorState) {
  if let Some(pen) = emf_plus_pen(value.pen_id, state) {
    let old = state.current_pen;
    state.current_pen = Some(pen);
    let (left, top, right, bottom) = emf_plus_rect_bounds(value.rect);
    state.fill_arc_segment(
      (left, top, right, bottom),
      value.start_angle,
      value.sweep_angle,
      false,
    );
    state.current_pen = old;
  }
}

fn arc_segment_points(
  left: i32,
  top: i32,
  right: i32,
  bottom: i32,
  start_angle: f32,
  sweep_angle: f32,
  pie: bool,
) -> Vec<EmfPoint> {
  let steps = ((sweep_angle.abs() / 5.0).ceil() as usize).clamp(6, 144);
  let cx = (left + right) as f32 / 2.0;
  let cy = (top + bottom) as f32 / 2.0;
  let rx = (right - left).abs() as f32 / 2.0;
  let ry = (bottom - top).abs() as f32 / 2.0;
  let mut points = Vec::with_capacity(steps + usize::from(pie) + 1);
  if pie {
    points.push(EmfPoint {
      x: cx.round() as i32,
      y: cy.round() as i32,
    });
  }
  for index in 0..=steps {
    let angle = (start_angle + sweep_angle * index as f32 / steps as f32).to_radians();
    points.push(EmfPoint {
      x: (cx + angle.cos() * rx).round() as i32,
      y: (cy + angle.sin() * ry).round() as i32,
    });
  }
  points
}

fn draw_emf_plus_beziers(value: EmfPlusDrawPointsData, state: &mut EmfVectorState) {
  if let Some(pen) = emf_plus_pen(value.pen_id, state) {
    let old = state.current_pen;
    state.current_pen = Some(pen);
    let points = emf_plus_points_to_emf_points(&value.points);
    let flattened = flatten_bezier_sequence(&points);
    state.draw_polyline(&flattened, false);
    state.current_pen = old;
  }
}

fn emf_plus_points_to_emf_points(points: &EmfPlusPointData) -> Vec<EmfPoint> {
  match points {
    EmfPlusPointData::Relative(points) => {
      let mut current = EmfPoint { x: 0, y: 0 };
      points
        .iter()
        .map(|point| {
          current.x += i32::from(point.x);
          current.y += i32::from(point.y);
          current
        })
        .collect()
    }
    EmfPlusPointData::Compressed(points) => points
      .iter()
      .map(|point| EmfPoint {
        x: i32::from(point.x),
        y: i32::from(point.y),
      })
      .collect(),
    EmfPlusPointData::Float(points) => points
      .iter()
      .map(|point| EmfPoint {
        x: point.x.round() as i32,
        y: point.y.round() as i32,
      })
      .collect(),
  }
}

fn emf_plus_path_object_points(path: &EmfPlusPathObject) -> Vec<EmfPoint> {
  let points = emf_plus_points_to_emf_points(&path.points);
  let types = expanded_path_point_types(&path.point_types);
  if types.is_empty() {
    return points;
  }
  let mut result = Vec::with_capacity(points.len());
  let mut index = 0usize;
  while index < points.len() && index < types.len() {
    let point = points[index];
    let point_type = types[index];
    if point_type.path_point_type() == Some(EmfPlusPathPointType::Bezier)
      && index + 2 < points.len()
      && let Some(start) = result.last().copied()
    {
      result.extend(sample_cubic_bezier(
        start,
        points[index],
        points[index + 1],
        points[index + 2],
      ));
      index += 3;
      continue;
    }
    result.push(point);
    if point_type
      .path_point_flags()
      .contains(EmfPlusPathPointTypeFlags::CLOSE_SUBPATH)
      && let Some(first) = result.first().copied()
    {
      result.push(first);
    }
    index += 1;
  }
  result
}

fn flatten_bezier_sequence(points: &[EmfPoint]) -> Vec<EmfPoint> {
  let Some(first) = points.first().copied() else {
    return Vec::new();
  };
  let mut result = vec![first];
  let mut index = 1usize;
  while index + 2 < points.len() {
    let start = *result.last().unwrap_or(&first);
    result.extend(sample_cubic_bezier(
      start,
      points[index],
      points[index + 1],
      points[index + 2],
    ));
    index += 3;
  }
  result.extend_from_slice(&points[index..]);
  result
}

fn sample_cubic_bezier(p0: EmfPoint, p1: EmfPoint, p2: EmfPoint, p3: EmfPoint) -> Vec<EmfPoint> {
  let chord = ((p3.x - p0.x).unsigned_abs() + (p3.y - p0.y).unsigned_abs()) as usize;
  let control = ((p1.x - p0.x).unsigned_abs()
    + (p1.y - p0.y).unsigned_abs()
    + (p2.x - p3.x).unsigned_abs()
    + (p2.y - p3.y).unsigned_abs()) as usize;
  let steps = ((chord + control) / 16).clamp(8, 64);
  (1..=steps)
    .map(|step| {
      let t = step as f32 / steps as f32;
      let mt = 1.0 - t;
      let x = mt.powi(3) * p0.x as f32
        + 3.0 * mt.powi(2) * t * p1.x as f32
        + 3.0 * mt * t.powi(2) * p2.x as f32
        + t.powi(3) * p3.x as f32;
      let y = mt.powi(3) * p0.y as f32
        + 3.0 * mt.powi(2) * t * p1.y as f32
        + 3.0 * mt * t.powi(2) * p2.y as f32
        + t.powi(3) * p3.y as f32;
      EmfPoint {
        x: x.round() as i32,
        y: y.round() as i32,
      }
    })
    .collect()
}

fn flatten_cardinal_curve(points: &[EmfPoint], tension: f32, closed: bool) -> Vec<EmfPoint> {
  if points.len() < 2 {
    return points.to_vec();
  }
  let mut result = Vec::new();
  if !closed {
    result.push(points[0]);
  }
  let segment_count = if closed {
    points.len()
  } else {
    points.len() - 1
  };
  let tension = tension.clamp(0.0, 1.0);
  let tangent_scale = (1.0 - tension) / 2.0;
  for index in 0..segment_count {
    let p0 = if index == 0 {
      if closed {
        points[points.len() - 1]
      } else {
        points[0]
      }
    } else {
      points[index - 1]
    };
    let p1 = points[index];
    let p2 = points[(index + 1) % points.len()];
    let p3 = if index + 2 < points.len() {
      points[index + 2]
    } else if closed {
      points[(index + 2) % points.len()]
    } else {
      points[points.len() - 1]
    };
    let distance = ((p2.x - p1.x).unsigned_abs() + (p2.y - p1.y).unsigned_abs()) as usize;
    let steps = (distance / 12).clamp(6, 32);
    for step in 1..=steps {
      let t = step as f32 / steps as f32;
      let t2 = t * t;
      let t3 = t2 * t;
      let m1x = (p2.x - p0.x) as f32 * tangent_scale;
      let m1y = (p2.y - p0.y) as f32 * tangent_scale;
      let m2x = (p3.x - p1.x) as f32 * tangent_scale;
      let m2y = (p3.y - p1.y) as f32 * tangent_scale;
      let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
      let h10 = t3 - 2.0 * t2 + t;
      let h01 = -2.0 * t3 + 3.0 * t2;
      let h11 = t3 - t2;
      result.push(EmfPoint {
        x: (h00 * p1.x as f32 + h10 * m1x + h01 * p2.x as f32 + h11 * m2x).round() as i32,
        y: (h00 * p1.y as f32 + h10 * m1y + h01 * p2.y as f32 + h11 * m2y).round() as i32,
      });
    }
  }
  result
}

fn expanded_path_point_types(types: &EmfPlusPathPointTypes) -> Vec<EmfPlusPathPointTypeValue> {
  match types {
    EmfPlusPathPointTypes::Values(values) => values.clone(),
    EmfPlusPathPointTypes::Rle(values) => {
      let mut expanded = Vec::new();
      for value in values {
        expanded.extend(std::iter::repeat_n(
          value.point_type,
          value.run_count() as usize,
        ));
      }
      expanded
    }
  }
}

fn emf_plus_path_points(id: u8, state: &EmfVectorState) -> Option<Vec<EmfPoint>> {
  match state.emf_plus_objects.get(id as usize)? {
    Some(EmfPlusRenderObject::Path(points)) => Some(points.clone()),
    _ => None,
  }
}

fn emf_plus_region_points(id: u8, state: &EmfVectorState) -> Option<Vec<EmfPoint>> {
  match state.emf_plus_objects.get(id as usize)? {
    Some(EmfPlusRenderObject::Region(points)) => Some(points.clone()),
    _ => None,
  }
}

fn emf_plus_region_object_points(region: &crate::emfplus::EmfPlusRegionObject) -> Vec<EmfPoint> {
  let Ok(nodes) = region.parse_region_nodes() else {
    return Vec::new();
  };
  nodes
    .first()
    .and_then(emf_plus_region_node_points)
    .unwrap_or_default()
}

fn emf_plus_region_node_points(node: &crate::emfplus::EmfPlusRegionNode) -> Option<Vec<EmfPoint>> {
  match &node.data {
    crate::emfplus::EmfPlusRegionNodeData::Rect(rect) => {
      let (left, top, right, bottom) = emf_plus_rectf_bounds(*rect);
      Some(rect_points(left, top, right, bottom))
    }
    crate::emfplus::EmfPlusRegionNodeData::Path(path) => {
      path.path().map(emf_plus_path_object_points)
    }
    crate::emfplus::EmfPlusRegionNodeData::Infinite => Some(rect_points(
      0,
      0,
      DEFAULT_RENDER_WIDTH as i32,
      DEFAULT_RENDER_HEIGHT as i32,
    )),
    crate::emfplus::EmfPlusRegionNodeData::ChildNodes(children) => {
      let left = emf_plus_region_node_points(&children.left);
      let right = emf_plus_region_node_points(&children.right);
      union_point_bounds(left.as_deref(), right.as_deref())
    }
    crate::emfplus::EmfPlusRegionNodeData::Empty
    | crate::emfplus::EmfPlusRegionNodeData::Raw(_) => None,
  }
}

fn union_point_bounds(
  left: Option<&[EmfPoint]>,
  right: Option<&[EmfPoint]>,
) -> Option<Vec<EmfPoint>> {
  let mut points = Vec::new();
  if let Some(left) = left {
    points.extend_from_slice(left);
  }
  if let Some(right) = right {
    points.extend_from_slice(right);
  }
  point_bounds(&points).map(|(left, top, right, bottom)| rect_points(left, top, right, bottom))
}

fn point_bounds(points: &[EmfPoint]) -> Option<(i32, i32, i32, i32)> {
  let first = points.first().copied()?;
  let mut left = first.x;
  let mut top = first.y;
  let mut right = first.x;
  let mut bottom = first.y;
  for point in points.iter().copied().skip(1) {
    left = left.min(point.x);
    top = top.min(point.y);
    right = right.max(point.x);
    bottom = bottom.max(point.y);
  }
  Some((left, top, right, bottom))
}

fn rect_points(left: i32, top: i32, right: i32, bottom: i32) -> Vec<EmfPoint> {
  vec![
    EmfPoint { x: left, y: top },
    EmfPoint { x: right, y: top },
    EmfPoint {
      x: right,
      y: bottom,
    },
    EmfPoint { x: left, y: bottom },
  ]
}

fn draw_emf_plus_image(value: EmfPlusDrawImageData, state: &mut EmfVectorState) {
  let Some(image) = state
    .emf_plus_objects
    .get(value.image_id as usize)
    .and_then(|value| match value {
      Some(EmfPlusRenderObject::Image(image)) => Some(image.clone()),
      _ => None,
    })
  else {
    return;
  };
  let (left, top, right, bottom) = emf_plus_rect_bounds(value.dest_rect);
  state.draw_rgb_image(left, top, right - left, bottom - top, &image);
}

fn draw_emf_plus_image_points(value: EmfPlusDrawImagePointsData, state: &mut EmfVectorState) {
  let Some(image) = state
    .emf_plus_objects
    .get(value.image_id as usize)
    .and_then(|value| match value {
      Some(EmfPlusRenderObject::Image(image)) => Some(image.clone()),
      _ => None,
    })
  else {
    return;
  };
  let points = emf_plus_points_to_emf_points(&value.points);
  let Some(first) = points.first().copied() else {
    return;
  };
  let width = points
    .get(1)
    .map(|point| point.x - first.x)
    .unwrap_or(image.width as i32);
  let height = points
    .get(2)
    .map(|point| point.y - first.y)
    .unwrap_or(image.height as i32);
  state.draw_rgb_image(first.x, first.y, width, height, &image);
}

fn draw_emf_plus_string(value: EmfPlusDrawStringData, state: &mut EmfVectorState) {
  let Some(color) = emf_plus_brush_ref_to_color(value.brush, state) else {
    return;
  };
  let text = value
    .string
    .as_str()
    .map(|text| text.to_string())
    .unwrap_or_default();
  let height = match state.emf_plus_objects.get(value.font_id as usize) {
    Some(Some(EmfPlusRenderObject::Font(font))) => font.em_size.round() as i32,
    _ => value.layout_rect.height.round() as i32,
  }
  .abs()
  .max(7);
  state.draw_text(
    value.layout_rect.x.round() as i32,
    value.layout_rect.y.round() as i32 + height,
    &text,
    color,
    height,
  );
}

fn emf_plus_image_object_to_rgb(
  image: &EmfPlusImageObject,
) -> Result<Option<RasterPixels>, String> {
  match image.parse_image_data().map_err(|err| err.to_string())? {
    EmfPlusImageData::Bitmap(bitmap) => emf_plus_bitmap_to_rgb(&bitmap),
    EmfPlusImageData::Metafile(metafile) => {
      let Some(raster) =
        decode_metafile_as_raster(&metafile.metafile_data, None).map_err(|err| err.to_string())?
      else {
        return Ok(None);
      };
      decoded_raster_to_rgb(&raster)
    }
    EmfPlusImageData::Unknown { .. } => Ok(None),
  }
}

fn emf_plus_bitmap_to_rgb(
  bitmap: &crate::emfplus::EmfPlusBitmapObject,
) -> Result<Option<RasterPixels>, String> {
  match bitmap.parse_bitmap_data().map_err(|err| err.to_string())? {
    EmfPlusBitmapPayload::Compressed(data) => {
      let raster = image::load_from_memory(&data.compressed_image_data)
        .map_err(|err| err.to_string())?
        .to_rgb8();
      let (width, height) = raster.dimensions();
      Ok(Some(RasterPixels {
        width: width as usize,
        height: height as usize,
        rgb: raster.into_raw(),
      }))
    }
    EmfPlusBitmapPayload::Pixel(data) => emf_plus_pixel_bitmap_to_rgb(bitmap, &data.pixel_data),
    EmfPlusBitmapPayload::Unknown { .. } => Ok(None),
  }
}

fn emf_plus_pixel_bitmap_to_rgb(
  bitmap: &crate::emfplus::EmfPlusBitmapObject,
  pixels: &[u8],
) -> Result<Option<RasterPixels>, String> {
  if bitmap.width <= 0 || bitmap.height <= 0 {
    return Ok(None);
  }
  let width = bitmap.width as usize;
  let height = bitmap.height as usize;
  let stride = bitmap.stride.unsigned_abs() as usize;
  let bpp = bitmap.bits_per_pixel();
  let bytes_per_pixel = match bpp {
    24 => 3,
    32 => 4,
    _ => return Ok(None),
  };
  let required = stride
    .checked_mul(height)
    .ok_or_else(|| "EMF+ bitmap dimensions overflow".to_string())?;
  if pixels.len() < required || stride < width * bytes_per_pixel {
    return Err("EMF+ bitmap payload is truncated".to_string());
  }
  let mut rgb = vec![0u8; width * height * RGB_BYTES_PER_PIXEL];
  for row in 0..height {
    let src_row = if bitmap.stride < 0 {
      height - 1 - row
    } else {
      row
    };
    let src = &pixels[src_row * stride..src_row * stride + stride];
    let dest = &mut rgb[row * width * RGB_BYTES_PER_PIXEL..(row + 1) * width * RGB_BYTES_PER_PIXEL];
    for col in 0..width {
      let src_pixel = &src[col * bytes_per_pixel..col * bytes_per_pixel + bytes_per_pixel];
      let dest_pixel =
        &mut dest[col * RGB_BYTES_PER_PIXEL..col * RGB_BYTES_PER_PIXEL + RGB_BYTES_PER_PIXEL];
      dest_pixel[0] = src_pixel[2];
      dest_pixel[1] = src_pixel[1];
      dest_pixel[2] = src_pixel[0];
    }
  }
  Ok(Some(RasterPixels { width, height, rgb }))
}

fn xform_to_transform(value: crate::XForm) -> EmfTransform {
  EmfTransform {
    m11: value.m11,
    m12: value.m12,
    m21: value.m21,
    m22: value.m22,
    dx: value.dx,
    dy: value.dy,
  }
}

fn translate_transform(value: EmfPlusTranslateWorldTransformData) -> EmfTransform {
  EmfTransform {
    dx: value.dx,
    dy: value.dy,
    ..EmfTransform::identity()
  }
}

fn scale_transform(value: EmfPlusScaleWorldTransformData) -> EmfTransform {
  EmfTransform {
    m11: value.sx,
    m22: value.sy,
    ..EmfTransform::identity()
  }
}

fn rotate_transform(value: EmfPlusRotateWorldTransformData) -> EmfTransform {
  let radians = value.angle.to_radians();
  EmfTransform {
    m11: radians.cos(),
    m12: radians.sin(),
    m21: -radians.sin(),
    m22: radians.cos(),
    dx: 0.0,
    dy: 0.0,
  }
}

fn multiply_emf_plus_transform(
  transform: EmfTransform,
  post_multiply: bool,
  state: &mut EmfVectorState,
) {
  state.world_transform = if post_multiply {
    state.world_transform.multiply(transform)
  } else {
    transform.multiply(state.world_transform)
  };
}

fn color_ref_to_emf(value: crate::ColorRef) -> EmfColor {
  EmfColor {
    r: value.red,
    g: value.green,
    b: value.blue,
  }
}

fn single_byte_text(bytes: &[u8]) -> String {
  bytes
    .iter()
    .take_while(|byte| **byte != 0)
    .map(|byte| char::from(*byte))
    .collect()
}

fn emf_current_font(state: &EmfVectorState) -> WmfTextFont {
  state
    .current_font
    .and_then(|id| state.fonts.get(&id))
    .map(|font| WmfTextFont {
      height: font.height,
      family: font.family.clone(),
      weight: font.weight,
      italic: font.italic,
    })
    .unwrap_or(WmfTextFont {
      height: 12,
      family: None,
      weight: 400,
      italic: false,
    })
}

fn emf_arc_rect(data: &[u8], record_offset: usize) -> Result<(i32, i32, i32, i32), String> {
  Ok((
    read_i32(data, record_offset + 8)?,
    read_i32(data, record_offset + 12)?,
    read_i32(data, record_offset + 16)?,
    read_i32(data, record_offset + 20)?,
  ))
}

fn angle_from_emf_arc_point(rect: (i32, i32, i32, i32), x: i32, y: i32) -> f32 {
  let (left, top, right, bottom) = rect;
  let cx = (left + right) as f32 / 2.0;
  let cy = (top + bottom) as f32 / 2.0;
  (y as f32 - cy).atan2(x as f32 - cx).to_degrees()
}

fn sweep_from_emf_arc_points(
  data: &[u8],
  record_offset: usize,
  rect: (i32, i32, i32, i32),
) -> Result<f32, String> {
  let start = angle_from_emf_arc_point(
    rect,
    read_i32(data, record_offset + 24)?,
    read_i32(data, record_offset + 28)?,
  );
  let end = angle_from_emf_arc_point(
    rect,
    read_i32(data, record_offset + 32)?,
    read_i32(data, record_offset + 36)?,
  );
  let mut sweep = end - start;
  if sweep <= 0.0 {
    sweep += 360.0;
  }
  Ok(sweep)
}

fn angle_from_arc_point(value: crate::wmf::WmfArcRecord, x: i16, y: i16) -> f32 {
  let cx = (i32::from(value.left) + i32::from(value.right)) as f32 / 2.0;
  let cy = (i32::from(value.top) + i32::from(value.bottom)) as f32 / 2.0;
  (f32::from(y) - cy).atan2(f32::from(x) - cx).to_degrees()
}

fn sweep_from_arc_points(value: crate::wmf::WmfArcRecord) -> f32 {
  let start = angle_from_arc_point(value, value.x_radial_1, value.y_radial_1);
  let end = angle_from_arc_point(value, value.x_radial_2, value.y_radial_2);
  let mut sweep = end - start;
  if sweep <= 0.0 {
    sweep += 360.0;
  }
  sweep
}

fn decoded_raster_to_rgb(raster: &DecodedMetafile) -> Result<Option<RasterPixels>, String> {
  match raster.content_type {
    "image/png" => {
      let image = image::load_from_memory_with_format(&raster.data, image::ImageFormat::Png)
        .map_err(|err| err.to_string())?
        .to_rgb8();
      let (width, height) = image.dimensions();
      Ok(Some(RasterPixels {
        width: width as usize,
        height: height as usize,
        rgb: image.into_raw(),
      }))
    }
    "image/jpeg" => Ok(None),
    _ => Ok(None),
  }
}

fn decoded_png_to_rgb(raster: &DecodedMetafile) -> Result<RasterPixels, String> {
  decoded_raster_to_rgb(raster)?
    .ok_or_else(|| "metafile transparent replay did not produce a PNG raster".to_string())
}

fn straight_rgba_from_black_white(black: &[u8], white: &[u8]) -> Result<Vec<u8>, String> {
  if black.len() != white.len() || !black.len().is_multiple_of(RGB_BYTES_PER_PIXEL) {
    return Err("metafile black/white replay buffers have incompatible lengths".to_string());
  }

  let mut rgba = Vec::with_capacity(black.len() / RGB_BYTES_PER_PIXEL * BGRA_BYTES_PER_PIXEL);
  for (black, white) in black
    .chunks_exact(RGB_BYTES_PER_PIXEL)
    .zip(white.chunks_exact(RGB_BYTES_PER_PIXEL))
  {
    let uncovered = white
      .iter()
      .zip(black)
      .map(|(white, black)| white.saturating_sub(*black))
      .max()
      .unwrap_or(u8::MAX);
    let alpha = u8::MAX - uncovered;
    if alpha == 0 {
      rgba.extend_from_slice(&[0, 0, 0, 0]);
      continue;
    }
    for channel in black {
      let straight =
        (u32::from(*channel) * u32::from(u8::MAX) + u32::from(alpha) / 2) / u32::from(alpha);
      rgba.push(straight.min(u32::from(u8::MAX)) as u8);
    }
    rgba.push(alpha);
  }
  Ok(rgba)
}

fn bitmap16_to_rgb(data: &[u8]) -> Result<Option<RasterPixels>, String> {
  let bitmap = crate::wmf::WmfBitmap16::read_from_slice(data).map_err(|err| err.to_string())?;
  let width = bitmap.header.width.max(1) as usize;
  let height = bitmap.header.height.max(1) as usize;
  let bits_pixel = bitmap.header.bits_pixel;
  let stride = bitmap
    .header
    .computed_width_bytes()
    .map_err(|err| err.to_string())?;
  let required = stride
    .checked_mul(height)
    .ok_or_else(|| "Bitmap16 dimensions overflow".to_string())?;
  if bitmap.bits.len() < required {
    return Err("Bitmap16 bits are truncated".to_string());
  }
  let mut rgb = vec![0u8; width * height * RGB_BYTES_PER_PIXEL];
  for row in 0..height {
    let src_row = height - 1 - row;
    let src = &bitmap.bits[src_row * stride..src_row * stride + stride];
    let dest = &mut rgb[row * width * RGB_BYTES_PER_PIXEL..(row + 1) * width * RGB_BYTES_PER_PIXEL];
    match bits_pixel {
      1 => {
        for col in 0..width {
          let bit = (src[col / 8] >> (7 - (col % 8))) & 1;
          let value = if bit == 0 { 0 } else { 255 };
          let offset = col * RGB_BYTES_PER_PIXEL;
          dest[offset] = value;
          dest[offset + 1] = value;
          dest[offset + 2] = value;
        }
      }
      8 => {
        for (col, value) in src.iter().copied().enumerate().take(width) {
          let offset = col * RGB_BYTES_PER_PIXEL;
          dest[offset] = value;
          dest[offset + 1] = value;
          dest[offset + 2] = value;
        }
      }
      24 => {
        for col in 0..width {
          let src_pixel =
            &src[col * RGB_BYTES_PER_PIXEL..col * RGB_BYTES_PER_PIXEL + RGB_BYTES_PER_PIXEL];
          let dest_pixel =
            &mut dest[col * RGB_BYTES_PER_PIXEL..col * RGB_BYTES_PER_PIXEL + RGB_BYTES_PER_PIXEL];
          dest_pixel[0] = src_pixel[2];
          dest_pixel[1] = src_pixel[1];
          dest_pixel[2] = src_pixel[0];
        }
      }
      _ => return Ok(None),
    }
  }
  Ok(Some(RasterPixels { width, height, rgb }))
}

fn packed_dib_to_rgb(
  data: &[u8],
  color_usage: DibColorUsage,
) -> Result<Option<RasterPixels>, String> {
  packed_dib_to_rgb_with_palette_override(data, color_usage, None)
}

fn packed_dib_to_rgb_with_palette_override(
  data: &[u8],
  color_usage: DibColorUsage,
  monochrome_palette_override: Option<[[u8; 3]; 2]>,
) -> Result<Option<RasterPixels>, String> {
  let dib =
    DeviceIndependentBitmap::from_packed_slice(data, color_usage).map_err(|err| err.to_string())?;
  device_independent_bitmap_to_rgb(&dib, color_usage, monochrome_palette_override)
}

fn device_independent_bitmap_to_rgb(
  dib: &DeviceIndependentBitmap,
  color_usage: DibColorUsage,
  monochrome_palette_override: Option<[[u8; 3]; 2]>,
) -> Result<Option<RasterPixels>, String> {
  match dib.info.header.compression_kind() {
    Some(BitmapCompression::Png) => {
      let image = image::load_from_memory_with_format(&dib.bits, image::ImageFormat::Png)
        .map_err(|err| err.to_string())?
        .to_rgb8();
      let (width, height) = image.dimensions();
      Ok(Some(RasterPixels {
        width: width as usize,
        height: height as usize,
        rgb: image.into_raw(),
      }))
    }
    Some(BitmapCompression::Jpeg) => Ok(None),
    Some(BitmapCompression::Rgb) => dib_rgb_bits_to_rgb(
      &dib.info.header,
      &dib.bits,
      &dib.info,
      color_usage,
      monochrome_palette_override,
    )
    .map(Some),
    Some(BitmapCompression::Bitfields) => {
      dib_bitfields_to_rgb(&dib.info.header, &dib.bits, &dib.info).map(Some)
    }
    Some(BitmapCompression::Rle8) | Some(BitmapCompression::Rle4) => {
      dib_rle_to_rgb(&dib.info.header, &dib.bits, &dib.info, color_usage).map(Some)
    }
    _ => Ok(None),
  }
}

fn dib_rgb_bits_to_rgb(
  header: &DibHeader,
  bits: &[u8],
  info: &crate::DibBitmapInfo,
  color_usage: DibColorUsage,
  monochrome_palette_override: Option<[[u8; 3]; 2]>,
) -> Result<RasterPixels, String> {
  let width = header.width();
  let height = header.height();
  if width <= 0 || height == 0 {
    return Err(format!("unsupported DIB size {width}x{height}"));
  }
  let width = width as usize;
  let height_abs = header.height_abs() as usize;
  let bit_count = header.bit_count();
  let row_stride = header
    .scan_line_stride_bytes()
    .map_err(|err| err.to_string())? as usize;
  let required_size = row_stride
    .checked_mul(height_abs)
    .ok_or_else(|| "DIB dimensions overflow".to_string())?;
  if bits.len() < required_size {
    return Err(format!(
      "DIB payload is truncated: need {required_size} bytes, got {}",
      bits.len()
    ));
  }
  let mut palette = match bit_count {
    1 | 4 | 8 => match info
      .parse_color_table(color_usage)
      .map_err(|err| err.to_string())?
    {
      DibColorTable::RgbQuads { entries, .. } => entries,
      _ => Vec::new(),
    },
    _ => Vec::new(),
  };
  if bit_count == 1
    && let Some(colors) = monochrome_palette_override
  {
    palette = colors
      .map(|[red, green, blue]| crate::RgbQuad {
        blue,
        green,
        red,
        reserved: 0,
      })
      .to_vec();
  }
  let mut rgb = vec![0u8; width * height_abs * RGB_BYTES_PER_PIXEL];
  for row in 0..height_abs {
    let src_row = if header.is_top_down() {
      row
    } else {
      height_abs - 1 - row
    };
    let src = &bits[src_row * row_stride..src_row * row_stride + row_stride];
    let dest = &mut rgb[row * width * RGB_BYTES_PER_PIXEL..(row + 1) * width * RGB_BYTES_PER_PIXEL];
    match bit_count {
      1 => {
        for col in 0..width {
          let byte = src[col / 8];
          let index = ((byte >> (7 - (col % 8))) & 0x01) as usize;
          write_palette_pixel(dest, col, &palette, index);
        }
      }
      4 => {
        for col in 0..width {
          let byte = src[col / 2];
          let index = if col.is_multiple_of(2) {
            (byte >> 4) as usize
          } else {
            (byte & 0x0f) as usize
          };
          write_palette_pixel(dest, col, &palette, index);
        }
      }
      8 => {
        for (col, index) in src.iter().copied().enumerate().take(width) {
          write_palette_pixel(dest, col, &palette, index as usize);
        }
      }
      24 => {
        for col in 0..width {
          let src_pixel =
            &src[col * RGB_BYTES_PER_PIXEL..col * RGB_BYTES_PER_PIXEL + RGB_BYTES_PER_PIXEL];
          let dest_pixel =
            &mut dest[col * RGB_BYTES_PER_PIXEL..col * RGB_BYTES_PER_PIXEL + RGB_BYTES_PER_PIXEL];
          dest_pixel[0] = src_pixel[2];
          dest_pixel[1] = src_pixel[1];
          dest_pixel[2] = src_pixel[0];
        }
      }
      32 => {
        for col in 0..width {
          let src_pixel =
            &src[col * BGRA_BYTES_PER_PIXEL..col * BGRA_BYTES_PER_PIXEL + BGRA_BYTES_PER_PIXEL];
          let dest_pixel =
            &mut dest[col * RGB_BYTES_PER_PIXEL..col * RGB_BYTES_PER_PIXEL + RGB_BYTES_PER_PIXEL];
          dest_pixel[0] = src_pixel[2];
          dest_pixel[1] = src_pixel[1];
          dest_pixel[2] = src_pixel[0];
        }
      }
      other => return Err(format!("unsupported BI_RGB bit depth: {other}")),
    }
  }
  Ok(RasterPixels {
    width,
    height: height_abs,
    rgb,
  })
}

fn dib_bitfields_to_rgb(
  header: &DibHeader,
  bits: &[u8],
  info: &crate::DibBitmapInfo,
) -> Result<RasterPixels, String> {
  let width = header.width();
  let height = header.height();
  if width <= 0 || height == 0 {
    return Err(format!("unsupported DIB size {width}x{height}"));
  }
  let width = width as usize;
  let height_abs = header.height_abs() as usize;
  let bit_count = header.bit_count();
  let bytes_per_pixel = match bit_count {
    16 => 2,
    32 => 4,
    other => return Err(format!("unsupported BI_BITFIELDS bit depth: {other}")),
  };
  let row_stride = header
    .scan_line_stride_bytes()
    .map_err(|err| err.to_string())? as usize;
  let required_size = row_stride
    .checked_mul(height_abs)
    .ok_or_else(|| "DIB dimensions overflow".to_string())?;
  if bits.len() < required_size {
    return Err(format!(
      "DIB payload is truncated: need {required_size} bytes, got {}",
      bits.len()
    ));
  }
  let masks = info.bitfield_masks().map_err(|err| err.to_string())?;
  let masks = masks.unwrap_or(match bit_count {
    16 => [0x7C00, 0x03E0, 0x001F],
    32 => [0x00FF_0000, 0x0000_FF00, 0x0000_00FF],
    _ => unreachable!(),
  });
  let mut rgb = vec![0u8; width * height_abs * RGB_BYTES_PER_PIXEL];
  for row in 0..height_abs {
    let src_row = if header.is_top_down() {
      row
    } else {
      height_abs - 1 - row
    };
    let src = &bits[src_row * row_stride..src_row * row_stride + row_stride];
    let dest = &mut rgb[row * width * RGB_BYTES_PER_PIXEL..(row + 1) * width * RGB_BYTES_PER_PIXEL];
    for col in 0..width {
      let offset = col * bytes_per_pixel;
      let value = if bytes_per_pixel == 2 {
        u32::from(u16::from_le_bytes([src[offset], src[offset + 1]]))
      } else {
        u32::from_le_bytes([
          src[offset],
          src[offset + 1],
          src[offset + 2],
          src[offset + 3],
        ])
      };
      let dest_pixel =
        &mut dest[col * RGB_BYTES_PER_PIXEL..col * RGB_BYTES_PER_PIXEL + RGB_BYTES_PER_PIXEL];
      dest_pixel[0] = bitfield_channel(value, masks[0]);
      dest_pixel[1] = bitfield_channel(value, masks[1]);
      dest_pixel[2] = bitfield_channel(value, masks[2]);
    }
  }
  Ok(RasterPixels {
    width,
    height: height_abs,
    rgb,
  })
}

fn bitfield_channel(value: u32, mask: u32) -> u8 {
  if mask == 0 {
    return 0;
  }
  let shift = mask.trailing_zeros();
  let bits = mask.count_ones();
  let raw = (value & mask) >> shift;
  let max = (1u32 << bits) - 1;
  ((raw * 255 + max / 2) / max) as u8
}

fn dib_rle_to_rgb(
  header: &DibHeader,
  bits: &[u8],
  info: &crate::DibBitmapInfo,
  color_usage: DibColorUsage,
) -> Result<RasterPixels, String> {
  let width = header.width();
  let height = header.height();
  if width <= 0 || height == 0 {
    return Err(format!("unsupported DIB size {width}x{height}"));
  }
  let width = width as usize;
  let height_abs = header.height_abs() as usize;
  let palette = match info
    .parse_color_table(color_usage)
    .map_err(|err| err.to_string())?
  {
    DibColorTable::RgbQuads { entries, .. } => entries,
    _ => Vec::new(),
  };
  let indices = match header.compression_kind() {
    Some(BitmapCompression::Rle8) => decode_rle8_indices(bits, width, height_abs)?,
    Some(BitmapCompression::Rle4) => decode_rle4_indices(bits, width, height_abs)?,
    other => return Err(format!("unsupported RLE compression: {other:?}")),
  };
  let mut rgb = vec![0u8; width * height_abs * RGB_BYTES_PER_PIXEL];
  for row in 0..height_abs {
    let src_row = if header.is_top_down() {
      row
    } else {
      height_abs - 1 - row
    };
    let dest = &mut rgb[row * width * RGB_BYTES_PER_PIXEL..(row + 1) * width * RGB_BYTES_PER_PIXEL];
    for col in 0..width {
      write_palette_pixel(dest, col, &palette, indices[src_row * width + col] as usize);
    }
  }
  Ok(RasterPixels {
    width,
    height: height_abs,
    rgb,
  })
}

fn decode_rle8_indices(bits: &[u8], width: usize, height: usize) -> Result<Vec<u8>, String> {
  let mut out = vec![0u8; width * height];
  let mut x = 0usize;
  let mut y = 0usize;
  let mut pos = 0usize;
  while pos + 1 < bits.len() && y < height {
    let count = bits[pos];
    let value = bits[pos + 1];
    pos += 2;
    if count != 0 {
      for _ in 0..count {
        if x < width && y < height {
          out[y * width + x] = value;
        }
        x = x.saturating_add(1);
      }
      continue;
    }
    match value {
      0 => {
        x = 0;
        y = y.saturating_add(1);
      }
      1 => break,
      2 if pos + 1 < bits.len() => {
        x = x.saturating_add(bits[pos] as usize);
        y = y.saturating_add(bits[pos + 1] as usize);
        pos += 2;
      }
      n => {
        let n = n as usize;
        if pos + n > bits.len() {
          return Err("RLE8 absolute run is truncated".to_string());
        }
        for value in &bits[pos..pos + n] {
          if x < width && y < height {
            out[y * width + x] = *value;
          }
          x = x.saturating_add(1);
        }
        pos += n + (n % 2);
      }
    }
  }
  Ok(out)
}

fn decode_rle4_indices(bits: &[u8], width: usize, height: usize) -> Result<Vec<u8>, String> {
  let mut out = vec![0u8; width * height];
  let mut x = 0usize;
  let mut y = 0usize;
  let mut pos = 0usize;
  while pos + 1 < bits.len() && y < height {
    let count = bits[pos];
    let value = bits[pos + 1];
    pos += 2;
    if count != 0 {
      let high = value >> 4;
      let low = value & 0x0F;
      for index in 0..count as usize {
        if x < width && y < height {
          out[y * width + x] = if index.is_multiple_of(2) { high } else { low };
        }
        x = x.saturating_add(1);
      }
      continue;
    }
    match value {
      0 => {
        x = 0;
        y = y.saturating_add(1);
      }
      1 => break,
      2 if pos + 1 < bits.len() => {
        x = x.saturating_add(bits[pos] as usize);
        y = y.saturating_add(bits[pos + 1] as usize);
        pos += 2;
      }
      n => {
        let pixel_count = n as usize;
        let byte_count = pixel_count.div_ceil(2);
        if pos + byte_count > bits.len() {
          return Err("RLE4 absolute run is truncated".to_string());
        }
        for index in 0..pixel_count {
          let byte = bits[pos + index / 2];
          let value = if index.is_multiple_of(2) {
            byte >> 4
          } else {
            byte & 0x0F
          };
          if x < width && y < height {
            out[y * width + x] = value;
          }
          x = x.saturating_add(1);
        }
        pos += byte_count + (byte_count % 2);
      }
    }
  }
  Ok(out)
}

fn write_palette_pixel(dest: &mut [u8], col: usize, palette: &[crate::RgbQuad], index: usize) {
  let Some(color) = palette.get(index) else {
    return;
  };
  let dest_pixel =
    &mut dest[col * RGB_BYTES_PER_PIXEL..col * RGB_BYTES_PER_PIXEL + RGB_BYTES_PER_PIXEL];
  dest_pixel[0] = color.red;
  dest_pixel[1] = color.green;
  dest_pixel[2] = color.blue;
}

fn draw_glyph_5x7(
  state: &mut EmfVectorState,
  x: i32,
  y: i32,
  ch: char,
  color: EmfColor,
  scale: usize,
) {
  let glyph = glyph_5x7(ch);
  for (row, bits) in glyph.iter().copied().enumerate() {
    for col in 0..5 {
      if bits & (1 << (4 - col)) == 0 {
        continue;
      }
      for yy in 0..scale {
        for xx in 0..scale {
          state.set_vector_pixel(
            x + (col * scale + xx) as i32,
            y + (row * scale + yy) as i32,
            color,
          );
        }
      }
    }
  }
}

fn glyph_5x7(ch: char) -> [u8; 7] {
  match ch.to_ascii_uppercase() {
    '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
    '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
    '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
    '3' => [0x1E, 0x01, 0x01, 0x0E, 0x01, 0x01, 0x1E],
    '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
    '5' => [0x1F, 0x10, 0x10, 0x1E, 0x01, 0x01, 0x1E],
    '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
    '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
    '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
    '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
    'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
    'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
    'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
    'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
    'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
    'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
    'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0E],
    'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
    'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
    'J' => [0x01, 0x01, 0x01, 0x01, 0x11, 0x11, 0x0E],
    'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
    'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
    'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
    'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
    'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
    'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
    'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
    'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
    'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
    'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
    'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
    'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
    'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A],
    'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
    'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
    'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
    '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
    '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
    ',' => [0x00, 0x00, 0x00, 0x00, 0x0C, 0x04, 0x08],
    ':' => [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00],
    '/' => [0x01, 0x01, 0x02, 0x04, 0x08, 0x10, 0x10],
    _ => [0x1F, 0x11, 0x15, 0x15, 0x15, 0x11, 0x1F],
  }
}

fn clamp_canvas_size(width: usize, height: usize, max_pixels: Option<u32>) -> (usize, usize) {
  let max_pixels = max_pixels.unwrap_or(DEFAULT_MAX_PIXELS as u32).max(1) as usize;
  match width.checked_mul(height) {
    Some(pixels) if pixels <= max_pixels => (width, height),
    Some(pixels) => {
      let scale = (max_pixels as f64 / pixels as f64).sqrt();
      (
        ((width as f64 * scale).round() as usize).max(1),
        ((height as f64 * scale).round() as usize).max(1),
      )
    }
    None => (DEFAULT_RENDER_WIDTH, DEFAULT_RENDER_HEIGHT),
  }
}

fn visit_polygon_scanline_spans(
  points: &[(f32, f32)],
  width: usize,
  height: usize,
  mut visit: impl FnMut(usize, usize, usize),
) {
  if points.len() < 3 || width == 0 || height == 0 {
    return;
  }

  let mut min_y = f32::INFINITY;
  let mut max_y = f32::NEG_INFINITY;
  for &(_, y) in points {
    if y.is_finite() {
      min_y = min_y.min(y);
      max_y = max_y.max(y);
    }
  }
  if !min_y.is_finite() || !max_y.is_finite() {
    return;
  }

  // A scanline samples at y + 0.5, so rows wholly outside the polygon's
  // vertical bounds cannot contribute. Retaining one floor/ceil boundary
  // row preserves the existing edge rule while avoiding a full-canvas scan
  // for every small metafile polygon.
  let start_y = min_y.floor().max(0.0).min(height as f32) as usize;
  let end_y = max_y.ceil().max(0.0).min(height as f32) as usize;
  let mut intersections = Vec::new();
  for y in start_y..end_y {
    let scan_y = y as f32 + 0.5;
    intersections.clear();
    for index in 0..points.len() {
      let (x1, y1) = points[index];
      let (x2, y2) = points[(index + 1) % points.len()];
      if (y1 <= scan_y && y2 > scan_y) || (y2 <= scan_y && y1 > scan_y) {
        let t = (scan_y - y1) / (y2 - y1);
        intersections.push(x1 + t * (x2 - x1));
      }
    }
    intersections.sort_by(|a, b| a.total_cmp(b));
    for pair in intersections.chunks_exact(2) {
      // Sample coverage at pixel centers and keep the trailing polygon edge
      // half-open. Adjacent polygons emitted for GDI gradients share that
      // edge; rounding both intersections outward paints it twice, which is
      // visibly wrong under R2_XORPEN.
      let start_x = (pair[0] - 0.5).ceil().max(0.0).min(width as f32) as usize;
      let end_x = (pair[1] - 0.5).ceil().max(0.0).min(width as f32) as usize;
      if end_x > start_x {
        visit(y, start_x, end_x);
      }
    }
  }
}

fn axis_aligned_clip_rect(
  points: &[(f32, f32)],
  width: usize,
  height: usize,
) -> Option<(i32, i32, i32, i32)> {
  let points = if points.len() == 5 && points_approximately_equal(points[0], points[4]) {
    &points[..4]
  } else {
    points
  };
  if points.len() != 4 || points.iter().any(|(x, y)| !x.is_finite() || !y.is_finite()) {
    return None;
  }
  for index in 0..points.len() {
    let current = points[index];
    let next = points[(index + 1) % points.len()];
    if !approximately_equal(current.0, next.0) && !approximately_equal(current.1, next.1) {
      return None;
    }
  }

  let min_x = points
    .iter()
    .map(|point| point.0)
    .fold(f32::INFINITY, f32::min);
  let max_x = points
    .iter()
    .map(|point| point.0)
    .fold(f32::NEG_INFINITY, f32::max);
  let min_y = points
    .iter()
    .map(|point| point.1)
    .fold(f32::INFINITY, f32::min);
  let max_y = points
    .iter()
    .map(|point| point.1)
    .fold(f32::NEG_INFINITY, f32::max);
  Some((
    min_x.floor().max(0.0).min(width as f32) as i32,
    (min_y - 0.5).ceil().max(0.0).min(height as f32) as i32,
    max_x.ceil().max(0.0).min(width as f32) as i32,
    (max_y - 0.5).ceil().max(0.0).min(height as f32) as i32,
  ))
}

fn points_approximately_equal(left: (f32, f32), right: (f32, f32)) -> bool {
  approximately_equal(left.0, right.0) && approximately_equal(left.1, right.1)
}

fn approximately_equal(left: f32, right: f32) -> bool {
  (left - right).abs() <= 0.001
}

fn intersect_rects(
  left: (i32, i32, i32, i32),
  right: (i32, i32, i32, i32),
) -> (i32, i32, i32, i32) {
  let x1 = left.0.max(right.0);
  let y1 = left.1.max(right.1);
  let x2 = left.2.min(right.2).max(x1);
  let y2 = left.3.min(right.3).max(y1);
  (x1, y1, x2, y2)
}

fn clip_line_to_rect(
  start: (f64, f64),
  end: (f64, f64),
  rect: (f64, f64, f64, f64),
) -> Option<((f64, f64), (f64, f64))> {
  if ![
    start.0, start.1, end.0, end.1, rect.0, rect.1, rect.2, rect.3,
  ]
  .iter()
  .all(|value| value.is_finite())
    || rect.2 < rect.0
    || rect.3 < rect.1
  {
    return None;
  }

  let dx = end.0 - start.0;
  let dy = end.1 - start.1;
  let mut first: f64 = 0.0;
  let mut last: f64 = 1.0;
  for (direction, distance) in [
    (-dx, start.0 - rect.0),
    (dx, rect.2 - start.0),
    (-dy, start.1 - rect.1),
    (dy, rect.3 - start.1),
  ] {
    if direction == 0.0 {
      if distance < 0.0 {
        return None;
      }
      continue;
    }
    let ratio = distance / direction;
    if direction < 0.0 {
      first = first.max(ratio);
    } else {
      last = last.min(ratio);
    }
    if first > last {
      return None;
    }
  }

  Some((
    (start.0 + first * dx, start.1 + first * dy),
    (start.0 + last * dx, start.1 + last * dy),
  ))
}

fn read_poly_polygons_i32(
  data: &[u8],
  record_offset: usize,
  record_size: usize,
) -> Result<Vec<Vec<EmfPoint>>, String> {
  let polygon_count = read_u32(data, record_offset + 24)? as usize;
  let total_points = read_u32(data, record_offset + 28)? as usize;
  let counts_offset = record_offset + 32;
  let points_offset = counts_offset
    .checked_add(polygon_count * 4)
    .ok_or_else(|| "EMF polygon counts overflow".to_string())?;
  if points_offset > record_offset + record_size {
    return Ok(Vec::new());
  }
  let mut counts = Vec::with_capacity(polygon_count);
  for index in 0..polygon_count {
    counts.push(read_u32(data, counts_offset + index * 4)? as usize);
  }
  let Some(points) = read_points_i32(data, points_offset, total_points) else {
    return Ok(Vec::new());
  };
  Ok(split_polygons(points, counts))
}

fn read_poly_polygons_i16(
  data: &[u8],
  record_offset: usize,
  record_size: usize,
) -> Result<Vec<Vec<EmfPoint>>, String> {
  let polygon_count = read_u32(data, record_offset + 24)? as usize;
  let total_points = read_u32(data, record_offset + 28)? as usize;
  let counts_offset = record_offset + 32;
  let points_offset = counts_offset
    .checked_add(polygon_count * 4)
    .ok_or_else(|| "EMF polygon counts overflow".to_string())?;
  if points_offset > record_offset + record_size {
    return Ok(Vec::new());
  }
  let mut counts = Vec::with_capacity(polygon_count);
  for index in 0..polygon_count {
    counts.push(read_u32(data, counts_offset + index * 4)? as usize);
  }
  let Some(points) = read_points_i16(data, points_offset, total_points) else {
    return Ok(Vec::new());
  };
  Ok(split_polygons(points, counts))
}

fn split_polygons(points: Vec<EmfPoint>, counts: Vec<usize>) -> Vec<Vec<EmfPoint>> {
  let mut polygons = Vec::with_capacity(counts.len());
  let mut cursor = 0usize;
  for count in counts {
    let end = cursor.saturating_add(count).min(points.len());
    polygons.push(points[cursor..end].to_vec());
    cursor = end;
  }
  polygons
}

fn read_points_i32(data: &[u8], offset: usize, count: usize) -> Option<Vec<EmfPoint>> {
  let end = offset.checked_add(count.checked_mul(8)?)?;
  if end > data.len() {
    return None;
  }
  let mut points = Vec::with_capacity(count);
  for index in 0..count {
    let point_offset = offset + index * 8;
    points.push(EmfPoint {
      x: read_i32(data, point_offset).ok()?,
      y: read_i32(data, point_offset + 4).ok()?,
    });
  }
  Some(points)
}

fn read_points_i16(data: &[u8], offset: usize, count: usize) -> Option<Vec<EmfPoint>> {
  let end = offset.checked_add(count.checked_mul(4)?)?;
  if end > data.len() {
    return None;
  }
  let mut points = Vec::with_capacity(count);
  for index in 0..count {
    let point_offset = offset + index * 4;
    points.push(EmfPoint {
      x: i32::from(read_i16(data, point_offset).ok()?),
      y: i32::from(read_i16(data, point_offset + 2).ok()?),
    });
  }
  Some(points)
}

fn read_color_ref(data: &[u8], offset: usize) -> Result<EmfColor, String> {
  let color_ref = read_u32(data, offset)?;
  Ok(EmfColor {
    r: (color_ref & 0xff) as u8,
    g: ((color_ref >> 8) & 0xff) as u8,
    b: ((color_ref >> 16) & 0xff) as u8,
  })
}

fn read_xform(data: &[u8], offset: usize) -> Result<EmfTransform, String> {
  Ok(EmfTransform {
    m11: read_f32(data, offset)?,
    m12: read_f32(data, offset + 4)?,
    m21: read_f32(data, offset + 8)?,
    m22: read_f32(data, offset + 12)?,
    dx: read_f32(data, offset + 16)?,
    dy: read_f32(data, offset + 20)?,
  })
}

fn rgb_to_png(rgb: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
  let mut output = Vec::new();
  let encoder = PngEncoder::new(&mut output);
  encoder
    .write_image(rgb, width, height, ColorType::Rgb8.into())
    .map_err(|err| err.to_string())?;
  Ok(output)
}

fn rgba_to_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
  let mut output = Vec::new();
  let encoder = PngEncoder::new(&mut output);
  encoder
    .write_image(rgba, width, height, ColorType::Rgba8.into())
    .map_err(|err| err.to_string())?;
  Ok(output)
}

fn emf_natural_canvas_size(data: &[u8]) -> Result<(usize, usize), String> {
  let bounds_width = i64::from(read_i32(data, EMF_BOUNDS_RIGHT_OFFSET)?)
    - i64::from(read_i32(data, EMF_BOUNDS_LEFT_OFFSET)?)
    + 1;
  let bounds_height = i64::from(read_i32(data, EMF_BOUNDS_BOTTOM_OFFSET)?)
    - i64::from(read_i32(data, EMF_BOUNDS_TOP_OFFSET)?)
    + 1;
  let fallback = (
    usize::try_from(bounds_width.max(1)).unwrap_or(1),
    usize::try_from(bounds_height.max(1)).unwrap_or(1),
  );

  let frame_width = (i64::from(read_i32(data, EMF_FRAME_RIGHT_OFFSET)?)
    - i64::from(read_i32(data, EMF_FRAME_LEFT_OFFSET)?))
  .unsigned_abs();
  let frame_height = (i64::from(read_i32(data, EMF_FRAME_BOTTOM_OFFSET)?)
    - i64::from(read_i32(data, EMF_FRAME_TOP_OFFSET)?))
  .unsigned_abs();
  let device_width = read_i32(data, EMF_DEVICE_WIDTH_OFFSET)?.unsigned_abs();
  let device_height = read_i32(data, EMF_DEVICE_HEIGHT_OFFSET)?.unsigned_abs();
  let millimeters_width = read_i32(data, EMF_MILLIMETERS_WIDTH_OFFSET)?.unsigned_abs();
  let millimeters_height = read_i32(data, EMF_MILLIMETERS_HEIGHT_OFFSET)?.unsigned_abs();
  if frame_width == 0
    || frame_height == 0
    || device_width == 0
    || device_height == 0
    || millimeters_width == 0
    || millimeters_height == 0
  {
    return Ok(fallback);
  }

  // [MS-EMF] Header.Frame is in 0.01 mm. Device and Millimeters describe
  // the reference device, so together they recover the authored playback
  // surface in device pixels. Bounds encloses only the marks and must not be
  // used to crop away the surrounding metafile surface.
  let pixel_axis = |frame: u64, device: u32, millimeters: u32| {
    ((frame as f64 * f64::from(device)) / (f64::from(millimeters) * 100.0))
      .round()
      .max(1.0) as usize
  };
  Ok((
    pixel_axis(frame_width, device_width, millimeters_width),
    pixel_axis(frame_height, device_height, millimeters_height),
  ))
}

fn emf_physical_size(data: &[u8]) -> Option<MetafilePhysicalSize> {
  const HUNDREDTHS_OF_MILLIMETER_PER_INCH: f32 = 2_540.0;
  let frame_width = (i64::from(read_i32(data, EMF_FRAME_RIGHT_OFFSET).ok()?)
    - i64::from(read_i32(data, EMF_FRAME_LEFT_OFFSET).ok()?))
  .unsigned_abs();
  let frame_height = (i64::from(read_i32(data, EMF_FRAME_BOTTOM_OFFSET).ok()?)
    - i64::from(read_i32(data, EMF_FRAME_TOP_OFFSET).ok()?))
  .unsigned_abs();
  if frame_width == 0 || frame_height == 0 {
    return None;
  }
  let (natural_width_px, natural_height_px) = emf_natural_canvas_size(data).ok()?;
  Some(MetafilePhysicalSize {
    width_pt: frame_width as f32 * 72.0 / HUNDREDTHS_OF_MILLIMETER_PER_INCH,
    height_pt: frame_height as f32 * 72.0 / HUNDREDTHS_OF_MILLIMETER_PER_INCH,
    natural_width_px: u32::try_from(natural_width_px).ok()?,
    natural_height_px: u32::try_from(natural_height_px).ok()?,
  })
}

fn is_emf(data: &[u8]) -> bool {
  data.len() >= EMF_HEADER_SIZE
    && matches!(read_u32(data, 0), Ok(1))
    && matches!(read_u32(data, 4), Ok(size) if size as usize == EMF_HEADER_SIZE)
}

fn extract_emr_ext_text_out_w(
  data: &[u8],
  record_offset: usize,
  record_size: usize,
) -> Option<String> {
  let units = emr_ext_text_out_w_units(data, record_offset, record_size)?;
  Some(
    String::from_utf16_lossy(&units)
      .trim_end_matches('\0')
      .to_string(),
  )
}

fn extract_semantic_emr_ext_text_out_w(
  data: &[u8],
  record_offset: usize,
  record_size: usize,
) -> Option<String> {
  let units = emr_ext_text_out_w_units(data, record_offset, record_size)?;
  // [MS-EMF] §2.3.5.8 defines each EMR_EXTTEXTOUTW record as an
  // independent UTF-16LE Unicode string. A lone surrogate can still paint a
  // GDI missing-glyph cell, but it has no Unicode scalar value and therefore
  // must not become U+FFFD in searchable semantic text. Raster replay keeps
  // using the lossy decoder above so the visible glyph cell is preserved.
  let text = char::decode_utf16(units)
    .filter_map(|character| character.ok())
    .collect::<String>();
  Some(text.trim_end_matches('\0').to_string())
}

fn emr_ext_text_out_w_units(
  data: &[u8],
  record_offset: usize,
  record_size: usize,
) -> Option<Vec<u16>> {
  let text = ext_text_record(data, record_offset, record_size)?;
  let byte_len = text.characters.checked_mul(2)?;
  let start = record_offset.checked_add(text.string_offset)?;
  let end = start.checked_add(byte_len)?;
  let bytes = data.get(start..end)?;
  let units = bytes
    .chunks_exact(2)
    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
    .collect::<Vec<_>>();
  Some(units)
}

fn extract_emr_ext_text_out_a(
  data: &[u8],
  record_offset: usize,
  record_size: usize,
) -> Option<String> {
  let text = ext_text_record(data, record_offset, record_size)?;
  let start = record_offset.checked_add(text.string_offset)?;
  let end = start.checked_add(text.characters)?;
  let bytes = data.get(start..end)?;
  Some(
    bytes
      .iter()
      .take_while(|byte| **byte != 0)
      .map(|byte| char::from(*byte))
      .collect(),
  )
}

#[derive(Clone, Copy, Debug)]
struct ExtTextRecord {
  graphics_mode: u32,
  x_scale: f32,
  y_scale: f32,
  x: i32,
  y: i32,
  characters: usize,
  string_offset: usize,
  options: u32,
  dx_offset: Option<usize>,
}

fn ext_text_record(data: &[u8], record_offset: usize, record_size: usize) -> Option<ExtTextRecord> {
  // with rclBounds, graphics mode, scales, then EMRTEXT. EMRTEXT::offString is
  // relative to the record start.
  const EMRTEXT_OFFSET: usize = 36;
  const GRAPHICS_MODE_OFFSET: usize = 24;
  const X_SCALE_OFFSET: usize = 28;
  const Y_SCALE_OFFSET: usize = 32;
  const EMRTEXT_REFERENCE_X_OFFSET: usize = EMRTEXT_OFFSET;
  const EMRTEXT_REFERENCE_Y_OFFSET: usize = EMRTEXT_OFFSET + 4;
  const EMRTEXT_CHARS_OFFSET: usize = EMRTEXT_OFFSET + 8;
  const EMRTEXT_STRING_OFFSET: usize = EMRTEXT_OFFSET + 12;
  const EMRTEXT_OPTIONS_OFFSET: usize = EMRTEXT_OFFSET + 16;
  const EMRTEXT_DX_OFFSET: usize = EMRTEXT_OFFSET + 36;
  let minimum_size = EMRTEXT_OFFSET + 40;
  if record_size < minimum_size {
    return None;
  }
  let characters = read_u32(data, record_offset + EMRTEXT_CHARS_OFFSET).ok()? as usize;
  let string_offset = read_u32(data, record_offset + EMRTEXT_STRING_OFFSET).ok()? as usize;
  if characters == 0 || string_offset >= record_size {
    return None;
  }
  Some(ExtTextRecord {
    graphics_mode: read_u32(data, record_offset + GRAPHICS_MODE_OFFSET).ok()?,
    x_scale: read_f32(data, record_offset + X_SCALE_OFFSET).ok()?,
    y_scale: read_f32(data, record_offset + Y_SCALE_OFFSET).ok()?,
    x: read_i32(data, record_offset + EMRTEXT_REFERENCE_X_OFFSET).ok()?,
    y: read_i32(data, record_offset + EMRTEXT_REFERENCE_Y_OFFSET).ok()?,
    characters,
    string_offset,
    options: read_u32(data, record_offset + EMRTEXT_OPTIONS_OFFSET).ok()?,
    dx_offset: match read_u32(data, record_offset + EMRTEXT_DX_OFFSET).ok()? as usize {
      0 => None,
      offset if offset < record_size => Some(offset),
      _ => None,
    },
  })
}

fn ext_text_advances(
  data: &[u8],
  record_offset: usize,
  record_size: usize,
  text: ExtTextRecord,
) -> Option<Vec<i32>> {
  const ETO_PDY: u32 = 0x0000_2000;
  let dx_offset = text.dx_offset?;
  let stride = if text.options & ETO_PDY != 0 { 2 } else { 1 };
  let value_count = text.characters.checked_mul(stride)?;
  let byte_count = value_count.checked_mul(4)?;
  let start = record_offset.checked_add(dx_offset)?;
  let end = start.checked_add(byte_count)?;
  if end > record_offset.checked_add(record_size)? || end > data.len() {
    return None;
  }
  (0..text.characters)
    .map(|index| read_i32(data, start + index * stride * 4).ok())
    .collect()
}

fn cumulative_mapped_advances(
  logical_advances: &[i32],
  mut map_cumulative: impl FnMut(i64) -> f32,
) -> Vec<f32> {
  let mut logical_cumulative = 0i64;
  let mut mapped_previous = 0.0f32;
  logical_advances
    .iter()
    .map(|advance| {
      logical_cumulative = logical_cumulative.saturating_add(i64::from(*advance));
      // ExtTextOut Dx values define consecutive logical advances, but their
      // device positions are obtained by mapping cumulative distances. Mapping
      // each small delta independently accumulates fractional pixel error.
      let mapped_cumulative = map_cumulative(logical_cumulative);
      let mapped_advance = mapped_cumulative - mapped_previous;
      mapped_previous = mapped_cumulative;
      mapped_advance
    })
    .collect()
}

fn read_logfont_object(
  data: &[u8],
  record_offset: usize,
  record_size: usize,
) -> Option<(u32, EmfFont)> {
  // EMR_EXTCREATEFONTINDIRECTW reads an object index followed by LOGFONTW.
  const OBJECT_ID_OFFSET: usize = 8;
  const LOGFONT_OFFSET: usize = 12;
  const LOGFONT_HEIGHT_OFFSET: usize = LOGFONT_OFFSET;
  const LOGFONT_WEIGHT_OFFSET: usize = LOGFONT_OFFSET + 16;
  const LOGFONT_ITALIC_OFFSET: usize = LOGFONT_OFFSET + 20;
  const LOGFONT_FACE_NAME_OFFSET: usize = LOGFONT_OFFSET + 28;
  let face_end = LOGFONT_FACE_NAME_OFFSET.checked_add(LOGFONT_FACE_NAME_CHARS * 2)?;
  if record_size < face_end {
    return None;
  }
  let object_id = read_u32(data, record_offset + OBJECT_ID_OFFSET).ok()?;
  let height = read_i32(data, record_offset + LOGFONT_HEIGHT_OFFSET).ok()?;
  let weight = read_i32(data, record_offset + LOGFONT_WEIGHT_OFFSET)
    .ok()?
    .clamp(0, 1000) as u16;
  let italic = *data.get(record_offset + LOGFONT_ITALIC_OFFSET)? != 0;
  let face_bytes = data.get(
    record_offset + LOGFONT_FACE_NAME_OFFSET
      ..record_offset + LOGFONT_FACE_NAME_OFFSET + LOGFONT_FACE_NAME_CHARS * 2,
  )?;
  let family = String::from_utf16_lossy(
    &face_bytes
      .chunks_exact(2)
      .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
      .take_while(|unit| *unit != 0)
      .collect::<Vec<_>>(),
  );
  Some((
    object_id,
    EmfFont {
      height,
      family: (!family.is_empty()).then_some(family),
      weight,
      italic,
    },
  ))
}

fn read_i16(data: &[u8], offset: usize) -> Result<i16, String> {
  let bytes = data
    .get(offset..offset + 2)
    .ok_or_else(|| format!("read past end of buffer at offset {offset}"))?;
  Ok(i16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32, String> {
  let bytes = data
    .get(offset..offset + 4)
    .ok_or_else(|| format!("read past end of buffer at offset {offset}"))?;
  Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_i32(data: &[u8], offset: usize) -> Result<i32, String> {
  let bytes = data
    .get(offset..offset + 4)
    .ok_or_else(|| format!("read past end of buffer at offset {offset}"))?;
  Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_f32(data: &[u8], offset: usize) -> Result<f32, String> {
  let bytes = data
    .get(offset..offset + 4)
    .ok_or_else(|| format!("read past end of buffer at offset {offset}"))?;
  Ok(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn apply_binary_raster_operation(
  pen: EmfColor,
  destination: EmfColor,
  operation: WmfBinaryRasterOperation,
) -> EmfColor {
  let apply = |pen: u8, destination: u8| match operation {
    WmfBinaryRasterOperation::Black => 0,
    WmfBinaryRasterOperation::NotMergePen => !(destination | pen),
    WmfBinaryRasterOperation::MaskNotPen => destination & !pen,
    WmfBinaryRasterOperation::NotCopyPen => !pen,
    WmfBinaryRasterOperation::MaskPenNot => pen & !destination,
    WmfBinaryRasterOperation::Not => !destination,
    WmfBinaryRasterOperation::XorPen => destination ^ pen,
    WmfBinaryRasterOperation::NotMaskPen => !(destination & pen),
    WmfBinaryRasterOperation::MaskPen => destination & pen,
    WmfBinaryRasterOperation::NotXorPen => !(destination ^ pen),
    WmfBinaryRasterOperation::Nop => destination,
    WmfBinaryRasterOperation::MergeNotPen => destination | !pen,
    WmfBinaryRasterOperation::CopyPen => pen,
    WmfBinaryRasterOperation::MergePenNot => pen | !destination,
    WmfBinaryRasterOperation::MergePen => destination | pen,
    WmfBinaryRasterOperation::White => u8::MAX,
  };
  EmfColor {
    r: apply(pen.r, destination.r),
    g: apply(pen.g, destination.g),
    b: apply(pen.b, destination.b),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::emf::EmrStretchBlt;
  use crate::wmf::{
    WmfColorRecord, WmfDibCreatePatternBrushRecord, WmfExtTextOutRecord, WmfMetafileType,
    WmfMetafileVersion, WmfObjectIndexRecord, WmfPatBltRecord, WmfPointRecord, WmfRectObject,
    WmfSetPixelRecord,
  };
  use crate::{
    BitmapSourceBounds, ColorRef, DibColorUsage, EMR_EOF, EMR_HEADER, EmfMetafile, EmfRecord,
    EmfRecordData, EmrBitmapBuffer, EmrStretchDiBits, PointL, RectL, SdkEnumValue, SizeL,
    WmfHeader, WmfMetafile, WmfRecord, WmfRecordData, XForm,
  };

  fn minimal_header_record() -> EmfRecord {
    let mut data = vec![0; 100];
    data[8..12].copy_from_slice(&1i32.to_le_bytes());
    data[12..16].copy_from_slice(&1i32.to_le_bytes());
    data[32..36].copy_from_slice(&crate::emf::EMF_SIGNATURE.to_le_bytes());
    EmfRecord::new(EMR_HEADER, data)
  }

  fn eof_record() -> EmfRecord {
    EmfRecord::new(EMR_EOF, vec![0; 12])
  }

  fn set_text_align_record(alignment: WmfTextAlignmentModeFlags) -> EmfRecord {
    EmfRecord::new(
      super::EMR_SET_TEXT_ALIGN,
      u32::from(alignment.bits()).to_le_bytes().to_vec(),
    )
  }

  fn ext_text_out_w_record(x: i32, y: i32, text: &str) -> EmfRecord {
    let units = text.encode_utf16().collect::<Vec<_>>();
    let mut data = vec![0; 68];
    data[16..20].copy_from_slice(&1u32.to_le_bytes());
    data[20..24].copy_from_slice(&1.0f32.to_le_bytes());
    data[24..28].copy_from_slice(&1.0f32.to_le_bytes());
    data[28..32].copy_from_slice(&x.to_le_bytes());
    data[32..36].copy_from_slice(&y.to_le_bytes());
    data[36..40].copy_from_slice(&(units.len() as u32).to_le_bytes());
    data[40..44].copy_from_slice(&76u32.to_le_bytes());
    for unit in units {
      data.extend_from_slice(&unit.to_le_bytes());
    }
    while !data.len().is_multiple_of(4) {
      data.push(0);
    }
    let dx_offset = (data.len() + 8) as u32;
    data[64..68].copy_from_slice(&dx_offset.to_le_bytes());
    for _ in text.encode_utf16() {
      data.extend_from_slice(&8i32.to_le_bytes());
    }
    EmfRecord::new(super::EMR_EXT_TEXTOUT_W, data)
  }

  fn bitmap_info(width: i32, height: i32, bit_count: u16, compression: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&40u32.to_le_bytes());
    bytes.extend_from_slice(&width.to_le_bytes());
    bytes.extend_from_slice(&height.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&bit_count.to_le_bytes());
    bytes.extend_from_slice(&compression.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0i32.to_le_bytes());
    bytes.extend_from_slice(&0i32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes
  }

  #[test]
  fn emf_natural_canvas_uses_frame_and_reference_device() {
    let mut data = vec![0; EMF_HEADER_SIZE];
    data[EMF_BOUNDS_LEFT_OFFSET..EMF_BOUNDS_LEFT_OFFSET + 4].copy_from_slice(&16i32.to_le_bytes());
    data[EMF_BOUNDS_TOP_OFFSET..EMF_BOUNDS_TOP_OFFSET + 4].copy_from_slice(&1i32.to_le_bytes());
    data[EMF_BOUNDS_RIGHT_OFFSET..EMF_BOUNDS_RIGHT_OFFSET + 4]
      .copy_from_slice(&84i32.to_le_bytes());
    data[EMF_BOUNDS_BOTTOM_OFFSET..EMF_BOUNDS_BOTTOM_OFFSET + 4]
      .copy_from_slice(&46i32.to_le_bytes());
    data[EMF_FRAME_RIGHT_OFFSET..EMF_FRAME_RIGHT_OFFSET + 4]
      .copy_from_slice(&2580i32.to_le_bytes());
    data[EMF_FRAME_BOTTOM_OFFSET..EMF_FRAME_BOTTOM_OFFSET + 4]
      .copy_from_slice(&1597i32.to_le_bytes());
    data[EMF_DEVICE_WIDTH_OFFSET..EMF_DEVICE_WIDTH_OFFSET + 4]
      .copy_from_slice(&1920i32.to_le_bytes());
    data[EMF_DEVICE_HEIGHT_OFFSET..EMF_DEVICE_HEIGHT_OFFSET + 4]
      .copy_from_slice(&1080i32.to_le_bytes());
    data[EMF_MILLIMETERS_WIDTH_OFFSET..EMF_MILLIMETERS_WIDTH_OFFSET + 4]
      .copy_from_slice(&480i32.to_le_bytes());
    data[EMF_MILLIMETERS_HEIGHT_OFFSET..EMF_MILLIMETERS_HEIGHT_OFFSET + 4]
      .copy_from_slice(&260i32.to_le_bytes());

    assert_eq!(emf_natural_canvas_size(&data).unwrap(), (103, 66));
    let physical = emf_physical_size(&data).unwrap();
    assert!((physical.width_pt - 73.133_86).abs() < 0.000_1);
    assert!((physical.height_pt - 45.269_29).abs() < 0.000_1);
    assert_eq!(physical.natural_width_px, 103);
    assert_eq!(physical.natural_height_px, 66);
  }

  #[test]
  fn emf_logfont_preserves_visible_text_face_properties() {
    let mut record = vec![0; 104];
    record[8..12].copy_from_slice(&7u32.to_le_bytes());
    record[12..16].copy_from_slice(&(-11i32).to_le_bytes());
    record[28..32].copy_from_slice(&700i32.to_le_bytes());
    record[32] = 1;
    for (index, unit) in "Segoe UI".encode_utf16().enumerate() {
      let offset = 40 + index * 2;
      record[offset..offset + 2].copy_from_slice(&unit.to_le_bytes());
    }

    let (object_id, font) = read_logfont_object(&record, 0, record.len()).unwrap();
    assert_eq!(object_id, 7);
    assert_eq!(font.height, -11);
    assert_eq!(font.family.as_deref(), Some("Segoe UI"));
    assert_eq!(font.weight, 700);
    assert!(font.italic);
  }

  #[test]
  fn emf_ext_text_out_preserves_graphics_scales_and_cumulative_dx_mapping() {
    let mut record = vec![0; 96];
    record[24..28].copy_from_slice(&1u32.to_le_bytes());
    record[28..32].copy_from_slice(&25.0f32.to_le_bytes());
    record[32..36].copy_from_slice(&24.074_074f32.to_le_bytes());
    record[36..40].copy_from_slice(&16i32.to_le_bytes());
    record[40..44].copy_from_slice(&34i32.to_le_bytes());
    record[44..48].copy_from_slice(&2u32.to_le_bytes());
    record[48..52].copy_from_slice(&80u32.to_le_bytes());
    record[72..76].copy_from_slice(&84u32.to_le_bytes());
    record[84..88].copy_from_slice(&6i32.to_le_bytes());
    record[88..92].copy_from_slice(&3i32.to_le_bytes());

    let text = ext_text_record(&record, 0, record.len()).unwrap();
    assert_eq!(text.graphics_mode, 1);
    assert_eq!(text.x_scale, 25.0);
    assert_eq!(text.y_scale, 24.074_074);
    assert_eq!((text.x, text.y), (16, 34));
    assert_eq!(
      ext_text_advances(&record, 0, record.len(), text).unwrap(),
      [6, 3]
    );

    let mapped = cumulative_mapped_advances(&[6, 3, 4], |logical| {
      (logical as f32 * 214.0 / 103.0).round()
    });
    assert_eq!(mapped, [12.0, 7.0, 8.0]);
  }

  #[test]
  fn emf_semantic_text_honors_text_alignment_reference_point() {
    let top = metafile_with(ext_text_out_w_record(0, 0, "A"));
    let baseline = metafile_with_records(vec![
      set_text_align_record(WmfTextAlignmentModeFlags::BASELINE),
      ext_text_out_w_record(0, 0, "A"),
    ]);

    let top_run = extract_metafile_text_runs(&top, Some("image/x-emf"))
      .pop()
      .expect("default TA_TOP text");
    let baseline_run = extract_metafile_text_runs(&baseline, Some("image/x-emf"))
      .pop()
      .expect("TA_BASELINE text");

    assert!(top_run.y > baseline_run.y);
    assert_eq!(baseline_run.y, 0.0);
  }

  #[test]
  fn emf_semantic_text_honors_horizontal_text_alignment() {
    let left = metafile_with(ext_text_out_w_record(20, 0, "AB"));
    let center = metafile_with_records(vec![
      set_text_align_record(WmfTextAlignmentModeFlags::CENTER),
      ext_text_out_w_record(20, 0, "AB"),
    ]);
    let right = metafile_with_records(vec![
      set_text_align_record(WmfTextAlignmentModeFlags::RIGHT),
      ext_text_out_w_record(20, 0, "AB"),
    ]);

    let left_x = extract_metafile_text_runs(&left, Some("image/x-emf"))[0].x;
    let center_x = extract_metafile_text_runs(&center, Some("image/x-emf"))[0].x;
    let right_x = extract_metafile_text_runs(&right, Some("image/x-emf"))[0].x;
    let left_width = extract_metafile_text_runs(&left, Some("image/x-emf"))[0]
      .width
      .expect("Dx width");

    assert!(right_x < center_x);
    assert!(center_x < left_x);
    assert_eq!(left_width, 8.0);
  }

  #[test]
  fn line_clip_limits_huge_off_canvas_segments() {
    let clipped = clip_line_to_rect(
      (-1_000_000_000.0, 5.0),
      (1_000_000_000.0, 5.0),
      (0.0, 0.0, 9.0, 9.0),
    )
    .expect("horizontal line crosses the canvas");
    assert!((clipped.0.0 - 0.0).abs() < 0.001);
    assert!((clipped.0.1 - 5.0).abs() < 0.001);
    assert!((clipped.1.0 - 9.0).abs() < 0.001);
    assert!((clipped.1.1 - 5.0).abs() < 0.001);

    assert_eq!(
      clip_line_to_rect(
        (-1_000_000_000.0, -5.0),
        (1_000_000_000.0, -5.0),
        (0.0, 0.0, 9.0, 9.0),
      ),
      None
    );
  }

  #[test]
  fn render_target_defines_the_playback_viewport_in_both_directions() {
    assert_eq!(
      RenderOptions {
        target_width_px: Some(200),
        target_height_px: Some(100),
        max_pixels: None,
        transparent_background: false,
        background_color: None,
        monochrome_dib_palette_override: None,
        filter_high_frequency_pattern_brushes: false,
      }
      .resolved_canvas_size(400, 300),
      (200, 100)
    );
    assert_eq!(
      RenderOptions {
        target_width_px: Some(400),
        target_height_px: Some(300),
        max_pixels: None,
        transparent_background: false,
        background_color: None,
        monochrome_dib_palette_override: None,
        filter_high_frequency_pattern_brushes: false,
      }
      .resolved_canvas_size(76, 76),
      (400, 300)
    );

    let mut state = EmfVectorState::new_with_options(
      &metafile_with_records(Vec::new()),
      RenderOptions {
        target_width_px: Some(4),
        target_height_px: Some(3),
        ..RenderOptions::default()
      },
    )
    .expect("minimal EMF playback state");
    // EMR_SET{WINDOW,VIEWPORT}EXT records describe the metafile's logical to
    // natural-device mapping. They must not discard the player's outer
    // target-viewport transform.
    let (natural_width, natural_height) =
      emf_natural_canvas_size(&metafile_with_records(Vec::new())).unwrap();
    state.window_ext_x = natural_width as i32;
    state.window_ext_y = natural_height as i32;
    state.viewport_ext_x = natural_width as i32;
    state.viewport_ext_y = natural_height as i32;
    assert_eq!(
      state.map_point(EmfPoint {
        x: natural_width as i32,
        y: natural_height as i32,
      }),
      (4.0, 3.0)
    );
  }

  #[test]
  fn wmf_ext_text_out_opaque_fills_background_and_restores_temporary_clip() {
    let records = vec![
      WmfRecordData::SetWindowExt(WmfPointRecord { x: 8, y: 8 })
        .to_record()
        .unwrap(),
      WmfRecordData::SetBkColor(WmfColorRecord {
        color: crate::ColorRef {
          red: 0,
          green: 192,
          blue: 0,
          reserved: 0,
        },
      })
      .to_record()
      .unwrap(),
      WmfRecordData::ExtTextOut(WmfExtTextOutRecord {
        y: 1,
        x: 1,
        string_length: 0,
        options: WmfExtTextOutOptions::OPAQUE | WmfExtTextOutOptions::CLIPPED,
        rectangle: Some(WmfRectObject {
          left: 1,
          top: 1,
          right: 7,
          bottom: 7,
        }),
        string: Vec::new(),
        string_padding: Vec::new(),
        dx: Vec::new(),
        trailing_data: Vec::new(),
      })
      .to_record()
      .unwrap(),
      WmfRecordData::SetPixel(WmfSetPixelRecord {
        color: crate::ColorRef {
          red: 255,
          green: 0,
          blue: 0,
          reserved: 0,
        },
        y: 0,
        x: 0,
      })
      .to_record()
      .unwrap(),
      WmfRecordData::Eof(crate::wmf::WmfEofRecord::default())
        .to_record()
        .unwrap(),
    ];
    let metafile = WmfMetafile {
      placeable_header: None,
      header: WmfHeader {
        metafile_type: WmfMetafileType::Memory.raw(),
        header_size_words: 9,
        version: WmfMetafileVersion::Version300.raw(),
        file_size_words: 0,
        number_of_objects: 0,
        max_record_words: 0,
        number_of_parameters: 0,
      },
      records,
      trailing_data: Vec::new(),
    };

    let decoded = decode_metafile_as_raster(&metafile.to_bytes().unwrap(), Some("image/x-wmf"))
      .unwrap()
      .unwrap();
    let image = image::load_from_memory_with_format(&decoded.data, image::ImageFormat::Png)
      .unwrap()
      .to_rgb8();

    assert_eq!(image.get_pixel(4, 4).0, [0, 192, 0]);
    assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0]);
    assert_eq!(image.get_pixel(7, 7).0, [255, 255, 255]);
  }

  #[test]
  fn wmf_rendering_keeps_valid_output_around_an_unparseable_record() {
    let records = vec![
      WmfRecordData::SetWindowExt(WmfPointRecord { x: 2, y: 2 })
        .to_record()
        .unwrap(),
      WmfRecordData::SetPixel(WmfSetPixelRecord {
        color: crate::ColorRef {
          red: 255,
          green: 0,
          blue: 0,
          reserved: 0,
        },
        x: 0,
        y: 0,
      })
      .to_record()
      .unwrap(),
      // META_ESCAPE with an unsupported EscapeFunction. This is a valid raw
      // WMF record retained by compatibility parsing, but has no typed form.
      WmfRecord::new(crate::wmf::WmfRecordFunction::Escape.raw(), vec![0; 4]),
      WmfRecordData::Eof(crate::wmf::WmfEofRecord::default())
        .to_record()
        .unwrap(),
    ];
    let metafile = WmfMetafile {
      placeable_header: None,
      header: WmfHeader {
        metafile_type: WmfMetafileType::Memory.raw(),
        header_size_words: 9,
        version: WmfMetafileVersion::Version300.raw(),
        file_size_words: 29,
        number_of_objects: 0,
        max_record_words: 7,
        number_of_parameters: 0,
      },
      records,
      trailing_data: Vec::new(),
    };

    let bytes = metafile.to_bytes().unwrap();
    let decoded = decode_metafile_as_raster(&bytes, Some("image/x-wmf"))
      .unwrap()
      .unwrap();
    let image = image::load_from_memory_with_format(&decoded.data, image::ImageFormat::Png)
      .unwrap()
      .to_rgb8();

    assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0]);
  }

  #[test]
  fn wmf_dib_pattern_brush_preserves_its_color_table_on_color_output() {
    let mut pattern = bitmap_info(8, 8, 1, 0);
    pattern.extend_from_slice(&[0, 0, 0, 0, 255, 255, 255, 0]);
    for row in 0..8 {
      pattern.extend_from_slice(&[if row % 2 == 0 { 0xAA } else { 0x55 }, 0, 0, 0]);
    }
    let records = vec![
      WmfRecordData::SetWindowExt(WmfPointRecord { x: 8, y: 8 })
        .to_record()
        .unwrap(),
      WmfRecordData::SetBkColor(WmfColorRecord {
        color: crate::ColorRef {
          red: 255,
          green: 128,
          blue: 255,
          reserved: 0,
        },
      })
      .to_record()
      .unwrap(),
      WmfRecordData::SetTextColor(WmfColorRecord {
        color: crate::ColorRef {
          red: 255,
          green: 255,
          blue: 255,
          reserved: 0,
        },
      })
      .to_record()
      .unwrap(),
      WmfRecordData::DibCreatePatternBrush(WmfDibCreatePatternBrushRecord {
        style: WmfBrushStyle::Pattern.raw(),
        color_usage: DibColorUsage::RgbColors.wmf_raw(),
        target: pattern,
      })
      .to_record()
      .unwrap(),
      WmfRecordData::SelectObject(WmfObjectIndexRecord { index: 0 })
        .to_record()
        .unwrap(),
      WmfRecordData::PatBlt(WmfPatBltRecord {
        raster_operation: 0x00F0_0021,
        height: 8,
        width: 8,
        y_left: 0,
        x_left: 0,
      })
      .to_record()
      .unwrap(),
      WmfRecordData::Eof(crate::wmf::WmfEofRecord::default())
        .to_record()
        .unwrap(),
    ];
    let metafile = WmfMetafile {
      placeable_header: None,
      header: WmfHeader {
        metafile_type: WmfMetafileType::Memory.raw(),
        header_size_words: 9,
        version: WmfMetafileVersion::Version300.raw(),
        file_size_words: 0,
        number_of_objects: 1,
        max_record_words: 0,
        number_of_parameters: 0,
      },
      records,
      trailing_data: Vec::new(),
    };

    let bytes = metafile.to_bytes().unwrap();
    let decoded = decode_metafile_as_raster(&bytes, Some("image/x-wmf"))
      .unwrap()
      .unwrap();
    let image = image::load_from_memory_with_format(&decoded.data, image::ImageFormat::Png)
      .unwrap()
      .to_rgb8();

    assert!(
      image.pixels().any(|pixel| pixel.0 == [0, 0, 0]),
      "the first DIB palette entry remains black"
    );
    assert!(
      image.pixels().any(|pixel| pixel.0 == [255, 255, 255]),
      "the second DIB palette entry remains white"
    );
    assert!(
      !image.pixels().any(|pixel| pixel.0 == [255, 128, 255]),
      "color output does not substitute the DC background color"
    );

    let decoded = decode_metafile_as_raster_with_options(
      &bytes,
      Some("image/x-wmf"),
      RenderOptions {
        monochrome_dib_palette_override: Some([[255, 128, 255], [255, 255, 255]]),
        ..RenderOptions::default()
      },
    )
    .unwrap()
    .unwrap();
    let image = image::load_from_memory_with_format(&decoded.data, image::ImageFormat::Png)
      .unwrap()
      .to_rgb8();
    assert!(
      image.pixels().any(|pixel| pixel.0 == [255, 128, 255]),
      "the opt-in realization palette replaces entry zero"
    );
    assert!(
      image.pixels().any(|pixel| pixel.0 == [255, 255, 255]),
      "the opt-in realization palette preserves entry one"
    );
    assert!(
      !image.pixels().any(|pixel| pixel.0 == [0, 0, 0]),
      "the embedded black entry is replaced only for this caller"
    );

    let decoded = decode_metafile_as_raster_with_options(
      &bytes,
      Some("image/x-wmf"),
      RenderOptions {
        monochrome_dib_palette_override: Some([[255, 128, 255], [255, 255, 255]]),
        filter_high_frequency_pattern_brushes: true,
        ..RenderOptions::default()
      },
    )
    .unwrap()
    .unwrap();
    let image = image::load_from_memory_with_format(&decoded.data, image::ImageFormat::Png)
      .unwrap()
      .to_rgb8();
    assert!(
      image.pixels().all(|pixel| pixel.0 == [255, 191, 255]),
      "fixed output box-filters a one-pixel checkerboard before later rescaling"
    );
  }

  #[test]
  fn world_unit_pen_width_uses_the_active_device_transform() {
    let mut data = vec![0; EMF_HEADER_SIZE];
    data[16..20].copy_from_slice(&999i32.to_le_bytes());
    data[20..24].copy_from_slice(&999i32.to_le_bytes());
    let mut state = EmfVectorState::new_with_options(
      &data,
      RenderOptions {
        target_width_px: Some(100),
        target_height_px: Some(100),
        max_pixels: None,
        transparent_background: false,
        background_color: None,
        monochrome_dib_palette_override: None,
        filter_high_frequency_pattern_brushes: false,
      },
    )
    .expect("minimal EMF bounds");
    state.world_transform.m11 = 0.5;
    state.world_transform.m22 = 0.5;

    let pen = state.resolve_pen(EmfPen {
      color: EmfColor { r: 0, g: 0, b: 0 },
      width: 100,
      transform_width: true,
    });

    assert_eq!(pen.width, 5);
    assert!(!pen.transform_width);
  }

  #[test]
  fn emf_null_pen_disables_polygon_outlines() {
    let pen = EmfPen {
      color: EmfColor {
        r: 255,
        g: 255,
        b: 255,
      },
      width: 1,
      transform_width: false,
    };
    assert!(emf_pen_from_style(EmrPenLineStyle::Solid.raw(), pen).is_some());
    assert!(emf_pen_from_style(EmrPenLineStyle::Null.raw(), pen).is_none());

    let mut data = vec![0; EMF_HEADER_SIZE];
    data[16..20].copy_from_slice(&9i32.to_le_bytes());
    data[20..24].copy_from_slice(&9i32.to_le_bytes());
    let mut state = EmfVectorState::new_with_options(&data, RenderOptions::default())
      .expect("minimal EMF bounds");
    state.pens.insert(7, None);
    state.select_object(7);
    assert!(state.current_pen.is_none());
  }

  #[test]
  fn emf_binary_raster_operations_follow_rop2_boolean_semantics() {
    let pen = EmfColor {
      r: 0b1010_1010,
      g: 0b1100_1100,
      b: 0b1111_0000,
    };
    let destination = EmfColor {
      r: 0b1111_0000,
      g: 0b1010_1010,
      b: 0b1100_1100,
    };

    assert_eq!(
      apply_binary_raster_operation(pen, destination, WmfBinaryRasterOperation::XorPen),
      EmfColor {
        r: destination.r ^ pen.r,
        g: destination.g ^ pen.g,
        b: destination.b ^ pen.b,
      }
    );
    assert_eq!(
      apply_binary_raster_operation(pen, destination, WmfBinaryRasterOperation::CopyPen),
      pen
    );
    assert_eq!(
      apply_binary_raster_operation(pen, destination, WmfBinaryRasterOperation::Nop),
      destination
    );
    assert_eq!(
      apply_binary_raster_operation(pen, destination, WmfBinaryRasterOperation::Black),
      EmfColor { r: 0, g: 0, b: 0 }
    );
    assert_eq!(
      apply_binary_raster_operation(pen, destination, WmfBinaryRasterOperation::White),
      EmfColor {
        r: u8::MAX,
        g: u8::MAX,
        b: u8::MAX,
      }
    );
  }

  fn stretch_record(bitmap_info: Vec<u8>, bitmap_bits: Vec<u8>) -> EmfRecord {
    EmfRecordData::StretchDiBits(EmrStretchDiBits {
      bounds: RectL::default(),
      dest: crate::PointL { x: 0, y: 0 },
      source: BitmapSourceBounds {
        x: 0,
        y: 0,
        width: 2,
        height: 2,
      },
      color_usage: DibColorUsage::RgbColors.raw(),
      raster_operation: 0x00CC_0020,
      dest_size: SizeL { cx: 2, cy: 2 },
      bitmap: EmrBitmapBuffer {
        undefined_space_before_bitmap_info: Vec::new(),
        bitmap_info,
        undefined_space_before_bitmap_bits: Vec::new(),
        bitmap_bits,
      },
      padding: Vec::new(),
    })
    .to_record()
    .unwrap()
  }

  fn stretch_blt_record(
    bitmap_info: Vec<u8>,
    bitmap_bits: Vec<u8>,
    raster_operation: u32,
  ) -> EmfRecord {
    EmfRecordData::StretchBlt(EmrStretchBlt {
      bounds: RectL::default(),
      dest: PointL { x: 0, y: 0 },
      dest_size: SizeL { cx: 2, cy: 2 },
      raster_operation,
      source: PointL { x: 0, y: 0 },
      xform_source: XForm {
        m11: 1.0,
        m22: 1.0,
        ..XForm::default()
      },
      background_color_source: ColorRef::default(),
      color_usage: DibColorUsage::RgbColors.raw(),
      source_size: SizeL { cx: 2, cy: 2 },
      bitmap: Some(EmrBitmapBuffer {
        undefined_space_before_bitmap_info: Vec::new(),
        bitmap_info,
        undefined_space_before_bitmap_bits: Vec::new(),
        bitmap_bits,
      }),
      padding: Vec::new(),
    })
    .to_record()
    .unwrap()
  }

  fn metafile_with(record: EmfRecord) -> Vec<u8> {
    metafile_with_records(vec![record])
  }

  fn metafile_with_records(records: Vec<EmfRecord>) -> Vec<u8> {
    let mut all_records = Vec::with_capacity(records.len() + 2);
    all_records.push(minimal_header_record());
    all_records.extend(records);
    all_records.push(eof_record());
    EmfMetafile {
      records: all_records,
      trailing_data: Vec::new(),
    }
    .to_bytes()
    .unwrap()
  }

  fn set_pixel_record(x: i32, y: i32, color_ref: u32) -> EmfRecord {
    let mut data = Vec::new();
    data.extend_from_slice(&x.to_le_bytes());
    data.extend_from_slice(&y.to_le_bytes());
    data.extend_from_slice(&color_ref.to_le_bytes());
    EmfRecord::new(super::EMR_SET_PIXEL_V, data)
  }

  #[test]
  fn polygon_scanlines_are_limited_to_the_visible_vertical_bounds() {
    let points = [(2.0, 10.0), (5.0, 10.0), (5.0, 12.0), (2.0, 12.0)];
    let mut spans = Vec::new();
    visit_polygon_scanline_spans(&points, 20, 10_000, |y, start, end| {
      spans.push((y, start, end));
    });

    assert_eq!(spans, [(10, 2, 5), (11, 2, 5)]);

    spans.clear();
    visit_polygon_scanline_spans(&points, 20, 8, |y, start, end| {
      spans.push((y, start, end));
    });
    assert!(spans.is_empty());
  }

  #[test]
  fn adjacent_slanted_polygon_bands_use_a_half_open_shared_edge() {
    let left = [(0.0, 0.0), (2.0, 0.0), (4.0, 2.0), (2.0, 2.0)];
    let right = [(2.0, 0.0), (4.0, 0.0), (6.0, 2.0), (4.0, 2.0)];
    let mut left_spans = Vec::new();
    let mut right_spans = Vec::new();
    visit_polygon_scanline_spans(&left, 8, 2, |y, start, end| {
      left_spans.push((y, start, end));
    });
    visit_polygon_scanline_spans(&right, 8, 2, |y, start, end| {
      right_spans.push((y, start, end));
    });

    assert_eq!(left_spans[0], (0, 0, 2));
    assert_eq!(right_spans[0], (0, 2, 4));
    assert_eq!(left_spans[0].2, right_spans[0].1);
  }

  #[test]
  fn axis_aligned_polygon_clip_uses_the_same_pixel_center_bounds() {
    let points = [(2.2, 10.8), (5.2, 10.8), (5.2, 13.2), (2.2, 13.2)];
    assert_eq!(
      axis_aligned_clip_rect(&points, 20, 20),
      Some((2, 11, 6, 13))
    );

    let rotated = [(2.0, 3.0), (4.0, 2.0), (5.0, 4.0), (3.0, 5.0)];
    assert_eq!(axis_aligned_clip_rect(&rotated, 20, 20), None);
  }

  #[test]
  fn rectangle_clip_intersection_preserves_empty_regions() {
    assert_eq!(intersect_rects((1, 2, 8, 9), (4, 0, 10, 6)), (4, 2, 8, 6));
    assert_eq!(intersect_rects((1, 1, 2, 2), (4, 4, 5, 5)), (4, 4, 4, 4));
  }

  #[test]
  fn decode_emf_embedded_png_bitmap() {
    let bits = vec![0x89, b'P', b'N', b'G'];
    let mut info = bitmap_info(2, 2, 0, BI_PNG);
    info[20..24].copy_from_slice(&(bits.len() as u32).to_le_bytes());
    let emf = metafile_with(stretch_record(info, bits));

    let decoded = decode_metafile_as_raster(&emf, Some("image/x-emf"))
      .unwrap()
      .unwrap();
    assert_eq!(decoded.content_type, "image/png");
    assert_eq!(decoded.data, [0x89, b'P', b'N', b'G']);
  }

  #[test]
  fn decode_emf_bi_rgb_bitmap_as_png() {
    let bits = vec![
      0, 0, 255, 0, 255, 0, 0, 0, // bottom row: red, green, padding
      255, 0, 0, 255, 255, 255, 0, 0, // top row: blue, white, padding
    ];
    let emf = metafile_with(stretch_record(bitmap_info(2, 2, 24, BI_RGB), bits));

    let decoded = decode_metafile_as_raster(&emf, Some("image/x-emf"))
      .unwrap()
      .unwrap();
    assert_eq!(decoded.content_type, "image/png");
    assert!(decoded.data.starts_with(&[0x89, b'P', b'N', b'G']));
  }

  #[test]
  fn decode_emf_replays_bitmap_with_later_vector_records() {
    let bits = vec![
      255, 255, 255, 255, 255, 255, 0, 0, // bottom row: white, white, padding
      255, 255, 255, 255, 255, 255, 0, 0, // top row: white, white, padding
    ];
    let emf = metafile_with_records(vec![
      stretch_record(bitmap_info(2, 2, 24, BI_RGB), bits),
      set_pixel_record(0, 0, 0x0000_00ff),
    ]);

    let decoded = decode_metafile_as_raster(&emf, Some("image/x-emf"))
      .unwrap()
      .unwrap();
    assert_eq!(decoded.content_type, "image/png");
    let image = image::load_from_memory(&decoded.data).unwrap().to_rgb8();
    assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0]);
  }

  #[test]
  fn decode_emf_replays_stretch_blt_mask_and_source_rops() {
    let mut mask_info = bitmap_info(2, 2, 1, BI_RGB);
    mask_info.extend_from_slice(&[
      0, 0, 0, 0, // black
      255, 255, 255, 0, // white
    ]);
    let mask_bits = vec![
      0, 0, 0, 0, // bottom row: black, black, padding
      0, 0, 0, 0, // top row: black, black, padding
    ];
    let source_bits = vec![
      0, 0, 255, 0, 0, 0, 255, 0, // bottom row: red, red
      0, 0, 255, 0, 0, 0, 255, 0, // top row: red, red
    ];
    let emf = metafile_with_records(vec![
      stretch_blt_record(mask_info, mask_bits, 0x0088_00C6),
      stretch_blt_record(bitmap_info(2, 2, 32, BI_RGB), source_bits, 0x0066_0046),
    ]);

    let decoded = decode_metafile_as_raster(&emf, Some("image/x-emf"))
      .unwrap()
      .unwrap();
    assert_eq!(decoded.content_type, "image/png");
    let image = image::load_from_memory(&decoded.data).unwrap().to_rgb8();
    assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0]);
  }

  #[test]
  fn decode_emf_blt_composes_against_the_caller_background() {
    let mut mask_info = bitmap_info(2, 2, 1, BI_RGB);
    mask_info.extend_from_slice(&[
      0, 0, 0, 0, // black
      255, 255, 255, 0, // white
    ]);
    let mask_bits = vec![
      0x40, 0, 0, 0, // bottom row: black, white, padding
      0x40, 0, 0, 0, // top row: black, white, padding
    ];
    let source_bits = vec![
      255, 0, 0, 0, 0, 0, 0, 0, // bottom row: blue, black
      255, 0, 0, 0, 0, 0, 0, 0, // top row: blue, black
    ];
    let emf = metafile_with_records(vec![
      stretch_blt_record(mask_info, mask_bits, 0x0088_00C6),
      stretch_blt_record(bitmap_info(2, 2, 32, BI_RGB), source_bits, 0x0066_0046),
    ]);

    let decoded = decode_metafile_as_raster_with_options(
      &emf,
      Some("image/x-emf"),
      RenderOptions {
        background_color: Some([255, 0, 0]),
        ..RenderOptions::default()
      },
    )
    .unwrap()
    .unwrap();
    let image = image::load_from_memory(&decoded.data).unwrap().to_rgb8();
    assert_eq!(image.get_pixel(0, 0).0, [0, 0, 255]);
    assert_eq!(image.get_pixel(1, 0).0, [255, 0, 0]);
  }

  #[test]
  fn decode_emf_blt_reconstructs_a_transparent_destination() {
    let mut mask_info = bitmap_info(2, 2, 1, BI_RGB);
    mask_info.extend_from_slice(&[
      0, 0, 0, 0, // black
      255, 255, 255, 0, // white
    ]);
    let mask_bits = vec![
      0x40, 0, 0, 0, // bottom row: black, white, padding
      0x40, 0, 0, 0, // top row: black, white, padding
    ];
    let source_bits = vec![
      255, 0, 0, 0, 0, 0, 0, 0, // bottom row: blue, black
      255, 0, 0, 0, 0, 0, 0, 0, // top row: blue, black
    ];
    let emf = metafile_with_records(vec![
      stretch_blt_record(mask_info, mask_bits, 0x0088_00C6),
      stretch_blt_record(bitmap_info(2, 2, 32, BI_RGB), source_bits, 0x0066_0046),
    ]);

    let decoded = decode_metafile_as_raster_with_options(
      &emf,
      Some("image/x-emf"),
      RenderOptions {
        transparent_background: true,
        ..RenderOptions::default()
      },
    )
    .unwrap()
    .unwrap();
    let image = image::load_from_memory(&decoded.data).unwrap().to_rgba8();
    assert_eq!(image.get_pixel(0, 0).0, [0, 0, 255, 255]);
    assert_eq!(image.get_pixel(1, 0).0, [0, 0, 0, 0]);
  }

  #[test]
  fn gdi_plus_metafile_bilinear_sampling_uses_endpoint_over_target_extent() {
    let image = RasterPixels {
      width: 32,
      height: 1,
      rgb: (0..32)
        .flat_map(|index| {
          let value = index * 8;
          [value, value, value]
        })
        .collect(),
    };
    let samples = (0..66)
      .map(|x| gdi_plus_bilinear_raster_color(&image, x, 0, 66, 1).r)
      .collect::<Vec<_>>();

    assert_eq!(&samples[..5], &[0, 4, 8, 11, 15]);
    assert_eq!(samples[65], 244);
  }

  #[test]
  fn resized_masked_blt_keeps_filtered_source_color_fringe() {
    let mut mask_info = bitmap_info(2, 2, 1, BI_RGB);
    mask_info.extend_from_slice(&[
      0, 0, 0, 0, // black
      255, 255, 255, 0, // white
    ]);
    let mask_bits = vec![
      0x40, 0, 0, 0, // bottom row: black, white, padding
      0x40, 0, 0, 0, // top row: black, white, padding
    ];
    let source_bits = vec![
      255, 0, 0, 0, 0, 0, 0, 0, // bottom row: blue, black
      255, 0, 0, 0, 0, 0, 0, 0, // top row: blue, black
    ];
    let emf = metafile_with_records(vec![
      stretch_blt_record(mask_info, mask_bits, 0x0088_00C6),
      stretch_blt_record(bitmap_info(2, 2, 32, BI_RGB), source_bits, 0x0066_0046),
    ]);

    let decoded = decode_metafile_as_raster_with_options(
      &emf,
      Some("image/x-emf"),
      RenderOptions {
        target_width_px: Some(4),
        target_height_px: Some(2),
        transparent_background: true,
        ..RenderOptions::default()
      },
    )
    .unwrap()
    .unwrap();
    let image = image::load_from_memory(&decoded.data).unwrap().to_rgba8();

    assert_eq!(image.get_pixel(0, 0).0, [0, 0, 255, 255]);
    assert_eq!(image.get_pixel(1, 0).0, [0, 0, 191, 255]);
    assert_eq!(image.get_pixel(2, 0).0, [0, 0, 128, 255]);
    assert_eq!(image.get_pixel(3, 0).0, [0, 0, 64, 255]);
  }

  #[test]
  fn cleartype_box_decimation_displaces_rgb_windows_over_six_samples() {
    let (left, width, coverage) = cleartype_box_decimate(&[255; 6], 6, 1, 0);

    assert_eq!(left, -1);
    assert_eq!(width, 3);
    assert_eq!(
      coverage,
      [[0, 0, 85], [170, 255, 170], [85, 0, 0]],
      "the one-pixel box is centered independently on the R, G, and B stripes"
    );
  }

  #[test]
  fn crop_raster_pixels_matches_the_valid_source_rectangle() {
    let image = RasterPixels {
      width: 3,
      height: 2,
      rgb: vec![
        255, 0, 0, 0, 255, 0, 0, 0, 255, // first row
        255, 255, 0, 0, 255, 255, 255, 0, 255, // second row
      ],
    };
    let cropped = crop_raster_pixels(&image, (1, 0, 2, 2)).unwrap();
    assert_eq!(cropped.width, 2);
    assert_eq!(cropped.height, 2);
    assert_eq!(
      cropped.rgb,
      [0, 255, 0, 0, 0, 255, 0, 255, 255, 255, 0, 255,]
    );
    assert!(crop_raster_pixels(&image, (-1, 0, 2, 2)).is_none());
    assert!(crop_raster_pixels(&image, (2, 0, 2, 2)).is_none());
  }

  #[test]
  fn bilinear_stretch_samples_pixel_centers_without_blurring_the_outer_edge() {
    let image = RasterPixels {
      width: 2,
      height: 1,
      rgb: vec![0, 0, 0, 255, 255, 255],
    };

    assert_eq!(bilinear_raster_color(&image, 0, 0, 4, 1).r, 0);
    assert_eq!(bilinear_raster_color(&image, 1, 0, 4, 1).r, 64);
    assert_eq!(bilinear_raster_color(&image, 2, 0, 4, 1).r, 191);
    assert_eq!(bilinear_raster_color(&image, 3, 0, 4, 1).r, 255);
  }

  #[test]
  fn nearest_stretch_samples_destination_pixel_centers() {
    assert_eq!(
      (0..5)
        .map(|destination| nearest_raster_index(destination, 5, 2))
        .collect::<Vec<_>>(),
      [0, 0, 1, 1, 1]
    );
  }

  #[test]
  fn two_color_palette_rasters_stay_discrete_during_stretch() {
    let two_color = RasterPixels {
      width: 2,
      height: 1,
      rgb: vec![255, 255, 255, 0, 236, 236],
    };
    assert!(is_discrete_two_color_raster(&two_color));
    let mut three_color = two_color;
    three_color.width = 3;
    three_color.rgb.extend_from_slice(&[1, 2, 3]);
    assert!(!is_discrete_two_color_raster(&three_color));
  }

  #[test]
  fn non_metafile_returns_none() {
    assert!(
      decode_metafile_as_raster(b"not a metafile", None)
        .unwrap()
        .is_none()
    );
  }
}
