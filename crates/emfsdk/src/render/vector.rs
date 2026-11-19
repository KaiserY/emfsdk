//! Strict vector lifting for classic solid-fill metafile streams.
//!
//! This interpreter is intentionally all-or-nothing. It returns a scene only
//! when every record that can paint is either representable as a solid closed
//! path or was explicitly suppressed after an independent layer extractor
//! lifted it. Callers retain the complete raster replay for every other
//! metafile. That keeps vector output faithful without treating a partially
//! understood record stream as complete.

use std::collections::HashMap;

use crate::common::SdkEnumValue;
use crate::emf::{
  EmfMetafileRef, EmfRecordData, EmrComment, EmrMapMode, EmrModifyWorldTransformMode,
  EmrPenLineStyle, EmrPenType, EmrPolygonFillMode, EmrPublicComment, EmrStockObject,
};
use crate::emfplus::EmfPlusRecordData;
use crate::types::{ColorRef, PointL, PointS, XForm};
use crate::wmf::{
  WmfBinaryRasterOperation, WmfBrushStyle, WmfMapMode, WmfMetafileRef, WmfPenLineStyle,
  WmfPolyFillMode, WmfRecordData, WmfTernaryRasterOperationCode,
};

use super::{
  EmfPoint, EmfTransform, RenderError, RenderOptions, RenderResult, emf_gdiplus_playback_geometry,
  is_emf, looks_like_metafile, nonzero_mapping_extent, scale_wmf_extent, wmf_initial_window,
};

mod word_pen;

/// The polygon fill rule active when a metafile path was recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetafileVectorFillRule {
  /// Odd crossings fill a point (`ALTERNATE`).
  Alternate,
  /// Any nonzero winding count fills a point (`WINDING`).
  Winding,
}

/// A point normalized to the metafile playback surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MetafileVectorPoint {
  pub x: f32,
  pub y: f32,
}

/// One GDI fill operation.
///
/// All subpaths remain in one operation so `PolyPolygon` holes keep the
/// record's `ALTERNATE` or `WINDING` semantics.
#[derive(Clone, Debug, PartialEq)]
pub struct MetafileVectorFill {
  pub subpaths: Vec<Vec<MetafileVectorPoint>>,
  pub color: [u8; 3],
  pub fill_rule: MetafileVectorFillRule,
}

/// A completely representable classic EMF/WMF solid-fill scene.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MetafileVectorScene {
  pub fills: Vec<MetafileVectorFill>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetafileVectorLineCap {
  Flat,
  Round,
  Square,
}

/// An ordered classic GDI operation, including its device-space clipping.
#[derive(Clone, Debug, PartialEq)]
pub enum MetafileVectorDraw {
  Fill {
    fill: MetafileVectorFill,
    clip: Option<[f32; 4]>,
  },
  Stroke {
    subpaths: Vec<Vec<MetafileVectorPoint>>,
    color: [u8; 3],
    /// One cosmetic device pixel along the two normalized playback axes.
    width: [f32; 2],
    line_cap: MetafileVectorLineCap,
    closed: bool,
    clip: Option<[f32; 4]>,
  },
}

/// A complete classic solid-brush/cosmetic-pen fixed-output drawing.
///
/// Kept separate from the established fill-only scene so existing callers
/// retain their completeness and raster-fallback contract.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MetafileVectorDrawing {
  pub operations: Vec<MetafileVectorDraw>,
}

/// Consecutive vector operations or a source bitmap in WMF playback order.
#[derive(Clone, Debug, PartialEq)]
pub enum MetafilePictureLayer {
  Drawing(MetafileVectorDrawing),
  Bitmap(super::MetafileBitmapPicture),
}

/// Preserve the order of paths and SRCCOPY bitmaps in a Word fixed-output
/// picture. Text can be separated only when it follows all visible graphics.
pub fn extract_word_wmf_picture_layers(
  data: &[u8],
  options: RenderOptions,
) -> RenderResult<Option<Vec<MetafilePictureLayer>>> {
  use crate::wmf::WmfRecordFunction as F;
  if !options.word_emf_vector_pens
    || !crate::wmf::looks_like_wmf(data)
    || options.monochrome_dib_palette_override.is_some()
  {
    return Ok(None);
  }
  let source =
    WmfMetafileRef::from_bytes(data).map_err(|err| RenderError::from(err.to_string()))?;
  let mut bitmap = false;
  let mut other_paint = false;
  for record in source.records() {
    match record.normalized_function_kind() {
      Some(F::StretchDib) => bitmap = true,
      Some(F::Polygon | F::PolyPolygon | F::Polyline | F::LineTo | F::TextOut | F::ExtTextOut) => {
        other_paint = true
      }
      _ => {}
    }
    if bitmap && other_paint {
      break;
    }
  }
  if !bitmap
    || !other_paint
    || crate::wmf::embedded_enhanced_metafile(data)
      .map_err(|err| RenderError::from(err.to_string()))?
      .is_some()
  {
    return Ok(None);
  }
  let mut layers = Vec::new();
  let Some(mut interpreter) =
    extract_wmf_interpreter(data, options, WmfVectorMode::Picture, Some(&mut layers))
      .map_err(RenderError::from)?
  else {
    return Ok(None);
  };
  flush_wmf_drawing(&mut interpreter, &mut layers);
  Ok(Some(layers))
}

fn flush_wmf_drawing(interpreter: &mut VectorInterpreter, layers: &mut Vec<MetafilePictureLayer>) {
  if let Some(operations) = &mut interpreter.drawing
    && !operations.is_empty()
  {
    layers.push(MetafilePictureLayer::Drawing(MetafileVectorDrawing {
      operations: std::mem::take(operations),
    }));
  }
}

/// A complete vector prefix followed by unchanged WMF text playback.
///
/// The residual metafile retains fonts, colours, DC state and original text
/// records. It contains no graphics already lifted into `drawing`.
#[derive(Clone, Debug, PartialEq)]
pub struct MetafileVectorPrefix {
  pub drawing: MetafileVectorDrawing,
  pub residual_metafile: Vec<u8>,
}

/// Lift a complete classic EMF drawing after independent layers were removed.
///
/// This is the fixed-output path for solid fills, one-device-pixel pens and
/// rectangular DC clips. Under the Word fixed-output profile, classic WMF
/// solid pens retain their logical widths and flat caps. Unsupported paint,
/// pen styles, destination operations and non-rectangular clips still reject
/// the entire scene. EMF+ retains its established path.
pub fn extract_metafile_vector_drawing_with_options(
  data: &[u8],
  content_type: Option<&str>,
  options: RenderOptions,
) -> RenderResult<Option<MetafileVectorDrawing>> {
  if !looks_like_metafile(data, content_type) {
    return Ok(None);
  }
  let embedded;
  let data = if is_emf(data) {
    data
  } else {
    embedded =
      crate::wmf::embedded_enhanced_metafile(data).map_err(|e| RenderError::from(e.to_string()))?;
    let Some(embedded) = &embedded else {
      return if options.word_emf_vector_pens {
        let mode = if options.suppress_text && word_wmf_text_can_be_lifted(data, options) {
          WmfVectorMode::BeforeText
        } else {
          WmfVectorMode::Drawing
        };
        extract_wmf_interpreter(data, options, mode, None)
          .map(|interpreter| {
            interpreter.map(|interpreter| MetafileVectorDrawing {
              operations: interpreter.drawing.unwrap_or_default(),
            })
          })
          .map_err(RenderError::from)
      } else {
        Ok(None)
      };
    };
    &embedded.data
  };
  extract_emf_interpreter(data, options, true)
    .map(|interpreter| {
      interpreter.map(|interpreter| MetafileVectorDrawing {
        operations: interpreter.drawing.unwrap_or_default(),
      })
    })
    .map_err(RenderError::from)
}

/// Split a complete Word WMF graphics prefix from its following text layer.
///
/// Text stays in the existing renderer, including opaque backgrounds, clipping,
/// LOGFONT widths and GDI font realization. Graphics after the first text
/// record, unsupported drawing, embedded EMF and destination operations retain
/// whole-metafile playback. This is a composition proof, not permission to
/// reorder interleaved text or to lift partially supported graphics.
pub fn extract_metafile_vector_prefix_with_options(
  data: &[u8],
  content_type: Option<&str>,
  options: RenderOptions,
) -> RenderResult<Option<MetafileVectorPrefix>> {
  use crate::wmf::WmfRecordFunction as F;
  if !options.word_emf_vector_pens || !looks_like_metafile(data, content_type) || is_emf(data) {
    return Ok(None);
  }
  if crate::wmf::embedded_enhanced_metafile(data)
    .map_err(|err| RenderError::from(err.to_string()))?
    .is_some()
  {
    return Ok(None);
  }
  let source =
    WmfMetafileRef::from_bytes(data).map_err(|err| RenderError::from(err.to_string()))?;
  let mut text_seen = false;
  let mut graphics_seen = false;
  for record in source.records() {
    match record.normalized_function_kind() {
      Some(F::TextOut | F::ExtTextOut) => text_seen = true,
      Some(F::Polygon | F::PolyPolygon | F::Polyline | F::LineTo) => {
        if text_seen {
          return Ok(None);
        }
        graphics_seen = true;
      }
      _ => {}
    }
  }
  if !text_seen || !graphics_seen {
    return Ok(None);
  }
  let Some(interpreter) = extract_wmf_interpreter(data, options, WmfVectorMode::BeforeText, None)
    .map_err(RenderError::from)?
  else {
    return Ok(None);
  };
  let operations = interpreter.drawing.unwrap_or_default();
  if operations.is_empty() {
    return Ok(None);
  }
  let mut residual = crate::wmf::WmfMetafile {
    placeable_header: source.placeable_header,
    header: source.header,
    records: source
      .records()
      .filter_map(|record| match record.normalized_function_kind() {
        Some(F::Polygon | F::PolyPolygon | F::Polyline) => None,
        Some(F::LineTo) => {
          // LineTo updates the DC current position; deleting it would move
          // a later TA_UPDATECP text run. MoveTo retains that state change.
          let mut record = record.into_owned();
          record.function = F::MoveTo.raw();
          Some(record)
        }
        _ => Some(record.into_owned()),
      })
      .collect(),
    trailing_data: source.trailing_data().to_vec(),
  };
  residual.header.file_size_words = residual
    .computed_file_size_words()
    .map_err(|err| RenderError::from(err.to_string()))?;
  residual.header.max_record_words = residual
    .computed_max_record_words()
    .map_err(|err| RenderError::from(err.to_string()))?;
  Ok(Some(MetafileVectorPrefix {
    drawing: MetafileVectorDrawing { operations },
    residual_metafile: residual
      .to_bytes()
      .map_err(|err| RenderError::from(err.to_string()))?,
  }))
}

/// Extracts a vector scene using the metafile's self-contained playback
/// geometry.
pub fn extract_metafile_vector_scene(
  data: &[u8],
  content_type: Option<&str>,
) -> RenderResult<Option<MetafileVectorScene>> {
  extract_metafile_vector_scene_with_options(data, content_type, RenderOptions::default())
}

