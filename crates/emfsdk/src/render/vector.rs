//! Strict vector lifting for classic solid-fill metafile streams.
//!
//! This interpreter is intentionally all-or-nothing. It returns a scene only
//! when every record that can paint is representable as a solid closed path;
//! callers retain the complete raster replay for every other metafile. That
//! keeps vector output faithful without treating a partially understood record
//! stream as complete.

use std::collections::HashMap;

use crate::common::SdkEnumValue;
use crate::emf::{
  EmfMetafileRef, EmfRecordData, EmrComment, EmrMapMode, EmrModifyWorldTransformMode,
  EmrPolygonFillMode, EmrPublicComment, EmrRegionMode, EmrStockObject,
};
use crate::types::{ColorRef, PointL, PointS, XForm};
use crate::wmf::{
  WmfBrushStyle, WmfMapMode, WmfMetafileRef, WmfPenLineStyle, WmfPolyFillMode, WmfRecordData,
};

use super::{
  EmfPoint, EmfTransform, RenderError, RenderOptions, RenderResult, emf_gdiplus_playback_geometry,
  is_emf, looks_like_metafile, nonzero_mapping_extent, scale_wmf_extent, wmf_initial_window,
};

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

/// Extracts a vector scene using the metafile's self-contained playback
/// geometry.
pub fn extract_metafile_vector_scene(
  data: &[u8],
  content_type: Option<&str>,
) -> RenderResult<Option<MetafileVectorScene>> {
  extract_metafile_vector_scene_with_options(data, content_type, RenderOptions::default())
}

/// Extracts a vector scene when the entire visible stream is a supported
/// classic solid-fill subset.
///
/// `Ok(None)` is a normal completeness fallback, not a malformed-file error.
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
    extract_emf_scene(data)
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
  Visible,
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
      world_transform: EmfTransform::identity(),
    }
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
}

#[derive(Clone, Copy, Debug)]
struct VectorGraphicsState {
  mapping: VectorMapping,
  brush: VectorBrush,
  pen: VectorPen,
  fill_rule: MetafileVectorFillRule,
  current_pos: EmfPoint,
}

struct VectorInterpreter {
  graphics: VectorGraphicsState,
  saved: Vec<VectorGraphicsState>,
  fills: Vec<MetafileVectorFill>,
}