/// Extracts a vector scene when the entire remaining visible stream is a
/// supported classic solid-fill subset.
///
/// `Ok(None)` is a normal completeness fallback, not a malformed-file error.
/// Suppression options are accepted only for records whose visible output is
/// independently lifted; a cosmetic line may disappear with its following
/// `PATCOPY` rectangle only when their one-device-pixel geometry and color
/// prove that the later opaque rectangle completely covers the line.
/// For non-placeable WMF streams, `options.wmf_external_header` supplies the
/// same `METAFILEPICT` playback rectangle used by raster replay.
pub fn extract_metafile_vector_scene_with_options(
  data: &[u8],
  content_type: Option<&str>,
  options: RenderOptions,
) -> RenderResult<Option<MetafileVectorScene>> {
  if !looks_like_metafile(data, content_type) {
    return Ok(None);
  }

  let result = if is_emf(data) {
    extract_emf_scene(data, options)
  } else if let Ok(Some(embedded)) = crate::wmf::embedded_enhanced_metafile(data) {
    extract_emf_scene(&embedded.data, options)
  } else {
    extract_wmf_scene(data, options)
  };
  result.map_err(RenderError::from)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VectorBrush {
  Solid([u8; 3]),
  Null,
  Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VectorPen {
  Null,
  SolidCosmetic([u8; 3]),
  WmfLogical { color: [u8; 3], width: u16 },
  Word(word_pen::Pen),
  Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VectorObject {
  Brush(VectorBrush),
  Pen(VectorPen),
  Other,
}

#[derive(Clone, Copy, Debug)]
struct VectorMapping {
  surface_width: f32,
  surface_height: f32,
  playback_origin_x: f32,
  playback_origin_y: f32,
  playback_scale_x: f32,
  playback_scale_y: f32,
  window_org_x: i32,
  window_org_y: i32,
  window_ext_x: i32,
  window_ext_y: i32,
  viewport_org_x: i32,
  viewport_org_y: i32,
  viewport_ext_x: i32,
  viewport_ext_y: i32,
  world_transform: EmfTransform,
  map_mode: EmrMapMode,
}

impl VectorMapping {
  fn emf(data: &[u8]) -> Result<Self, String> {
    let geometry = emf_gdiplus_playback_geometry(data)?;
    Ok(Self {
      surface_width: geometry.width.max(1) as f32,
      surface_height: geometry.height.max(1) as f32,
      playback_origin_x: geometry.origin_x,
      playback_origin_y: geometry.origin_y,
      playback_scale_x: geometry.scale_x,
      playback_scale_y: geometry.scale_y,
      window_org_x: 0,
      window_org_y: 0,
      window_ext_x: geometry.width as i32,
      window_ext_y: geometry.height as i32,
      viewport_org_x: 0,
      viewport_org_y: 0,
      viewport_ext_x: geometry.width as i32,
      viewport_ext_y: geometry.height as i32,
      map_mode: EmrMapMode::Text,
      world_transform: EmfTransform::identity(),
    })
  }

  fn wmf(metafile: &WmfMetafileRef<'_>, options: RenderOptions) -> Self {
    let (window_org_x, window_org_y, window_ext_x, window_ext_y) =
      wmf_initial_window(metafile, options.wmf_external_header);
    let surface_width = window_ext_x.unsigned_abs().max(1) as f32;
    let surface_height = window_ext_y.unsigned_abs().max(1) as f32;
    Self {
      surface_width,
      surface_height,
      playback_origin_x: 0.0,
      playback_origin_y: 0.0,
      playback_scale_x: 1.0,
      playback_scale_y: 1.0,
      window_org_x,
      window_org_y,
      window_ext_x: nonzero_mapping_extent(window_ext_x),
      window_ext_y: nonzero_mapping_extent(window_ext_y),
      viewport_org_x: 0,
      viewport_org_y: 0,
      viewport_ext_x: surface_width as i32,
      viewport_ext_y: surface_height as i32,
      map_mode: EmrMapMode::Anisotropic,
      world_transform: EmfTransform::identity(),
    }
    .with_wmf_conversion(metafile, options)
  }

  fn with_wmf_conversion(mut self, metafile: &WmfMetafileRef<'_>, options: RenderOptions) -> Self {
    let (Some(header), Some(profile)) = (
      metafile.placeable_header.as_ref(),
      options.wmf_conversion_profile,
    ) else {
      return self;
    };
    if header.inch == 0
      || header.right <= header.left
      || header.bottom <= header.top
      || profile.device_pixels.contains(&0)
      || profile.device_millimeters.contains(&0)
    {
      return self;
    }

    // Word lifts placeable WMF paths through GdipConvertToEmfPlus. The
    // reference DC's physical resolution realizes both the original origin
    // and extent; its logical DPI is not the Device/Millimeters ratio.
    let origin = [f64::from(header.left), f64::from(header.top)];
    let extent = [
      f64::from(header.bounding_box_width()),
      f64::from(header.bounding_box_height()),
    ];
    let mm100_per_pixel: [f64; 2] = std::array::from_fn(|axis| {
      f64::from(profile.device_millimeters[axis]) * 100.0 / f64::from(profile.device_pixels[axis])
    });
    let pixel_scale: [f64; 2] =
      std::array::from_fn(|axis| 2540.0 / (f64::from(header.inch) * mm100_per_pixel[axis]));
    let mut device_origin: [f64; 2] = std::array::from_fn(|axis| origin[axis] * pixel_scale[axis]);
    let mut device_extent: [f64; 2] = std::array::from_fn(|axis| extent[axis] * pixel_scale[axis]);
    // Native conversion keeps logical coordinates as device coordinates
    // when either realized dimension is below 20 pixels. This is a joint
    // boundary, not an independent scale clamp on each axis.
    if device_extent.iter().any(|value| *value < 20.0) {
      device_origin = origin;
      device_extent = extent;
    }
    let frame_origin: [f64; 2] = std::array::from_fn(|axis| {
      (device_origin[axis] * mm100_per_pixel[axis]).round_ties_even() / mm100_per_pixel[axis]
    });
    let surface: [f64; 2] = std::array::from_fn(|axis| {
      // The recorded frame addresses the last pixel. PDF vector playback
      // restores that inclusive endpoint but preserves the fractional frame
      // origin; rounding it to a device integer loses translated path phase.
      ((device_origin[axis] + device_extent[axis] - 1.0) * mm100_per_pixel[axis]).round_ties_even()
        / mm100_per_pixel[axis]
        - frame_origin[axis]
        + 1.0
    });
    self.surface_width = surface[0].max(1.0) as f32;
    self.surface_height = surface[1].max(1.0) as f32;
    self.playback_origin_x = frame_origin[0] as f32;
    self.playback_origin_y = frame_origin[1] as f32;
    self.viewport_org_x = device_origin[0].round_ties_even() as i32;
    self.viewport_org_y = device_origin[1].round_ties_even() as i32;
    self.viewport_ext_x = device_extent[0].round_ties_even().max(1.0) as i32;
    self.viewport_ext_y = device_extent[1].round_ties_even().max(1.0) as i32;
    self
  }

  fn map_point(self, point: EmfPoint) -> Option<MetafileVectorPoint> {
    let (x, y) = self.world_transform.apply(point);
    let scale_x = self.viewport_ext_x as f32 / nonzero_mapping_extent(self.window_ext_x) as f32;
    let scale_y = self.viewport_ext_y as f32 / nonzero_mapping_extent(self.window_ext_y) as f32;
    let x = ((self.viewport_org_x as f32 + (x - self.window_org_x as f32) * scale_x
      - self.playback_origin_x)
      * self.playback_scale_x)
      / self.surface_width;
    let y = ((self.viewport_org_y as f32 + (y - self.window_org_y as f32) * scale_y
      - self.playback_origin_y)
      * self.playback_scale_y)
      / self.surface_height;
    (x.is_finite() && y.is_finite()).then_some(MetafileVectorPoint { x, y })
  }

  fn map_device_point(self, point: EmfPoint, centered: bool) -> Option<MetafileVectorPoint> {
    let point = self.map_point(point)?;
    // Classic polygon/line playback addresses device pixel centers. Its
    // outer endpoint is separate from the inclusive GDI+ header surface.
    // Rectangle records retain their explicit inclusive box below.
    let offset = if centered { 0.5 } else { 0.0 };
    let x = (point.x * self.surface_width + offset) / (self.surface_width - 1.0);
    let y = (point.y * self.surface_height + offset) / (self.surface_height - 1.0);
    (x.is_finite() && y.is_finite()).then_some(MetafileVectorPoint { x, y })
  }

  fn map_clip_rect(self, rect: crate::types::RectL) -> Option<[f32; 4]> {
    let points = [
      EmfPoint {
        x: rect.left,
        y: rect.top,
      },
      EmfPoint {
        x: rect.right,
        y: rect.top,
      },
      EmfPoint {
        x: rect.right,
        y: rect.bottom,
      },
      EmfPoint {
        x: rect.left,
        y: rect.bottom,
      },
    ];
    let mapped = points.map(|point| self.map_point(point));
    let [Some(a), Some(b), Some(c), Some(d)] = mapped else {
      return None;
    };
    if !((a.y == b.y && b.x == c.x && c.y == d.y && d.x == a.x)
      || (a.x == b.x && b.y == c.y && c.x == d.x && d.y == a.y))
    {
      return None;
    }
    Some([a.x.min(c.x), a.y.min(c.y), a.x.max(c.x), a.y.max(c.y)])
  }

  fn reset_for_emf_device_context(mut self) -> Self {
    let width = self.surface_width.max(1.0).round() as i32;
    let height = self.surface_height.max(1.0).round() as i32;
    self.window_org_x = 0;
    self.window_org_y = 0;
    self.window_ext_x = width;
    self.window_ext_y = height;
    self.viewport_org_x = 0;
    self.viewport_org_y = 0;
    self.viewport_ext_x = width;
    self.viewport_ext_y = height;
    self.map_mode = EmrMapMode::Text;
    self.world_transform = EmfTransform::identity();
    self
  }

  fn is_identity_device_mapping(self) -> bool {
    let width = self.surface_width.max(1.0).round() as i32;
    let height = self.surface_height.max(1.0).round() as i32;
    self.window_org_x == 0
      && self.window_org_y == 0
      && self.window_ext_x == width
      && self.window_ext_y == height
      && self.viewport_org_x == 0
      && self.viewport_org_y == 0
      && self.viewport_ext_x == width
      && self.viewport_ext_y == height
      && self.world_transform.m11 == 1.0
      && self.world_transform.m12 == 0.0
      && self.world_transform.m21 == 0.0
      && self.world_transform.m22 == 1.0
      && self.world_transform.dx == 0.0
      && self.world_transform.dy == 0.0
  }
}

#[derive(Clone, Copy, Debug)]
struct VectorGraphicsState {
  mapping: VectorMapping,
  brush: VectorBrush,
  pen: VectorPen,
  binary_raster_operation: Option<WmfBinaryRasterOperation>,
  fill_rule: MetafileVectorFillRule,
  current_pos: EmfPoint,
  clip: Option<[f32; 4]>,
}

struct VectorInterpreter {
  graphics: VectorGraphicsState,
  saved: Vec<VectorGraphicsState>,
  fills: Vec<MetafileVectorFill>,
  drawing: Option<Vec<MetafileVectorDraw>>,
  word_pens: Option<word_pen::State>,
  wmf_drawing: bool,
}

impl VectorInterpreter {
  fn new(mapping: VectorMapping) -> Self {
    Self {
      graphics: VectorGraphicsState {
        mapping,
        // Win32 initializes a memory DC with WHITE_BRUSH and BLACK_PEN.
        brush: VectorBrush::Solid([255; 3]),
        pen: VectorPen::SolidCosmetic([0; 3]),
        binary_raster_operation: Some(WmfBinaryRasterOperation::CopyPen),
        fill_rule: MetafileVectorFillRule::Alternate,
        current_pos: EmfPoint { x: 0, y: 0 },
        clip: None,
      },
      saved: Vec::new(),
      fills: Vec::new(),
      drawing: None,
      word_pens: None,
      wmf_drawing: false,
    }
  }

  fn save(&mut self) {
    self.saved.push(self.graphics);
  }

  fn restore_latest(&mut self) -> bool {
    let Some(graphics) = self.saved.pop() else {
      return false;
    };
    self.graphics = graphics;
    true
  }

  fn emit_polygon(&mut self, points: &[EmfPoint]) -> bool {
    let polygon = points.to_vec();
    self.emit_polygons(std::slice::from_ref(&polygon))
  }

  fn emit_polygons(&mut self, polygons: &[Vec<EmfPoint>]) -> bool {
    // Polygon and PolyPolygon both stroke their contours after filling. A
    // visible pen therefore makes a fill-only PDF scene incomplete.
    if self.drawing.is_none() && self.graphics.pen != VectorPen::Null {
      return false;
    }
    if self.graphics.brush == VectorBrush::Unsupported {
      return false;
    }
    if self.drawing.is_none() && self.graphics.brush == VectorBrush::Null {
      return true;
    }
    let mut subpaths = Vec::with_capacity(polygons.len());
    for polygon in polygons {
      if polygon.len() < 2 {
        continue;
      }
      let mut mapped = Vec::with_capacity(polygon.len());
      for point in polygon {
        let point = if self.drawing.is_some() {
          self.map_drawing_point(*point)
        } else {
          self.graphics.mapping.map_point(*point)
        };
        let Some(point) = point else {
          return false;
        };
        mapped.push(point);
      }
      subpaths.push(mapped);
    }
    self.emit_mapped_polygons(subpaths)
  }

  fn map_drawing_point(&self, point: EmfPoint) -> Option<MetafileVectorPoint> {
    if self.wmf_drawing {
      if matches!(self.graphics.pen, VectorPen::WmfLogical { width: 1, .. }) {
        self.graphics.mapping.map_device_point(point, true)
      } else {
        self.graphics.mapping.map_point(point)
      }
    } else {
      self
        .graphics
        .mapping
        .map_device_point(point, self.graphics.pen != VectorPen::Null)
    }
  }

  fn emit_wmf_line(&mut self, start: EmfPoint, end: EmfPoint) -> bool {
    let narrow = matches!(self.graphics.pen, VectorPen::WmfLogical { width: 1, .. });
    let end = if narrow {
      let Some(end) = word_pen::line_endpoint(start, end) else {
        return true;
      };
      end
    } else {
      end
    };
    let supported = self.emit_polyline(&[start, end]);
    if supported
      && narrow
      && let Some(MetafileVectorDraw::Stroke { line_cap, .. }) =
        self.drawing.as_mut().and_then(|ops| ops.last_mut())
    {
      *line_cap = MetafileVectorLineCap::Square;
    }
    supported
  }

  fn emit_mapped_polygons(&mut self, subpaths: Vec<Vec<MetafileVectorPoint>>) -> bool {
    if subpaths.is_empty() {
      return true;
    }
    if self.drawing.is_none() {
      if let VectorBrush::Solid(color) = self.graphics.brush {
        self.fills.push(MetafileVectorFill {
          subpaths,
          color,
          fill_rule: self.graphics.fill_rule,
        });
      }
      return true;
    }
    if let VectorBrush::Solid(color) = self.graphics.brush {
      let fill = MetafileVectorFill {
        subpaths: subpaths.clone(),
        color,
        fill_rule: self.graphics.fill_rule,
      };
      if let Some(drawing) = &mut self.drawing {
        drawing.push(MetafileVectorDraw::Fill {
          fill,
          clip: self.graphics.clip,
        });
      }
    }
    self.emit_mapped_stroke(subpaths, true)
  }

  fn emit_mapped_stroke(&mut self, subpaths: Vec<Vec<MetafileVectorPoint>>, closed: bool) -> bool {
    let (color, width) = match self.graphics.pen {
      VectorPen::Null => return true,
      VectorPen::SolidCosmetic(color) => (
        color,
        [
          1.0 / self.graphics.mapping.surface_width,
          1.0 / self.graphics.mapping.surface_height,
        ],
      ),
      VectorPen::WmfLogical { color, width } => {
        let mapping = self.graphics.mapping;
        let width = f32::from(width);
        (
          color,
          [
            (width * mapping.viewport_ext_x as f32 / mapping.window_ext_x as f32
              * mapping.playback_scale_x
              / mapping.surface_width)
              .abs(),
            (width * mapping.viewport_ext_y as f32 / mapping.window_ext_y as f32
              * mapping.playback_scale_y
              / mapping.surface_height)
              .abs(),
          ],
        )
      }
      VectorPen::Word(_) | VectorPen::Unsupported => return false,
    };
    if self.graphics.binary_raster_operation != Some(WmfBinaryRasterOperation::CopyPen) {
      return false;
    }
    let Some(drawing) = &mut self.drawing else {
      return false;
    };
    drawing.push(MetafileVectorDraw::Stroke {
      subpaths,
      color,
      width,
      closed,
      line_cap: if closed || self.wmf_drawing {
        MetafileVectorLineCap::Flat
      } else {
        MetafileVectorLineCap::Square
      },
      clip: self.graphics.clip,
    });
    true
  }

  fn emit_polyline(&mut self, points: &[EmfPoint]) -> bool {
    if self.graphics.pen == VectorPen::Null || points.len() < 2 {
      return true;
    }
    let Some(mapped) = points
      .iter()
      .map(|point| self.map_drawing_point(*point))
      .collect::<Option<Vec<_>>>()
    else {
      return false;
    };
    self.emit_mapped_stroke(vec![mapped], false)
  }

  fn emit_rectangle(&mut self, rect: crate::types::RectL) -> bool {
    if self.word_pens.is_some() {
      return word_pen::rectangle(self, rect);
    }
    if self.graphics.brush == VectorBrush::Unsupported {
      return false;
    }
    let Some(right) = rect.right.checked_add(1) else {
      return false;
    };
    let Some(bottom) = rect.bottom.checked_add(1) else {
      return false;
    };
    let points = [
      EmfPoint {
        x: rect.left,
        y: rect.top,
      },
      EmfPoint {
        x: right,
        y: rect.top,
      },
      EmfPoint {
        x: right,
        y: bottom,
      },
      EmfPoint {
        x: rect.left,
        y: bottom,
      },
    ];
    let Some(mapped) = points
      .into_iter()
      .map(|point| self.graphics.mapping.map_point(point))
      .collect::<Option<Vec<_>>>()
    else {
      return false;
    };
    self.emit_mapped_polygons(vec![mapped])
  }

  fn intersect_clip(&mut self, rect: crate::types::RectL) -> bool {
    let Some(mut clip) = self.graphics.mapping.map_clip_rect(rect) else {
      return false;
    };
    if let Some(previous) = self.graphics.clip {
      clip = [
        previous[0].max(clip[0]),
        previous[1].max(clip[1]),
        previous[2].min(clip[2]),
        previous[3].min(clip[3]),
      ];
    }
    self.graphics.clip = Some(clip);
    true
  }

  fn finish(self) -> MetafileVectorScene {
    MetafileVectorScene { fills: self.fills }
  }
}

struct VectorEmfDeviceContextBridge {
  graphics: VectorGraphicsState,
  saved: Vec<VectorGraphicsState>,
  objects: HashMap<u32, VectorObject>,
}

fn begin_vector_emf_device_context(
  interpreter: &mut VectorInterpreter,
  objects: &mut HashMap<u32, VectorObject>,
) -> VectorEmfDeviceContextBridge {
  let bridge = VectorEmfDeviceContextBridge {
    graphics: interpreter.graphics,
    saved: std::mem::take(&mut interpreter.saved),
    objects: std::mem::take(objects),
  };
  interpreter.graphics = VectorGraphicsState {
    mapping: interpreter.graphics.mapping.reset_for_emf_device_context(),
    brush: VectorBrush::Unsupported,
    pen: VectorPen::SolidCosmetic([0; 3]),
    binary_raster_operation: Some(WmfBinaryRasterOperation::CopyPen),
    fill_rule: MetafileVectorFillRule::Alternate,
    current_pos: EmfPoint { x: 0, y: 0 },
    clip: None,
  };
  bridge
}

fn end_vector_emf_device_context(
  bridge: VectorEmfDeviceContextBridge,
  interpreter: &mut VectorInterpreter,
  objects: &mut HashMap<u32, VectorObject>,
) {
  interpreter.graphics = bridge.graphics;
  interpreter.saved = bridge.saved;
  *objects = bridge.objects;
}

#[derive(Clone, Copy, Debug, Default)]
struct VectorEmfPlusControl {
  header: bool,
  get_dc: bool,
}

fn nonpainting_emf_plus_control(
  records: &[crate::emfplus::EmfPlusRecord],
) -> Result<Option<VectorEmfPlusControl>, String> {
  let mut control = VectorEmfPlusControl::default();
  for record in records {
    let parsed = record
      .as_ref()
      .parse_data_relaxed()
      .map_err(|error| error.to_string())?;
    match parsed {
      EmfPlusRecordData::Header(_) => control.header = true,
      EmfPlusRecordData::GetDc => control.get_dc = true,
      EmfPlusRecordData::Eof
      | EmfPlusRecordData::SetPixelOffsetMode(_)
      | EmfPlusRecordData::SetAntiAliasMode(_)
      | EmfPlusRecordData::SetCompositingQuality(_)
      | EmfPlusRecordData::SetPageTransform(_)
      | EmfPlusRecordData::SetInterpolationMode(_)
      | EmfPlusRecordData::SetWorldTransform(_) => {}
      _ => return Ok(None),
    }
  }
  Ok(Some(control))
}

fn is_lifted_emf_solid_patcopy(value: &crate::emf::EmrBitBlt, brush: VectorBrush) -> bool {
  matches!(brush, VectorBrush::Solid(_))
    && value.raster_operation_code() == WmfTernaryRasterOperationCode::PATCOPY
    && value.bitmap.is_none()
    && value.dest_size.cx > 0
    && value.dest_size.cy > 0
}

fn emf_line_is_covered_by_following_patcopy(
  start: EmfPoint,
  end: EmfPoint,
  graphics: VectorGraphicsState,
  value: &crate::emf::EmrBitBlt,
) -> bool {
  let (VectorPen::SolidCosmetic(pen_color), VectorBrush::Solid(brush_color)) =
    (graphics.pen, graphics.brush)
  else {
    return false;
  };
  if pen_color != brush_color
    || graphics.binary_raster_operation != Some(WmfBinaryRasterOperation::CopyPen)
    || !graphics.mapping.is_identity_device_mapping()
    || !is_lifted_emf_solid_patcopy(value, graphics.brush)
  {
    return false;
  }

  let horizontal = start.y == end.y
    && end.x > start.x
    && value.dest.x == start.x
    && value.dest.y == start.y
    && value.dest_size.cx == end.x - start.x
    && value.dest_size.cy == 1;
  let vertical = start.x == end.x
    && end.y > start.y
    && value.dest.x == start.x
    && value.dest.y == start.y
    && value.dest_size.cx == 1
    && value.dest_size.cy == end.y - start.y;
  horizontal || vertical
}

fn emf_line_can_be_covered_by_patcopy(
  start: EmfPoint,
  end: EmfPoint,
  graphics: VectorGraphicsState,
) -> bool {
  matches!(graphics.pen, VectorPen::SolidCosmetic(_))
    && graphics.binary_raster_operation == Some(WmfBinaryRasterOperation::CopyPen)
    && graphics.mapping.is_identity_device_mapping()
    && ((start.y == end.y && end.x > start.x) || (start.x == end.x && end.y > start.y))
}

fn extract_emf_scene(
  data: &[u8],
  options: RenderOptions,
) -> Result<Option<MetafileVectorScene>, String> {
  Ok(extract_emf_interpreter(data, options, false)?.map(VectorInterpreter::finish))
}

fn extract_emf_interpreter(
  data: &[u8],
  options: RenderOptions,
  drawing: bool,
) -> Result<Option<VectorInterpreter>, String> {
  let metafile = EmfMetafileRef::from_bytes(data).map_err(|error| error.to_string())?;
  let mut interpreter = VectorInterpreter::new(VectorMapping::emf(data)?);
  if drawing {
    interpreter.drawing = Some(Vec::new());
    if options.word_emf_vector_pens {
      interpreter.word_pens =
        super::metafile_physical_size(data, Some("image/emf")).and_then(word_pen::State::new);
    }
  }
  let mut objects = HashMap::<u32, VectorObject>::new();
  let mut saw_eof = false;
  let mut saw_intersect_clip = false;
  let decomposed_stream = options.suppress_text && options.suppress_solid_pattern_rects;
  let mut emf_plus_playback = false;
  let mut emf_device_context = None;
  let mut pending_line = None;
  for record in metafile.records() {
    let record_type = record.record_type;
    let record_data = record.data;
    let record = record.parse_data().map_err(|error| error.to_string())?;
    let is_emf_plus_comment = matches!(&record, EmfRecordData::Comment(EmrComment::EmfPlus { .. }));
    if is_emf_plus_comment && let Some(bridge) = emf_device_context.take() {
      end_vector_emf_device_context(bridge, &mut interpreter, &mut objects);
    }
    if emf_plus_playback
      && emf_device_context.is_none()
      && !is_emf_plus_comment
      && !matches!(&record, EmfRecordData::Eof(_))
    {
      continue;
    }
    if pending_line.is_some()
      && !matches!(
        &record,
        EmfRecordData::SelectObject(_) | EmfRecordData::BitBlt(_)
      )
    {
      return Ok(None);
    }
    let supported = match record {
      EmfRecordData::Header(_) => true,
      EmfRecordData::Eof(_) => {
        saw_eof = true;
        true
      }
      EmfRecordData::SetWindowOrgEx(value) => {
        interpreter.graphics.mapping.window_org_x = value.origin.x;
        interpreter.graphics.mapping.window_org_y = value.origin.y;
        true
      }
      EmfRecordData::SetViewportOrgEx(value) => {
        interpreter.graphics.mapping.viewport_org_x = value.origin.x;
        interpreter.graphics.mapping.viewport_org_y = value.origin.y;
        true
      }
      EmfRecordData::SetWindowExtEx(_)
      | EmfRecordData::SetViewportExtEx(_)
      | EmfRecordData::ScaleWindowExtEx(_)
      | EmfRecordData::ScaleViewportExtEx(_)
      | EmfRecordData::SetMapMode(_) => {
        let mapping = &mut interpreter.graphics.mapping;
        let extents = super::emf_mapping::Extents {
          mode: mapping.map_mode,
          window: [mapping.window_ext_x, mapping.window_ext_y],
          viewport: [mapping.viewport_ext_x, mapping.viewport_ext_y],
        }
        .record_payload(data, record_type, record_data);
        mapping.map_mode = extents.mode;
        [mapping.window_ext_x, mapping.window_ext_y] = extents.window;
        [mapping.viewport_ext_x, mapping.viewport_ext_y] = extents.viewport;
        true
      }
      EmfRecordData::SetPolyFillMode(value) => {
        let Some(rule) = emf_fill_rule(value.polygon_fill_mode_kind()) else {
          return Ok(None);
        };
        interpreter.graphics.fill_rule = rule;
        true
      }
      EmfRecordData::SetRop2(value) => {
        interpreter.graphics.binary_raster_operation = value.binary_raster_operation_kind();
        interpreter.graphics.binary_raster_operation.is_some()
      }
      EmfRecordData::SaveDc => {
        interpreter.save();
        true
      }
      EmfRecordData::RestoreDc(value) => value.saved_dc == -1 && interpreter.restore_latest(),
      EmfRecordData::SetWorldTransform(value) => {
        interpreter.graphics.mapping.world_transform = emf_transform(value.transform);
        true
      }
      EmfRecordData::ModifyWorldTransform(value) => {
        let transform = emf_transform(value.transform);
        let current = interpreter.graphics.mapping.world_transform;
        let Some(updated) = (match value.mode_kind() {
          Some(EmrModifyWorldTransformMode::Identity) => Some(EmfTransform::identity()),
          Some(EmrModifyWorldTransformMode::LeftMultiply) => Some(transform.multiply(current)),
          Some(EmrModifyWorldTransformMode::RightMultiply) => Some(current.multiply(transform)),
          Some(EmrModifyWorldTransformMode::Set) => Some(transform),
          None => None,
        }) else {
          return Ok(None);
        };
        interpreter.graphics.mapping.world_transform = updated;
        true
      }
      EmfRecordData::CreatePen(value) => {
        let pen = if interpreter.word_pens.is_some() {
          word_pen::classic_pen(&value)
        } else {
          match value.pen_line_style_kind() {
            Some(EmrPenLineStyle::Null) => VectorPen::Null,
            Some(EmrPenLineStyle::Solid)
              if value.pen_type_kind() == Some(EmrPenType::Cosmetic)
                && (value.width.x == 0 || (drawing && value.width.x == 1))
                && value.width.y == 0
                && value.pen_reserved_bits() == 0 =>
            {
              VectorPen::SolidCosmetic(color(value.color))
            }
            _ => VectorPen::Unsupported,
          }
        };
        objects.insert(value.object_index, VectorObject::Pen(pen));
        true
      }
      EmfRecordData::ExtCreatePen(value) => {
        objects.insert(
          value.object_index,
          VectorObject::Pen(if interpreter.word_pens.is_some() {
            word_pen::extended_pen(&value)
          } else if value.pen_line_style_kind() == Some(EmrPenLineStyle::Null) {
            VectorPen::Null
          } else {
            VectorPen::Unsupported
          }),
        );
        true
      }
      EmfRecordData::CreateBrushIndirect(value) => {
        objects.insert(
          value.object_index,
          VectorObject::Brush(match value.brush_style_kind() {
            Some(WmfBrushStyle::Solid) => VectorBrush::Solid(color(value.color)),
            Some(WmfBrushStyle::Null) => VectorBrush::Null,
            _ => VectorBrush::Unsupported,
          }),
        );
        true
      }
      EmfRecordData::ExtCreateFontIndirectW(value) => {
        objects.insert(value.object_index, VectorObject::Other);
        true
      }
      EmfRecordData::SelectObject(value) => {
        select_emf_object(value.object_index, &objects, &mut interpreter)
      }
      EmfRecordData::DeleteObject(value) => {
        objects.remove(&value.object_index);
        true
      }
      EmfRecordData::MoveToEx(value) => {
        interpreter.graphics.current_pos = point_l(value.point);
        true
      }
      EmfRecordData::LineTo(value) => {
        let start = interpreter.graphics.current_pos;
        let end = point_l(value.point);
        let supported = if interpreter.graphics.pen == VectorPen::Null {
          true
        } else if interpreter.word_pens.is_some() {
          word_pen::line(&mut interpreter, start, end)
        } else if drawing {
          interpreter.emit_polyline(&[start, end])
        } else if options.suppress_solid_pattern_rects
          && emf_line_can_be_covered_by_patcopy(start, end, interpreter.graphics)
        {
          pending_line = Some((start, end, interpreter.graphics));
          true
        } else {
          false
        };
        interpreter.graphics.current_pos = end;
        supported
      }
      EmfRecordData::Polyline(value) if drawing => {
        let points = value.points.into_iter().map(point_l).collect::<Vec<_>>();
        interpreter.emit_polyline(&points)
      }
      EmfRecordData::Polyline16(value) if drawing => {
        let points = value.points.into_iter().map(point_s).collect::<Vec<_>>();
        interpreter.emit_polyline(&points)
      }
      EmfRecordData::Polyline(_) | EmfRecordData::Polyline16(_) => {
        interpreter.graphics.pen == VectorPen::Null
      }
      EmfRecordData::PolylineTo(value) => {
        if let Some(point) = value.points.last().copied() {
          interpreter.graphics.current_pos = point_l(point);
        }
        interpreter.graphics.pen == VectorPen::Null
      }
      EmfRecordData::PolylineTo16(value) => {
        if let Some(point) = value.points.last().copied() {
          interpreter.graphics.current_pos = point_s(point);
        }
        interpreter.graphics.pen == VectorPen::Null
      }
      EmfRecordData::PolyPolyline(_) | EmfRecordData::PolyPolyline16(_) => {
        interpreter.graphics.pen == VectorPen::Null
      }
      EmfRecordData::Polygon(value) => {
        let points = value.points.into_iter().map(point_l).collect::<Vec<_>>();
        interpreter.emit_polygon(&points)
      }
      EmfRecordData::Polygon16(value) => {
        let points = value.points.into_iter().map(point_s).collect::<Vec<_>>();
        interpreter.emit_polygon(&points)
      }
      EmfRecordData::PolyPolygon(value) => {
        let polygons = split_emf_polygons_l(&value.counts, &value.points);
        interpreter.emit_polygons(&polygons)
      }
      EmfRecordData::PolyPolygon16(value) => {
        let polygons = split_emf_polygons_s(&value.counts, &value.points);
        interpreter.emit_polygons(&polygons)
      }
      EmfRecordData::Rectangle(value) if drawing => interpreter.emit_rectangle(value.bounds),
      EmfRecordData::IntersectClipRect(value) if drawing => interpreter.intersect_clip(value.rect),
      EmfRecordData::IntersectClipRect(_) if decomposed_stream => {
        saw_intersect_clip = true;
        true
      }
      EmfRecordData::ExtSelectClipRgn(value) => {
        if let Some(region) = super::emf_mapping::copied_rectangle(&value) {
          let mapping = interpreter.graphics.mapping;
          interpreter.graphics.clip = region.map(|rect| {
            super::emf_mapping::device_edges(
              rect,
              [mapping.playback_origin_x, mapping.playback_origin_y],
              [
                mapping.playback_scale_x / mapping.surface_width,
                mapping.playback_scale_y / mapping.surface_height,
              ],
            )
          });
          true
        } else {
          false
        }
      }
      EmfRecordData::ExtTextOutA(_)
      | EmfRecordData::ExtTextOutW(_)
      | EmfRecordData::PolyTextOutA(_)
      | EmfRecordData::PolyTextOutW(_)
        if options.suppress_text =>
      {
        true
      }
      EmfRecordData::BitBlt(value) if options.suppress_solid_pattern_rects => {
        if let Some((start, end, mut graphics)) = pending_line.take() {
          graphics.brush = interpreter.graphics.brush;
          emf_line_is_covered_by_following_patcopy(start, end, graphics, &value)
        } else {
          is_lifted_emf_solid_patcopy(&value, interpreter.graphics.brush)
        }
      }
      EmfRecordData::Comment(EmrComment::EmfPlus { records, .. }) if decomposed_stream => {
        match nonpainting_emf_plus_control(&records)? {
          Some(control) => {
            emf_plus_playback |= control.header;
            if emf_plus_playback && control.get_dc {
              emf_device_context = Some(begin_vector_emf_device_context(
                &mut interpreter,
                &mut objects,
              ));
            }
            true
          }
          None => false,
        }
      }
      EmfRecordData::Comment(EmrComment::EmfPlus { .. }) => false,
      EmfRecordData::Comment(
        EmrComment::Public {
          comment:
            EmrPublicComment::WindowsMetafile(_)
            | EmrPublicComment::BeginGroup(_)
            | EmrPublicComment::EndGroup,
          ..
        }
        | EmrComment::PrivateData { .. }
        | EmrComment::EmfSpool { .. }
        | EmrComment::Raw { .. },
      ) => true,
      // These state records cannot affect the accepted solid-brush/null-pen
      // polygon subset. Text, bitmap, region, path, and escape records are
      // deliberately absent and force raster fallback below.
      EmfRecordData::SetBrushOrgEx(_)
      | EmfRecordData::SetMapperFlags(_)
      | EmfRecordData::SetBkMode(_)
      | EmfRecordData::SetStretchBltMode(_)
      | EmfRecordData::SetTextAlign(_)
      | EmfRecordData::SetColorAdjustment(_)
      | EmfRecordData::SetTextColor(_)
      | EmfRecordData::SetBkColor(_)
      | EmfRecordData::SelectPalette(_)
      | EmfRecordData::ResizePalette(_)
      | EmfRecordData::RealizePalette
      | EmfRecordData::SetArcDirection(_)
      | EmfRecordData::SetMiterLimit(_)
      | EmfRecordData::SetTextJustification(_) => true,
      _ => false,
    };
    if !supported {
      return Ok(None);
    }
    if saw_eof {
      break;
    }
  }

  if pending_line.is_some() || (saw_intersect_clip && !interpreter.fills.is_empty()) {
    return Ok(None);
  }
  Ok(saw_eof.then_some(interpreter))
}

fn extract_wmf_scene(
  data: &[u8],
  options: RenderOptions,
) -> Result<Option<MetafileVectorScene>, String> {
  extract_wmf_interpreter(data, options, WmfVectorMode::Scene, None)
    .map(|interpreter| interpreter.map(VectorInterpreter::finish))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WmfVectorMode {
  Scene,
  Drawing,
  BeforeText,
  Picture,
}

#[derive(Clone, Copy)]
enum WmfLiftObject {
  Font(Option<u8>),
  Other,
}

#[derive(Clone, Copy)]
struct WmfLiftTextState {
  font_charset: Option<u8>,
  baseline_left: bool,
  transparent: bool,
  direct_color: bool,
  anisotropic: bool,
}

impl WmfLiftTextState {
  fn admits(self, text: &[u8], dx: &[i16]) -> bool {
    if !(self.baseline_left && self.transparent && self.direct_color && self.anisotropic)
      || text.is_empty()
      || text.iter().any(|byte| *byte < 32)
    {
      return false;
    }
    match self.font_charset {
      Some(0..=2) => dx.is_empty() || dx.len() == text.len(),
      // The selected LOGFONT supplies the code page for ExtTextOutA. The
      // extractor can carry decoded characters with natural advances. A
      // DBCS byte-indexed dx array requires a separate advance conversion.
      Some(charset) => {
        dx.is_empty()
          && crate::string::SdkEncoding::WmfCharset(charset)
            .decode(text)
            .is_ok()
      }
      None => false,
    }
  }
}

/// A complete graphics prefix followed by selected-font, transparent text.
///
/// MS-WMF's object table and DC lifetimes apply to font selection too. The
/// existing extractor represents baseline-left, negative-height text; its
/// remaining color, clipping, decoration, spacing and bitmap-font cases stay
/// conservative. No later graphics may move in front of the lifted text.
pub(super) fn word_wmf_text_can_be_lifted(data: &[u8], options: RenderOptions) -> bool {
  use crate::wmf::WmfRecordFunction as F;
  let Ok(source) = WmfMetafileRef::from_bytes(data) else {
    return false;
  };
  if !options.word_emf_vector_pens
    || !source
      .placeable_header
      .is_some_and(|h| h.inch != 0 && h.right > h.left && h.bottom > h.top)
  {
    return false;
  }
  let mut dc = WmfLiftTextState {
    font_charset: None,
    baseline_left: false,
    transparent: false,
    direct_color: true,
    anisotropic: true,
  };
  let mut saved = Vec::new();
  let mut objects = vec![None; source.header.number_of_objects as usize];
  let mut text_seen = false;
  let mut bitmap_seen = false;
  let mut eof = false;
  for record in source.records() {
    if record.normalized_function_kind() == Some(F::SetRelabs) {
      continue;
    }
    let Ok(record) = record.parse_data() else {
      return false;
    };
    match record {
      WmfRecordData::Eof(_) => {
        eof = true;
        break;
      }
      WmfRecordData::SaveDc => saved.push(dc),
      WmfRecordData::RestoreDc(value) if value.value == -1 => {
        let Some(previous) = saved.pop() else {
          return false;
        };
        dc = previous;
      }
      WmfRecordData::RestoreDc(_) => return false,
      WmfRecordData::SetMapMode(value) => {
        dc.anisotropic = value.map_mode_kind() == Some(WmfMapMode::Anisotropic);
      }
      WmfRecordData::SetWindowExt(value) | WmfRecordData::SetViewportExt(value)
        if value.x <= 0 || value.y <= 0 =>
      {
        return false;
      }
      WmfRecordData::ScaleWindowExt(value) | WmfRecordData::ScaleViewportExt(value)
        if value.x_num <= 0 || value.y_num <= 0 || value.x_denom <= 0 || value.y_denom <= 0 =>
      {
        return false;
      }
      WmfRecordData::SetTextAlign(value) => dc.baseline_left = value.value == 24,
      WmfRecordData::SetBkMode(value) => dc.transparent = value.value == 1,
      // COLORREF RGB and PALETTERGB both carry the requested RGB components.
      // PALETTEINDEX needs an active logical palette to resolve its color.
      WmfRecordData::SetTextColor(value) => dc.direct_color = matches!(value.color.reserved, 0 | 2),
      WmfRecordData::SetTextCharExtra(value) if value.value != 0 => return false,
      WmfRecordData::SetTextJustification(_) => return false,
      // MS-WMF 2.3.5.18 uses bit zero to request a device-aspect match
      // during font selection. Word's fixed-output DC realizes the same
      // outline and placement for both settings. The selected LOGFONT and
      // the text records below still determine whether we can lift the run.
      WmfRecordData::SetMapperFlags(flags) if flags.value > 1 => return false,
      WmfRecordData::CreateFontIndirect(font) => {
        // Smaller logical fonts hit the existing extractor's seven-unit
        // raster minimum; do not lift them as an unclamped outline size.
        let supported = font.height <= -7
          && font.width == 0
          && font.escapement == 0
          && font.orientation == 0
          && font.underline == 0
          && font.strike_out == 0
          // MetafileTextRun carries a bold flag rather than arbitrary weights.
          && matches!(font.weight, 0 | 400 | 700)
          && matches!(font.char_set, 0..=2 | 128 | 129 | 134 | 136)
          && font.face_name.first().is_some_and(|b| *b != 0);
        insert_wmf_object(
          &mut objects,
          WmfLiftObject::Font(supported.then_some(font.char_set)),
        );
      }
      WmfRecordData::CreatePenIndirect(_)
      | WmfRecordData::CreateBrushIndirect(_)
      | WmfRecordData::CreatePalette(_)
      | WmfRecordData::CreatePatternBrush(_)
      | WmfRecordData::CreateRegion(_)
      | WmfRecordData::DibCreatePatternBrush(_) => {
        insert_wmf_object(&mut objects, WmfLiftObject::Other)
      }
      WmfRecordData::SelectObject(value) => {
        match objects.get(value.index as usize).copied().flatten() {
          Some(WmfLiftObject::Font(charset)) => dc.font_charset = charset,
          Some(WmfLiftObject::Other) => {}
          None => return false,
        }
      }
      WmfRecordData::DeleteObject(value) => {
        if let Some(slot) = objects.get_mut(value.index as usize) {
          *slot = None;
        }
      }
      WmfRecordData::TextOut(value) => {
        if !dc.admits(&value.string, &[]) {
          return false;
        }
        text_seen = true;
      }
      WmfRecordData::ExtTextOut(value) => {
        if !value.options.is_empty() || !dc.admits(&value.string, &value.dx) {
          return false;
        }
        text_seen = true;
      }
      WmfRecordData::LineTo(_)
      | WmfRecordData::Polyline(_)
      | WmfRecordData::Polygon(_)
      | WmfRecordData::PolyPolygon(_)
        if text_seen =>
      {
        return false;
      }
      WmfRecordData::StretchDib(_) => {
        if text_seen {
          return false;
        }
        bitmap_seen = true;
      }
      _ => {}
    }
  }
  eof
    && text_seen
    && matches!(
      extract_wmf_interpreter(
        data,
        RenderOptions {
          suppress_text: true,
          ..options
        },
        if bitmap_seen {
          WmfVectorMode::Picture
        } else {
          WmfVectorMode::BeforeText
        },
        None
      ),
      Ok(Some(_))
    )
}

fn extract_wmf_interpreter(
  data: &[u8],
  options: RenderOptions,
  mode: WmfVectorMode,
  mut picture_layers: Option<&mut Vec<MetafilePictureLayer>>,
) -> Result<Option<VectorInterpreter>, String> {
  let metafile = WmfMetafileRef::from_bytes(data).map_err(|error| error.to_string())?;
  let mut interpreter = VectorInterpreter::new(VectorMapping::wmf(&metafile, options));
  let drawing = mode != WmfVectorMode::Scene;
  if drawing {
    interpreter.drawing = Some(Vec::new());
    interpreter.wmf_drawing = true;
    interpreter.graphics.pen = VectorPen::WmfLogical {
      color: [0; 3],
      width: 1,
    };
  }
  let mut objects = vec![None; metafile.header.number_of_objects as usize];
  let mut saw_eof = false;
  let mut text_seen = false;

  for record in metafile.records() {
    if (mode == WmfVectorMode::BeforeText
      || (mode == WmfVectorMode::Picture && options.suppress_text))
      && matches!(
        record.normalized_function_kind(),
        Some(crate::wmf::WmfRecordFunction::TextOut | crate::wmf::WmfRecordFunction::ExtTextOut)
      )
    {
      text_seen = true;
      continue;
    }
    // [MS-WMF] 2.3.5.21 reserves SETRELABS and requires players to ignore
    // it. Historical producers retain a WORD parameter. Do not feed that
    // unused payload through the unit-variant decoder or discard its bytes
    // from the editable metafile model.
    if record.normalized_function_kind() == Some(crate::wmf::WmfRecordFunction::SetRelabs) {
      continue;
    }
    let record = record.parse_data().map_err(|error| error.to_string())?;
    if mode == WmfVectorMode::Picture
      && text_seen
      && matches!(
        record,
        WmfRecordData::Polygon(_)
          | WmfRecordData::PolyPolygon(_)
          | WmfRecordData::LineTo(_)
          | WmfRecordData::Polyline(_)
          | WmfRecordData::StretchDib(_)
      )
    {
      return Ok(None);
    }
    let supported = match record {
      WmfRecordData::Eof(_) => {
        saw_eof = true;
        true
      }
      WmfRecordData::SaveDc => {
        interpreter.save();
        true
      }
      WmfRecordData::RestoreDc(value) => value.value == -1 && interpreter.restore_latest(),
      WmfRecordData::SetWindowOrg(value) => {
        interpreter.graphics.mapping.window_org_x = i32::from(value.x);
        interpreter.graphics.mapping.window_org_y = i32::from(value.y);
        true
      }
      WmfRecordData::SetWindowExt(value) => {
        interpreter.graphics.mapping.window_ext_x = nonzero_mapping_extent(i32::from(value.x));
        interpreter.graphics.mapping.window_ext_y = nonzero_mapping_extent(i32::from(value.y));
        true
      }
      WmfRecordData::SetViewportOrg(value) => {
        interpreter.graphics.mapping.viewport_org_x = i32::from(value.x);
        interpreter.graphics.mapping.viewport_org_y = i32::from(value.y);
        true
      }
      WmfRecordData::SetViewportExt(value) => {
        interpreter.graphics.mapping.viewport_ext_x = nonzero_mapping_extent(i32::from(value.x));
        interpreter.graphics.mapping.viewport_ext_y = nonzero_mapping_extent(i32::from(value.y));
        true
      }
      WmfRecordData::OffsetWindowOrg(value) => {
        interpreter.graphics.mapping.window_org_x = interpreter
          .graphics
          .mapping
          .window_org_x
          .saturating_add(i32::from(value.x));
        interpreter.graphics.mapping.window_org_y = interpreter
          .graphics
          .mapping
          .window_org_y
          .saturating_add(i32::from(value.y));
        true
      }
      WmfRecordData::OffsetViewportOrg(value) => {
        interpreter.graphics.mapping.viewport_org_x = interpreter
          .graphics
          .mapping
          .viewport_org_x
          .saturating_add(i32::from(value.x));
        interpreter.graphics.mapping.viewport_org_y = interpreter
          .graphics
          .mapping
          .viewport_org_y
          .saturating_add(i32::from(value.y));
        true
      }
      WmfRecordData::ScaleWindowExt(value) => {
        interpreter.graphics.mapping.window_ext_x = nonzero_mapping_extent(scale_wmf_extent(
          interpreter.graphics.mapping.window_ext_x,
          value.x_num,
          value.x_denom,
        ));
        interpreter.graphics.mapping.window_ext_y = nonzero_mapping_extent(scale_wmf_extent(
          interpreter.graphics.mapping.window_ext_y,
          value.y_num,
          value.y_denom,
        ));
        true
      }
      WmfRecordData::ScaleViewportExt(value) => {
        interpreter.graphics.mapping.viewport_ext_x = nonzero_mapping_extent(scale_wmf_extent(
          interpreter.graphics.mapping.viewport_ext_x,
          value.x_num,
          value.x_denom,
        ));
        interpreter.graphics.mapping.viewport_ext_y = nonzero_mapping_extent(scale_wmf_extent(
          interpreter.graphics.mapping.viewport_ext_y,
          value.y_num,
          value.y_denom,
        ));
        true
      }
      WmfRecordData::SetMapMode(value) => matches!(
        WmfMapMode::from_raw(value.value),
        Some(WmfMapMode::Text | WmfMapMode::Anisotropic)
      ),
      WmfRecordData::SetPolyFillMode(value) => {
        let Some(rule) = wmf_fill_rule(WmfPolyFillMode::from_raw(value.value)) else {
          return Ok(None);
        };
        interpreter.graphics.fill_rule = rule;
        true
      }
      WmfRecordData::CreatePenIndirect(value) => {
        insert_wmf_object(
          &mut objects,
          VectorObject::Pen(
            if value.pen.pen_line_style_kind() == Some(WmfPenLineStyle::Null) {
              VectorPen::Null
            } else if drawing
              && value.pen.pen_style == WmfPenLineStyle::Solid.raw()
              && value.pen.width.y == 0
            {
              // Word's WMF conversion realizes a zero-width pen as one
              // logical unit, before either picture axis is transformed.
              // This differs from the standalone GDI cosmetic pixel.
              VectorPen::WmfLogical {
                color: color(value.pen.color_ref),
                width: value.pen.width.x.unsigned_abs().max(1),
              }
            } else {
              VectorPen::Unsupported
            },
          ),
        );
        true
      }
      WmfRecordData::CreateBrushIndirect(value) => {
        insert_wmf_object(
          &mut objects,
          VectorObject::Brush(match value.brush_style_kind() {
            Some(WmfBrushStyle::Solid) => VectorBrush::Solid(color(value.color_ref)),
            Some(WmfBrushStyle::Null) => VectorBrush::Null,
            _ => VectorBrush::Unsupported,
          }),
        );
        true
      }
      WmfRecordData::CreatePatternBrush(_) | WmfRecordData::DibCreatePatternBrush(_) => {
        insert_wmf_object(&mut objects, VectorObject::Brush(VectorBrush::Unsupported));
        true
      }
      WmfRecordData::CreateFontIndirect(_)
      | WmfRecordData::CreatePalette(_)
      | WmfRecordData::CreateRegion(_) => {
        insert_wmf_object(&mut objects, VectorObject::Other);
        true
      }
      WmfRecordData::SelectObject(value) => {
        select_wmf_object(value.index, &objects, &mut interpreter)
      }
      WmfRecordData::DeleteObject(value) => {
        if let Some(slot) = objects.get_mut(value.index as usize) {
          *slot = None;
        }
        true
      }
      WmfRecordData::MoveTo(value) => {
        interpreter.graphics.current_pos = EmfPoint {
          x: i32::from(value.x),
          y: i32::from(value.y),
        };
        true
      }
      WmfRecordData::LineTo(value) => {
        let start = interpreter.graphics.current_pos;
        let end = EmfPoint {
          x: i32::from(value.x),
          y: i32::from(value.y),
        };
        let supported = if drawing {
          interpreter.emit_wmf_line(start, end)
        } else {
          interpreter.graphics.pen == VectorPen::Null
        };
        interpreter.graphics.current_pos = end;
        supported
      }
      WmfRecordData::Polyline(value) if drawing => {
        let points = value.points.into_iter().map(point_s).collect::<Vec<_>>();
        interpreter.emit_polyline(&points)
      }
      WmfRecordData::Polyline(_) => interpreter.graphics.pen == VectorPen::Null,
      WmfRecordData::Polygon(value) => {
        let points = value.points.into_iter().map(point_s).collect::<Vec<_>>();
        interpreter.emit_polygon(&points)
      }
      WmfRecordData::PolyPolygon(value) => {
        let polygons = split_wmf_polygons(&value.points_per_polygon, &value.points);
        interpreter.emit_polygons(&polygons)
      }
      // Rectangle paints its interior and outline with the selected brush
      // and pen. With both null there is no output or current-position change.
      WmfRecordData::Rectangle(_) => {
        interpreter.graphics.brush == VectorBrush::Null
          && interpreter.graphics.pen == VectorPen::Null
      }
      WmfRecordData::StretchDib(value) if mode == WmfVectorMode::Picture => {
        let Some(source) = word_wmf_copy_bitmap(&value) else {
          return Ok(None);
        };
        if let Some(layers) = picture_layers.as_deref_mut() {
          let map = interpreter.graphics.mapping;
          let origin = EmfPoint {
            x: i32::from(value.x_dest),
            y: i32::from(value.y_dest),
          };
          let end = EmfPoint {
            x: origin.x + i32::from(value.dest_width),
            y: origin.y + i32::from(value.dest_height),
          };
          let (Some(a), Some(b)) = (map.map_point(origin), map.map_point(end)) else {
            return Ok(None);
          };
          let size = [source.width as u32, source.height as u32];
          let data = super::rgb_to_png(&source.rgb, size[0], size[1])?;
          flush_wmf_drawing(&mut interpreter, layers);
          layers.push(MetafilePictureLayer::Bitmap(super::MetafileBitmapPicture {
            bitmap: super::MetafileBitmapLayer {
              data,
              content_type: "image/png",
              x: a.x.min(b.x),
              y: a.y.min(b.y),
              width: (b.x - a.x).abs(),
              height: (b.y - a.y).abs(),
              flip_horizontal: b.x < a.x,
              flip_vertical: b.y < a.y,
            },
            source_size: size,
            half_pixel_offset: false,
          }));
        }
        true
      }
      // MS-WMF 2.1.1.17 GETTECHNOLOGY queries the playback device. Its
      // return value cannot change an already-recorded drawing operation.
      WmfRecordData::Escape(value)
        if value.escape_function == crate::wmf::WmfMetafileEscape::GetTechnology.raw()
          && value.escape_data.is_empty() =>
      {
        true
      }
      // MFCOMMENT can carry producer metadata without painting. A valid
      // embedded EMF has already taken precedence in the public extractor;
      // retain the strict fallback for an unconsumed EMF comment instead of
      // silently treating its alternative drawing as ordinary metadata.
      WmfRecordData::Escape(value)
        if value.escape_function == crate::wmf::WmfMetafileEscape::MetaFile.raw()
          && !value
            .escape_data
            .starts_with(&crate::wmf::WMF_EMF_COMMENT_IDENTIFIER.to_le_bytes()) =>
      {
        true
      }
      WmfRecordData::SetRop2(value) if drawing => {
        interpreter.graphics.binary_raster_operation =
          WmfBinaryRasterOperation::from_raw(value.value);
        interpreter.graphics.binary_raster_operation == Some(WmfBinaryRasterOperation::CopyPen)
      }
      // These records only affect output kinds that this strict stream does
      // not accept. Keeping them allows ordinary producer/DC setup while any
      // later text, bitmap, region, or stroked primitive still falls back.
      WmfRecordData::SetRelabs
      | WmfRecordData::SetBkMode(_)
      | WmfRecordData::SetRop2(_)
      | WmfRecordData::SetStretchBltMode(_)
      | WmfRecordData::SetTextAlign(_)
      | WmfRecordData::SetTextCharExtra(_)
      | WmfRecordData::SetMapperFlags(_)
      | WmfRecordData::SetTextJustification(_)
      | WmfRecordData::SetBkColor(_)
      | WmfRecordData::SetTextColor(_)
      | WmfRecordData::RealizePalette => true,
      _ => false,
    };
    if !supported {
      return Ok(None);
    }
    if saw_eof {
      break;
    }
  }

  Ok(saw_eof.then_some(interpreter))
}

fn word_wmf_copy_bitmap(value: &crate::wmf::WmfStretchDibRecord) -> Option<super::RasterPixels> {
  use super::DibColorUsage;
  if value.raster_operation_code() != WmfTernaryRasterOperationCode::SRCCOPY
    || value.color_usage_kind() != Some(DibColorUsage::RgbColors)
    || value.src_width <= 0
    || value.src_height <= 0
    || value.dest_width == 0
    || value.dest_height == 0
  {
    return None;
  }
  super::wmf_dib_source_rect(
    &value.dib,
    DibColorUsage::RgbColors,
    (value.x_src, value.y_src, value.src_width, value.src_height),
  )
}

fn select_emf_object(
  index: u32,
  objects: &HashMap<u32, VectorObject>,
  interpreter: &mut VectorInterpreter,
) -> bool {
  let object = match EmrStockObject::from_raw(index) {
    Some(EmrStockObject::WhiteBrush) => VectorObject::Brush(VectorBrush::Solid([255; 3])),
    Some(EmrStockObject::LtGrayBrush) => VectorObject::Brush(VectorBrush::Solid([192; 3])),
    Some(EmrStockObject::GrayBrush) => VectorObject::Brush(VectorBrush::Solid([128; 3])),
    Some(EmrStockObject::DkGrayBrush) => VectorObject::Brush(VectorBrush::Solid([64; 3])),
    Some(EmrStockObject::BlackBrush) => VectorObject::Brush(VectorBrush::Solid([0; 3])),
    Some(EmrStockObject::NullBrush) => VectorObject::Brush(VectorBrush::Null),
    Some(EmrStockObject::WhitePen) => VectorObject::Pen(VectorPen::SolidCosmetic([255; 3])),
    Some(EmrStockObject::BlackPen) => VectorObject::Pen(VectorPen::SolidCosmetic([0; 3])),
    Some(EmrStockObject::NullPen) => VectorObject::Pen(VectorPen::Null),
    Some(
      EmrStockObject::OemFixedFont
      | EmrStockObject::AnsiFixedFont
      | EmrStockObject::AnsiVarFont
      | EmrStockObject::SystemFont
      | EmrStockObject::DeviceDefaultFont
      | EmrStockObject::DefaultPalette
      | EmrStockObject::SystemFixedFont
      | EmrStockObject::DefaultGuiFont,
    ) => VectorObject::Other,
    Some(EmrStockObject::DcBrush | EmrStockObject::DcPen) => return false,
    None => {
      let Some(object) = objects.get(&index).copied() else {
        return false;
      };
      object
    }
  };
  select_object(object, interpreter);
  true
}

fn select_wmf_object(
  index: u16,
  objects: &[Option<VectorObject>],
  interpreter: &mut VectorInterpreter,
) -> bool {
  let Some(Some(object)) = objects.get(index as usize).copied() else {
    return false;
  };
  select_object(object, interpreter);
  true
}

fn select_object(object: VectorObject, interpreter: &mut VectorInterpreter) {
  match object {
    VectorObject::Brush(brush) => interpreter.graphics.brush = brush,
    VectorObject::Pen(pen) => interpreter.graphics.pen = pen,
    VectorObject::Other => {}
  }
}

fn insert_wmf_object<T>(objects: &mut Vec<Option<T>>, object: T) {
  if let Some(slot) = objects.iter_mut().find(|slot| slot.is_none()) {
    *slot = Some(object);
  } else {
    objects.push(Some(object));
  }
}

fn split_emf_polygons_l(counts: &[u32], points: &[PointL]) -> Vec<Vec<EmfPoint>> {
  split_polygons(counts.iter().map(|count| *count as usize), points, point_l)
}

fn split_emf_polygons_s(counts: &[u32], points: &[PointS]) -> Vec<Vec<EmfPoint>> {
  split_polygons(counts.iter().map(|count| *count as usize), points, point_s)
}

fn split_wmf_polygons(counts: &[u16], points: &[PointS]) -> Vec<Vec<EmfPoint>> {
  split_polygons(counts.iter().map(|count| *count as usize), points, point_s)
}

fn split_polygons<T: Copy>(
  counts: impl IntoIterator<Item = usize>,
  points: &[T],
  convert: fn(T) -> EmfPoint,
) -> Vec<Vec<EmfPoint>> {
  let mut cursor = 0usize;
  let mut polygons = Vec::new();
  for count in counts {
    let end = cursor.saturating_add(count).min(points.len());
    polygons.push(points[cursor..end].iter().copied().map(convert).collect());
    cursor = end;
  }
  polygons
}

fn point_l(point: PointL) -> EmfPoint {
  EmfPoint {
    x: point.x,
    y: point.y,
  }
}

fn point_s(point: PointS) -> EmfPoint {
  EmfPoint {
    x: i32::from(point.x),
    y: i32::from(point.y),
  }
}

fn color(color: ColorRef) -> [u8; 3] {
  [color.red, color.green, color.blue]
}

fn emf_transform(transform: XForm) -> EmfTransform {
  EmfTransform {
    m11: transform.m11,
    m12: transform.m12,
    m21: transform.m21,
    m22: transform.m22,
    dx: transform.dx,
    dy: transform.dy,
  }
}

fn emf_fill_rule(rule: Option<EmrPolygonFillMode>) -> Option<MetafileVectorFillRule> {
  match rule {
    Some(EmrPolygonFillMode::Alternate) => Some(MetafileVectorFillRule::Alternate),
    Some(EmrPolygonFillMode::Winding) => Some(MetafileVectorFillRule::Winding),
    None => None,
  }
}

fn wmf_fill_rule(rule: Option<WmfPolyFillMode>) -> Option<MetafileVectorFillRule> {
  match rule {
    Some(WmfPolyFillMode::Alternate) => Some(MetafileVectorFillRule::Alternate),
    Some(WmfPolyFillMode::Winding) => Some(MetafileVectorFillRule::Winding),
    None => None,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::render::WmfConversionProfile;
  use crate::wmf::WmfPlaceableHeader;

  fn word_wmf_primitives(pen_style: u16, width: i16) -> crate::wmf::WmfMetafile {
    use crate::wmf::*;
    let bytes = [
      1, 0, 9, 0, 0, 3, 12, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0,
    ];
    let mut source = WmfMetafile::from_bytes(&bytes).unwrap();
    source.header.number_of_objects = 2;
    let points = vec![
      PointS { x: 1, y: 2 },
      PointS { x: 9, y: 2 },
      PointS { x: 5, y: 18 },
    ];
    source.records = vec![
      WmfRecordData::SetWindowExt(WmfPointRecord { x: 10, y: 20 })
        .to_record()
        .unwrap(),
      WmfRecordData::CreatePenIndirect(WmfCreatePenIndirectRecord {
        pen: WmfPenObject {
          pen_style,
          width: PointS { x: width, y: 0 },
          ..Default::default()
        },
        trailing_data: Vec::new(),
      })
      .to_record()
      .unwrap(),
      WmfRecordData::CreateBrushIndirect(WmfLogBrushObject {
        brush_style: WmfBrushStyle::Solid.raw(),
        color_ref: ColorRef {
          red: 255,
          ..Default::default()
        },
        brush_hatch: 0,
      })
      .to_record()
      .unwrap(),
      WmfRecordData::SelectObject(WmfObjectIndexRecord { index: 0 })
        .to_record()
        .unwrap(),
      WmfRecordData::SelectObject(WmfObjectIndexRecord { index: 1 })
        .to_record()
        .unwrap(),
      WmfRecordData::Polygon(WmfPolyPointsRecord {
        points: points.clone(),
      })
      .to_record()
      .unwrap(),
      WmfRecordData::Polyline(WmfPolyPointsRecord { points })
        .to_record()
        .unwrap(),
      WmfRecordData::Eof(WmfEofRecord::default())
        .to_record()
        .unwrap(),
    ];
    source
  }

  fn word_wmf_selected_text() -> crate::wmf::WmfMetafile {
    use crate::wmf::*;
    let mut source = word_wmf_primitives(WmfPenLineStyle::Solid.raw(), 1);
    let mut header = WmfPlaceableHeader {
      key: PLACEABLE_KEY,
      handle: 0,
      left: 0,
      top: 0,
      right: 10,
      bottom: 20,
      inch: 1440,
      reserved: 0,
      checksum: 0,
    };
    header.refresh_checksum();
    source.placeable_header = Some(header);
    source.header.number_of_objects = 4;
    source.records.pop();
    let font = |name: &[u8], height, char_set| {
      let mut face_name = [0; 32];
      face_name[..name.len()].copy_from_slice(name);
      WmfRecordData::CreateFontIndirect(WmfFontObject {
        height,
        width: 0,
        escapement: 0,
        orientation: 0,
        weight: 400,
        italic: 0,
        underline: 0,
        strike_out: 0,
        char_set,
        out_precision: 4,
        clip_precision: 0,
        quality: 0,
        pitch_and_family: 0,
        face_name,
        face_name_bytes: 32,
      })
      .to_record()
      .unwrap()
    };
    source.records.extend([
      font(b"Arial", -12, 0),
      WmfRecordData::SelectObject(WmfObjectIndexRecord { index: 2 })
        .to_record()
        .unwrap(),
      WmfRecordData::SetBkMode(WmfU16Record {
        value: 1,
        reserved: Vec::new(),
      })
      .to_record()
      .unwrap(),
      WmfRecordData::SetTextAlign(WmfU16Record {
        value: 24,
        reserved: Vec::new(),
      })
      .to_record()
      .unwrap(),
      WmfRecordData::TextOut(WmfTextOutRecord {
        string: b"A".to_vec(),
        string_padding: vec![0],
        x_start: 2,
        y_start: 7,
      })
      .to_record()
      .unwrap(),
      font(b"Symbol", -8, 2),
      WmfRecordData::SelectObject(WmfObjectIndexRecord { index: 3 })
        .to_record()
        .unwrap(),
      WmfRecordData::ExtTextOut(WmfExtTextOutRecord {
        x: 6,
        y: 9,
        string_length: 2,
        options: WmfExtTextOutOptions::empty(),
        rectangle: None,
        string: b"BB".to_vec(),
        string_padding: Vec::new(),
        dx: vec![1, 2],
        trailing_data: Vec::new(),
      })
      .to_record()
      .unwrap(),
      WmfRecordData::Eof(WmfEofRecord::default())
        .to_record()
        .unwrap(),
    ]);
    source
  }

  #[test]
  fn word_wmf_paths_and_dib_retain_paint_order_before_colored_text() {
    use crate::wmf::*;
    let mut source = word_wmf_selected_text();
    let bitmap = WmfRecordData::StretchDib(WmfStretchDibRecord {
      raster_operation: 0x00cc_0020,
      color_usage: 0,
      src_width: 1,
      src_height: 1,
      x_src: 0,
      y_src: 0,
      dest_width: -4,
      dest_height: 8,
      x_dest: 9,
      y_dest: 2,
      // A BITMAPCOREHEADER and one padded 24-bit BGR scanline.
      dib: vec![12, 0, 0, 0, 1, 0, 1, 0, 1, 0, 24, 0, 23, 19, 7, 0],
    })
    .to_record()
    .unwrap();
    source.records.insert(6, bitmap.clone());
    let color = ColorRef {
      red: 255,
      green: 204,
      blue: 102,
      reserved: 2,
    };
    let set_color = WmfRecordData::SetTextColor(WmfColorRecord { color })
      .to_record()
      .unwrap();
    source
      .records
      .insert(12, WmfRecordData::SaveDc.to_record().unwrap());
    source.records.insert(13, set_color);
    source.records.insert(
      15,
      WmfRecordData::RestoreDc(WmfI16Record { value: -1 })
        .to_record()
        .unwrap(),
    );
    let options = RenderOptions {
      word_emf_vector_pens: true,
      suppress_text: true,
      ..Default::default()
    };
    let bytes = source.to_bytes().unwrap();
    assert!(word_wmf_text_can_be_lifted(&bytes, options));
    let layers = extract_word_wmf_picture_layers(&bytes, options)
      .unwrap()
      .unwrap();
    let [
      MetafilePictureLayer::Drawing(before),
      MetafilePictureLayer::Bitmap(picture),
      MetafilePictureLayer::Drawing(after),
    ] = layers.as_slice()
    else {
      panic!("unexpected layer order");
    };
    assert_eq!(before.operations.len(), 2);
    assert_eq!(after.operations.len(), 1);
    assert_eq!(picture.source_size, [1, 1]);
    assert!(picture.bitmap.flip_horizontal);
    assert!(!picture.bitmap.flip_vertical);
    assert_eq!(
      image::load_from_memory(&picture.bitmap.data)
        .unwrap()
        .to_rgb8()
        .as_raw(),
      &[7, 19, 23]
    );
    let runs = super::super::extract_metafile_text_runs(&bytes, Some("image/wmf"));
    assert_eq!(
      runs.iter().map(|r| r.color).collect::<Vec<_>>(),
      [[255, 204, 102], [0, 0, 0]]
    );
    assert!(
      extract_word_wmf_picture_layers(
        &bytes,
        RenderOptions {
          suppress_text: false,
          ..options
        }
      )
      .unwrap()
      .is_none()
    );
    source.records.insert(source.records.len() - 1, bitmap);
    let occluded = source.to_bytes().unwrap();
    assert!(!word_wmf_text_can_be_lifted(&occluded, options));
    assert!(
      extract_word_wmf_picture_layers(&occluded, options)
        .unwrap()
        .is_none()
    );
  }

  #[test]
  fn word_wmf_selected_font_text_preserves_complete_graphics_prefix() {
    let source = word_wmf_selected_text();
    let bytes = source.to_bytes().unwrap();
    let options = RenderOptions {
      word_emf_vector_pens: true,
      suppress_text: true,
      ..Default::default()
    };
    assert!(super::super::metafile_text_can_be_lifted_with_options(
      &bytes,
      Some("image/wmf"),
      options
    ));
    assert!(!super::super::metafile_text_can_be_lifted(
      &bytes,
      Some("image/wmf")
    ));
    let expected = extract_metafile_vector_drawing_with_options(
      &word_wmf_primitives(WmfPenLineStyle::Solid.raw(), 1)
        .to_bytes()
        .unwrap(),
      Some("image/wmf"),
      options,
    )
    .unwrap()
    .unwrap();
    let drawing = extract_metafile_vector_drawing_with_options(&bytes, Some("image/wmf"), options)
      .unwrap()
      .unwrap();
    assert_eq!(drawing, expected);
    let runs = super::super::extract_metafile_text_runs(&bytes, Some("image/wmf"));
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].font_family.as_deref(), Some("Arial"));
    assert_eq!(runs[1].font_family.as_deref(), Some("Symbol"));
    assert!(runs[0].font_size.unwrap() > runs[1].font_size.unwrap());
    assert_eq!(runs[1].advances.as_ref().unwrap().len(), 2);
  }

  #[test]
  fn word_wmf_selected_text_accepts_the_defined_font_mapper_flags() {
    use crate::wmf::*;
    let options = RenderOptions {
      word_emf_vector_pens: true,
      ..Default::default()
    };
    for (value, admitted) in [(0, true), (1, true), (2, false), (3, false)] {
      let mut source = word_wmf_selected_text();
      source.records.insert(
        0,
        WmfRecordData::SetMapperFlags(WmfU32Record { value })
          .to_record()
          .unwrap(),
      );
      assert_eq!(
        word_wmf_text_can_be_lifted(&source.to_bytes().unwrap(), options),
        admitted
      );
    }
  }

  #[test]
  fn word_wmf_selected_text_decodes_dbcs_with_natural_advances() {
    use crate::wmf::*;
    let options = RenderOptions {
      word_emf_vector_pens: true,
      ..Default::default()
    };
    let mut source = word_wmf_selected_text();
    let WmfRecordData::CreateFontIndirect(mut font) = source.records[7].parse_data().unwrap()
    else {
      panic!("expected a logical font");
    };
    font.char_set = 134;
    source.records[7] = WmfRecordData::CreateFontIndirect(font).to_record().unwrap();
    let mut text = WmfExtTextOutRecord {
      x: 2,
      y: 7,
      string_length: 2,
      options: WmfExtTextOutOptions::empty(),
      rectangle: None,
      string: vec![0xd6, 0xd0],
      string_padding: Vec::new(),
      dx: Vec::new(),
      trailing_data: Vec::new(),
    };
    source.records[11] = WmfRecordData::ExtTextOut(text.clone()).to_record().unwrap();
    source.records.insert(
      12,
      WmfRecordData::Escape(WmfEscapeRecord {
        escape_function: WmfMetafileEscape::GetTechnology.raw(),
        escape_data: Vec::new(),
        padding: Vec::new(),
      })
      .to_record()
      .unwrap(),
    );
    let bytes = source.to_bytes().unwrap();
    assert!(word_wmf_text_can_be_lifted(&bytes, options));
    let runs = super::super::extract_metafile_text_runs(&bytes, Some("image/wmf"));
    assert_eq!(runs[0].text, "中");
    text.dx = vec![1, 2];
    source.records[11] = WmfRecordData::ExtTextOut(text.clone()).to_record().unwrap();
    assert!(!word_wmf_text_can_be_lifted(
      &source.to_bytes().unwrap(),
      options
    ));
    text.dx.clear();
    text.string = vec![0x81, 0x30];
    source.records[11] = WmfRecordData::ExtTextOut(text).to_record().unwrap();
    assert!(!word_wmf_text_can_be_lifted(
      &source.to_bytes().unwrap(),
      options
    ));
  }

  #[test]
  fn word_wmf_selected_text_keeps_dc_and_reused_font_handle_owners() {
    use crate::wmf::*;
    let mut source = word_wmf_selected_text();
    let reused_font = source.records[7].clone();
    // The second font is already selected, so the first font can be deleted
    // before its slot2 is reused. The saved DC independently owns font3.
    source.records.splice(
      14..14,
      [
        WmfRecordData::DeleteObject(WmfObjectIndexRecord { index: 2 })
          .to_record()
          .unwrap(),
        WmfRecordData::SaveDc.to_record().unwrap(),
        WmfRecordData::SetBkMode(WmfU16Record {
          value: 2,
          reserved: Vec::new(),
        })
        .to_record()
        .unwrap(),
        WmfRecordData::RestoreDc(WmfI16Record { value: -1 })
          .to_record()
          .unwrap(),
        reused_font,
        WmfRecordData::SelectObject(WmfObjectIndexRecord { index: 2 })
          .to_record()
          .unwrap(),
      ],
    );
    assert!(word_wmf_text_can_be_lifted(
      &source.to_bytes().unwrap(),
      RenderOptions {
        word_emf_vector_pens: true,
        ..Default::default()
      }
    ));
  }

  #[test]
  fn word_wmf_selected_text_rejects_occlusion_and_unrepresented_text_modes() {
    use crate::wmf::*;
    let options = RenderOptions {
      word_emf_vector_pens: true,
      suppress_text: true,
      ..Default::default()
    };
    let check = |source: WmfMetafile| {
      let bytes = source.to_bytes().unwrap();
      assert!(!word_wmf_text_can_be_lifted(&bytes, options));
      assert!(
        extract_metafile_vector_drawing_with_options(&bytes, Some("image/wmf"), options)
          .unwrap()
          .is_none()
      );
    };
    let mut later_paint = word_wmf_selected_text();
    later_paint.records.insert(
      later_paint.records.len() - 1,
      WmfRecordData::LineTo(WmfPointRecord { x: 8, y: 16 })
        .to_record()
        .unwrap(),
    );
    check(later_paint);
    for (index, replacement) in [
      (
        9,
        WmfRecordData::SetBkMode(WmfU16Record {
          value: 2,
          reserved: Vec::new(),
        }),
      ),
      (
        10,
        WmfRecordData::SetTextAlign(WmfU16Record {
          value: 25,
          reserved: Vec::new(),
        }),
      ),
      (
        10,
        WmfRecordData::SetTextColor(WmfColorRecord {
          color: ColorRef {
            red: 255,
            ..Default::default()
          },
        }),
      ),
    ] {
      let mut source = word_wmf_selected_text();
      source.records[index] = replacement.to_record().unwrap();
      check(source);
    }
    for kind in 0..6 {
      let mut source = word_wmf_selected_text();
      let WmfRecordData::CreateFontIndirect(mut font) = source.records[7].parse_data().unwrap()
      else {
        panic!("selected font")
      };
      match kind {
        0 => font.height = 4,
        1 => font.width = 1,
        2 => font.underline = 1,
        3 => font.escapement = 900,
        4 => font.height = -4,
        _ => font.weight = 500,
      }
      source.records[7] = WmfRecordData::CreateFontIndirect(font).to_record().unwrap();
      check(source);
    }
  }

  #[test]
  fn word_wmf_vector_drawing_keeps_fill_stroke_order_and_logical_pen_axes() {
    for width in [0, 1, 2, -2] {
      let source = word_wmf_primitives(WmfPenLineStyle::Solid.raw(), width);
      let bytes = source.to_bytes().unwrap();
      // Standalone playback retains its strict fallback. The observed Word
      // conversion is selected explicitly, independently of raster GDI.
      assert!(
        extract_metafile_vector_drawing_with_options(
          &bytes,
          Some("image/wmf"),
          RenderOptions::default()
        )
        .unwrap()
        .is_none()
      );
      let drawing = extract_metafile_vector_drawing_with_options(
        &bytes,
        Some("image/wmf"),
        RenderOptions {
          word_emf_vector_pens: true,
          ..Default::default()
        },
      )
      .unwrap()
      .unwrap();
      assert_eq!(drawing.operations.len(), 3);
      let MetafileVectorDraw::Fill { fill, .. } = &drawing.operations[0] else {
        panic!("fill precedes its outline")
      };
      assert_eq!(fill.color, [255, 0, 0]);
      let expected_point = if width.unsigned_abs() <= 1 {
        MetafileVectorPoint {
          x: 1.5 / 9.0,
          y: 2.5 / 19.0,
        }
      } else {
        MetafileVectorPoint { x: 0.1, y: 0.1 }
      };
      assert_eq!(fill.subpaths[0][0], expected_point);
      for (operation, expected_closed) in drawing.operations[1..].iter().zip([true, false]) {
        let MetafileVectorDraw::Stroke {
          width: actual_width,
          line_cap,
          closed,
          subpaths,
          ..
        } = operation
        else {
          panic!("missing outline or polyline")
        };
        assert_eq!(*closed, expected_closed);
        assert_eq!(*line_cap, MetafileVectorLineCap::Flat);
        let nominal = f32::from(width.unsigned_abs().max(1));
        assert_eq!(*actual_width, [nominal / 10.0, nominal / 20.0]);
        assert_eq!(subpaths[0][0], fill.subpaths[0][0]);
      }
    }
  }

  #[test]
  fn wmf_vector_playback_ignores_reserved_relabs_without_changing_raw_roundtrip() {
    let source = word_wmf_primitives(WmfPenLineStyle::Null.raw(), 0);
    let expected =
      extract_metafile_vector_scene(&source.to_bytes().unwrap(), Some("image/wmf")).unwrap();
    for payload in [vec![], vec![1, 0], vec![0xAA, 0xBB, 0xCC, 0xDD]] {
      let mut variant = source.clone();
      variant.records.insert(
        0,
        crate::wmf::WmfRecord::new(crate::wmf::WmfRecordFunction::SetRelabs.raw(), payload),
      );
      let bytes = variant.to_bytes().unwrap();
      assert_eq!(
        crate::wmf::WmfMetafile::from_bytes(&bytes)
          .unwrap()
          .to_bytes()
          .unwrap(),
        bytes
      );
      assert_eq!(
        extract_metafile_vector_scene(&bytes, Some("image/wmf")).unwrap(),
        expected
      );
    }
  }

  #[test]
  fn word_wmf_line_to_keeps_narrow_excluded_endpoint_and_wide_flat_endpoint() {
    use crate::wmf::*;
    for width in [0, 1, 2] {
      let mut source = word_wmf_primitives(WmfPenLineStyle::Solid.raw(), width);
      source.records.splice(
        5..7,
        [
          WmfRecordData::MoveTo(WmfPointRecord { x: 1, y: 2 })
            .to_record()
            .unwrap(),
          WmfRecordData::LineTo(WmfPointRecord { x: 9, y: 2 })
            .to_record()
            .unwrap(),
        ],
      );
      let drawing = extract_metafile_vector_drawing_with_options(
        &source.to_bytes().unwrap(),
        Some("image/wmf"),
        RenderOptions {
          word_emf_vector_pens: true,
          ..Default::default()
        },
      )
      .unwrap()
      .unwrap();
      assert_eq!(drawing.operations.len(), 1);
      let MetafileVectorDraw::Stroke {
        subpaths, line_cap, ..
      } = &drawing.operations[0]
      else {
        panic!("missing line")
      };
      if width <= 1 {
        assert_eq!(*line_cap, MetafileVectorLineCap::Square);
        assert_eq!(
          subpaths[0][1],
          MetafileVectorPoint {
            x: 8.5 / 9.0,
            y: 2.5 / 19.0
          }
        );
      } else {
        assert_eq!(*line_cap, MetafileVectorLineCap::Flat);
        assert_eq!(subpaths[0][1], MetafileVectorPoint { x: 0.9, y: 0.1 });
      }
    }
  }

  #[test]
  fn word_wmf_vector_drawing_rejects_incomplete_text_pens_and_destination_ops() {
    use crate::wmf::*;
    let options = RenderOptions {
      word_emf_vector_pens: true,
      ..Default::default()
    };
    let mut source = word_wmf_primitives(WmfPenLineStyle::Solid.raw(), 0);
    let mut dashed = word_wmf_primitives(WmfPenLineStyle::Dash.raw(), 1);
    assert!(
      extract_metafile_vector_drawing_with_options(
        &dashed.to_bytes().unwrap(),
        Some("image/wmf"),
        options
      )
      .unwrap()
      .is_none()
    );
    dashed.records[1] = WmfRecord::new(
      WmfRecordFunction::CreatePenIndirect.raw(),
      vec![0, 1, 1, 0, 0, 0, 0, 0, 0, 0],
    );
    assert!(
      extract_metafile_vector_drawing_with_options(
        &dashed.to_bytes().unwrap(),
        Some("image/wmf"),
        options
      )
      .unwrap()
      .is_none()
    );
    source.records.insert(
      5,
      WmfRecordData::SetRop2(WmfU16Record {
        value: WmfBinaryRasterOperation::XorPen.raw(),
        reserved: Vec::new(),
      })
      .to_record()
      .unwrap(),
    );
    assert!(
      extract_metafile_vector_drawing_with_options(
        &source.to_bytes().unwrap(),
        Some("image/wmf"),
        options
      )
      .unwrap()
      .is_none()
    );
    source.records.remove(5);
    source.records.insert(
      7,
      WmfRecord::new(
        WmfRecordFunction::TextOut.raw(),
        vec![1, 0, b'X', 0, 2, 0, 1, 0],
      ),
    );
    assert!(
      extract_metafile_vector_drawing_with_options(
        &source.to_bytes().unwrap(),
        Some("image/wmf"),
        options
      )
      .unwrap()
      .is_none()
    );
  }

  #[test]
  fn word_wmf_vector_prefix_preserves_text_records_fonts_colours_and_current_position() {
    use crate::wmf::*;
    let options = RenderOptions {
      word_emf_vector_pens: true,
      ..Default::default()
    };
    let mut source = word_wmf_primitives(WmfPenLineStyle::Solid.raw(), 0);
    let line = WmfRecordData::LineTo(WmfPointRecord { x: 8, y: 16 })
      .to_record()
      .unwrap();
    source.records.insert(7, line);
    let text = WmfRecord::new(
      WmfRecordFunction::TextOut.raw(),
      vec![1, 0, b'X', 0, 2, 0, 1, 0],
    );
    let white = WmfRecordData::SetTextColor(WmfColorRecord {
      color: ColorRef {
        red: 255,
        green: 255,
        blue: 255,
        reserved: 0,
      },
    })
    .to_record()
    .unwrap();
    let update_cp = WmfRecordData::SetTextAlign(WmfU16Record {
      value: 1,
      reserved: Vec::new(),
    })
    .to_record()
    .unwrap();
    let expected_drawing = extract_metafile_vector_drawing_with_options(
      &source.to_bytes().unwrap(),
      Some("image/wmf"),
      options,
    )
    .unwrap()
    .unwrap();
    source
      .records
      .splice(8..8, [white.clone(), update_cp.clone(), text.clone()]);
    let bytes = source.to_bytes().unwrap();
    assert!(
      extract_metafile_vector_drawing_with_options(&bytes, Some("image/wmf"), options)
        .unwrap()
        .is_none()
    );
    let prefix = extract_metafile_vector_prefix_with_options(&bytes, Some("image/wmf"), options)
      .unwrap()
      .unwrap();
    assert_eq!(prefix.drawing, expected_drawing);
    let residual = WmfMetafile::from_bytes(&prefix.residual_metafile).unwrap();
    assert_eq!(
      residual.header.file_size_words,
      residual.computed_file_size_words().unwrap()
    );
    assert_eq!(
      residual.header.max_record_words,
      residual.computed_max_record_words().unwrap()
    );
    assert_eq!(
      &residual.records[residual.records.len() - 4..residual.records.len() - 1],
      &[white, update_cp, text]
    );
    assert!(residual.records.iter().all(|r| !matches!(
      r.normalized_function_kind(),
      Some(
        WmfRecordFunction::Polygon
          | WmfRecordFunction::PolyPolygon
          | WmfRecordFunction::Polyline
          | WmfRecordFunction::LineTo
      )
    )));
    assert!(residual.records.iter().any(|r| matches!(
      r.parse_data(),
      Ok(WmfRecordData::MoveTo(WmfPointRecord { x: 8, y: 16 }))
    )));
    assert_eq!(WmfMetafile::from_bytes(&bytes).unwrap(), source);
  }

  #[test]
  fn word_wmf_vector_prefix_rejects_interleaved_text_and_incomplete_graphics() {
    use crate::wmf::*;
    let options = RenderOptions {
      word_emf_vector_pens: true,
      ..Default::default()
    };
    let text = WmfRecord::new(
      WmfRecordFunction::TextOut.raw(),
      vec![1, 0, b'X', 0, 2, 0, 1, 0],
    );
    let source = word_wmf_primitives(WmfPenLineStyle::Solid.raw(), 0);
    assert!(
      extract_metafile_vector_prefix_with_options(
        &source.to_bytes().unwrap(),
        Some("image/wmf"),
        options
      )
      .unwrap()
      .is_none()
    );
    let mut interleaved = source.clone();
    interleaved.records.insert(6, text.clone());
    assert!(
      extract_metafile_vector_prefix_with_options(
        &interleaved.to_bytes().unwrap(),
        Some("image/wmf"),
        options
      )
      .unwrap()
      .is_none()
    );
    let mut dashed = word_wmf_primitives(WmfPenLineStyle::Dash.raw(), 0);
    dashed.records.insert(7, text.clone());
    assert!(
      extract_metafile_vector_prefix_with_options(
        &dashed.to_bytes().unwrap(),
        Some("image/wmf"),
        options
      )
      .unwrap()
      .is_none()
    );
    let mut destination = source;
    destination.records.insert(7, text);
    destination.records.insert(
      8,
      WmfRecordData::SetRop2(WmfU16Record {
        value: WmfBinaryRasterOperation::XorPen.raw(),
        reserved: Vec::new(),
      })
      .to_record()
      .unwrap(),
    );
    assert!(
      extract_metafile_vector_prefix_with_options(
        &destination.to_bytes().unwrap(),
        Some("image/wmf"),
        options
      )
      .unwrap()
      .is_none()
    );
    assert!(
      extract_metafile_vector_prefix_with_options(
        &destination.to_bytes().unwrap(),
        Some("image/wmf"),
        RenderOptions::default()
      )
      .unwrap()
      .is_none()
    );
  }

  fn wmf_mapping(origin: [i16; 2], extent: [i16; 2], inch: u16, office: bool) -> VectorMapping {
    // A complete empty WMF; the placeable header is independent of paint.
    let bytes = [
      1, 0, 9, 0, 0, 3, 12, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0,
    ];
    let mut metafile = WmfMetafileRef::from_bytes(&bytes).unwrap();
    metafile.placeable_header = Some(
      WmfPlaceableHeader {
        key: crate::wmf::PLACEABLE_KEY,
        handle: 0,
        left: origin[0],
        top: origin[1],
        right: origin[0] + extent[0],
        bottom: origin[1] + extent[1],
        inch,
        reserved: 0,
        checksum: 0,
      }
      .with_computed_checksum(),
    );
    VectorMapping::wmf(
      &metafile,
      RenderOptions {
        wmf_conversion_profile: office.then_some(WmfConversionProfile {
          dpi: [140, 140],
          device_pixels: [3840, 2160],
          device_millimeters: [697, 392],
          initial_system_font: None,
        }),
        ..RenderOptions::default()
      },
    )
  }

  #[test]
  fn wmf_vector_scene_conversion_preserves_native_frame_phase() {
    // Actual GdipConvertToEmfPlus frame/viewport observations. The final
    // translated case also matches Word Range.EnhMetaFileBits' image object.
    for (origin, extent, inch, frame, viewport, viewport_origin) in [
      ([0, 0], [50, 30], 94, [0, 0, 1333, 792], [74, 45], [0, 0]),
      (
        [17, 34],
        [50, 30],
        94,
        [459, 919, 1792, 1711],
        [74, 45],
        [25, 51],
      ),
      (
        [-17, -34],
        [50, 30],
        94,
        [-459, -919, 874, -126],
        [74, 45],
        [-25, -51],
      ),
      (
        [17, 34],
        [4069, 2441],
        1000,
        [43, 86, 10360, 6268],
        [569, 342],
        [2, 5],
      ),
      (
        [792, 516],
        [4069, 3431],
        1000,
        [2012, 1311, 12329, 10007],
        [569, 480],
        [111, 72],
      ),
    ] {
      let mapping = wmf_mapping(origin, extent, inch, true);
      assert_eq!(
        [mapping.viewport_org_x, mapping.viewport_org_y],
        viewport_origin
      );
      assert_eq!([mapping.viewport_ext_x, mapping.viewport_ext_y], viewport);
      for (axis, start, size) in [
        (0, mapping.playback_origin_x, mapping.surface_width),
        (1, mapping.playback_origin_y, mapping.surface_height),
      ] {
        let mm100_per_pixel = [69700.0 / 3840.0, 39200.0 / 2160.0][axis];
        assert!((f64::from(start) * mm100_per_pixel - f64::from(frame[axis])).abs() < 0.001);
        assert!(
          ((f64::from(start) + f64::from(size) - 1.0) * mm100_per_pixel
            - f64::from(frame[axis + 2]))
          .abs()
            < 0.001
        );
      }
      let logical = wmf_mapping(origin, extent, inch, false);
      assert_eq!(logical.surface_width, f32::from(extent[0]));
      assert_eq!(logical.surface_height, f32::from(extent[1]));
      assert_eq!(logical.playback_origin_x, 0.0);
      assert_eq!(logical.viewport_org_x, 0);
    }
  }

  #[test]
  fn wmf_vector_scene_conversion_small_surface_boundary_is_joint() {
    // Both axes must reach 20 reference pixels before physical scaling is
    // selected. These frame-left values distinguish even unit-scale paths.
    for (extent, inch, frame_left) in [
      ([20, 20], 139, 311),
      ([21, 20], 139, 311),
      ([21, 20], 140, 309),
      ([20, 21], 140, 309),
      ([21, 22], 140, 308),
      ([13, 7], 94, 309),
      ([50, 30], 1000, 309),
    ] {
      let mapping = wmf_mapping([17, 34], extent, inch, true);
      assert!(
        (f64::from(mapping.playback_origin_x) * (69700.0 / 3840.0) - f64::from(frame_left)).abs()
          < 0.001
      );
      assert_eq!([mapping.viewport_org_x, mapping.viewport_org_y], [17, 34]);
      assert_eq!(mapping.viewport_ext_x, i32::from(extent[0]));
      assert_eq!(mapping.viewport_ext_y, i32::from(extent[1]));
    }
  }
}