impl VectorInterpreter {
  fn new(mapping: VectorMapping) -> Self {
    Self {
      graphics: VectorGraphicsState {
        mapping,
        // Win32 initializes a memory DC with WHITE_BRUSH and BLACK_PEN.
        brush: VectorBrush::Solid([255; 3]),
        pen: VectorPen::Visible,
        fill_rule: MetafileVectorFillRule::Alternate,
        current_pos: EmfPoint { x: 0, y: 0 },
      },
      saved: Vec::new(),
      fills: Vec::new(),
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
    if self.graphics.pen != VectorPen::Null {
      return false;
    }
    let color = match self.graphics.brush {
      VectorBrush::Solid(color) => color,
      VectorBrush::Null => return true,
      VectorBrush::Unsupported => return false,
    };
    let mut subpaths = Vec::with_capacity(polygons.len());
    for polygon in polygons {
      if polygon.len() < 2 {
        continue;
      }
      let mut mapped = Vec::with_capacity(polygon.len());
      for point in polygon {
        let Some(point) = self.graphics.mapping.map_point(*point) else {
          return false;
        };
        mapped.push(point);
      }
      subpaths.push(mapped);
    }
    if !subpaths.is_empty() {
      self.fills.push(MetafileVectorFill {
        subpaths,
        color,
        fill_rule: self.graphics.fill_rule,
      });
    }
    true
  }

  fn finish(self) -> MetafileVectorScene {
    MetafileVectorScene { fills: self.fills }
  }
}

fn extract_emf_scene(data: &[u8]) -> Result<Option<MetafileVectorScene>, String> {
  let metafile = EmfMetafileRef::from_bytes(data).map_err(|error| error.to_string())?;
  let mut interpreter = VectorInterpreter::new(VectorMapping::emf(data)?);
  let mut objects = HashMap::<u32, VectorObject>::new();
  let mut saw_eof = false;

  for record in metafile.records() {
    let record = record.parse_data().map_err(|error| error.to_string())?;
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
      EmfRecordData::SetWindowExtEx(value) => {
        interpreter.graphics.mapping.window_ext_x = nonzero_mapping_extent(value.size.cx);
        interpreter.graphics.mapping.window_ext_y = nonzero_mapping_extent(value.size.cy);
        true
      }
      EmfRecordData::SetViewportOrgEx(value) => {
        interpreter.graphics.mapping.viewport_org_x = value.origin.x;
        interpreter.graphics.mapping.viewport_org_y = value.origin.y;
        true
      }
      EmfRecordData::SetViewportExtEx(value) => {
        interpreter.graphics.mapping.viewport_ext_x = nonzero_mapping_extent(value.size.cx);
        interpreter.graphics.mapping.viewport_ext_y = nonzero_mapping_extent(value.size.cy);
        true
      }
      EmfRecordData::ScaleWindowExtEx(value) => {
        let Some(x) = scale_i32_extent(
          interpreter.graphics.mapping.window_ext_x,
          value.x_num,
          value.x_denom,
        ) else {
          return Ok(None);
        };
        let Some(y) = scale_i32_extent(
          interpreter.graphics.mapping.window_ext_y,
          value.y_num,
          value.y_denom,
        ) else {
          return Ok(None);
        };
        interpreter.graphics.mapping.window_ext_x = x;
        interpreter.graphics.mapping.window_ext_y = y;
        true
      }
      EmfRecordData::ScaleViewportExtEx(value) => {
        let Some(x) = scale_i32_extent(
          interpreter.graphics.mapping.viewport_ext_x,
          value.x_num,
          value.x_denom,
        ) else {
          return Ok(None);
        };
        let Some(y) = scale_i32_extent(
          interpreter.graphics.mapping.viewport_ext_y,
          value.y_num,
          value.y_denom,
        ) else {
          return Ok(None);
        };
        interpreter.graphics.mapping.viewport_ext_x = x;
        interpreter.graphics.mapping.viewport_ext_y = y;
        true
      }
      EmfRecordData::SetMapMode(value) => matches!(
        value.map_mode_kind(),
        Some(EmrMapMode::Text | EmrMapMode::Anisotropic)
      ),
      EmfRecordData::SetPolyFillMode(value) => {
        let Some(rule) = emf_fill_rule(value.polygon_fill_mode_kind()) else {
          return Ok(None);
        };
        interpreter.graphics.fill_rule = rule;
        true
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
        objects.insert(
          value.object_index,
          VectorObject::Pen(
            if value.pen_line_style_kind() == Some(crate::emf::EmrPenLineStyle::Null) {
              VectorPen::Null
            } else {
              VectorPen::Visible
            },
          ),
        );
        true
      }
      EmfRecordData::ExtCreatePen(value) => {
        objects.insert(
          value.object_index,
          VectorObject::Pen(
            if value.pen_line_style_kind() == Some(crate::emf::EmrPenLineStyle::Null) {
              VectorPen::Null
            } else {
              VectorPen::Visible
            },
          ),
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
        let supported = interpreter.graphics.pen == VectorPen::Null;
        interpreter.graphics.current_pos = point_l(value.point);
        supported
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
      EmfRecordData::ExtSelectClipRgn(value) => {
        value.region_data.is_empty() && value.region_mode_kind() == Some(EmrRegionMode::Copy)
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
      | EmfRecordData::SetRop2(_)
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

  Ok(saw_eof.then(|| interpreter.finish()))
}

fn extract_wmf_scene(
  data: &[u8],
  options: RenderOptions,
) -> Result<Option<MetafileVectorScene>, String> {
  let metafile = WmfMetafileRef::from_bytes(data).map_err(|error| error.to_string())?;
  let mut interpreter = VectorInterpreter::new(VectorMapping::wmf(&metafile, options));
  let mut objects = vec![None; metafile.header.number_of_objects as usize];
  let mut saw_eof = false;

  for record in metafile.records() {
    let record = record.parse_data().map_err(|error| error.to_string())?;
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
            } else {
              VectorPen::Visible
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
        let supported = interpreter.graphics.pen == VectorPen::Null;
        interpreter.graphics.current_pos = EmfPoint {
          x: i32::from(value.x),
          y: i32::from(value.y),
        };
        supported
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

  Ok(saw_eof.then(|| interpreter.finish()))
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
    Some(EmrStockObject::WhitePen | EmrStockObject::BlackPen) => {
      VectorObject::Pen(VectorPen::Visible)
    }
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

fn insert_wmf_object(objects: &mut Vec<Option<VectorObject>>, object: VectorObject) {
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

fn scale_i32_extent(extent: i32, numerator: i32, denominator: i32) -> Option<i32> {
  if denominator == 0 {
    return None;
  }
  let value = (i64::from(extent) * i64::from(numerator)) / i64::from(denominator);
  Some(nonzero_mapping_extent(
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
  ))
}
