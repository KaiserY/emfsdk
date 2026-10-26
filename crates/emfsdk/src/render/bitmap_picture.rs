//! Complete EMF+ bitmap pictures, without rasterizing their playback surface.

use std::collections::HashMap;
use std::io::Cursor;

use crate::emf::{EmfMetafileRef, EmfRecordData, EmrComment, EmrPublicComment};
use crate::emfplus::{
  EmfPlusBitmapPayload, EmfPlusImageData, EmfPlusObjectAssembler, EmfPlusObjectData,
  EmfPlusPointData, EmfPlusRecordData,
};
use crate::types::{PointF, RectF, XForm};

use super::{MetafileBitmapLayer, RenderOptions, emf_gdiplus_playback_geometry, is_emf};

/// A complete bitmap picture and the sampling state active at its draw record.
#[derive(Clone, Debug, PartialEq)]
pub struct MetafileBitmapPicture {
  pub bitmap: MetafileBitmapLayer,
  pub source_size: [u32; 2],
  pub half_pixel_offset: bool,
}

/// Extract a complete EMF+ picture containing one unmodified compressed bitmap.
///
/// The returned rectangle is normalized to the GDI+ metafile playback surface.
/// Source pixels remain intact, including their alpha channel. This is not an
/// independent-layer extractor: any other visible paint, clip, effect, source
/// crop or unsupported transform rejects the entire picture. In EMF+ Dual,
/// classic fallback records are ignored only outside `GetDC` intervals.
pub fn extract_metafile_bitmap_picture_with_options(
  data: &[u8],
  options: RenderOptions,
) -> Option<MetafileBitmapPicture> {
  if !is_emf(data)
    || options.suppress_bitmap_layers
    || options.monochrome_dib_palette_override.is_some()
  {
    return None;
  }
  let file = EmfMetafileRef::from_bytes(data).ok()?;
  let surface = emf_gdiplus_playback_geometry(data).ok()?;
  let mut header = false;
  let mut eof = false;
  let mut get_dc = false;
  let mut pixel_units = false;
  let mut half_pixel_offset = false;
  let mut assembler = EmfPlusObjectAssembler::default();
  let mut objects = HashMap::new();
  let mut picture = None;
  for record in file.records() {
    match record.parse_data().ok()? {
      EmfRecordData::Header(_) | EmfRecordData::Eof(_) => {}
      EmfRecordData::Comment(EmrComment::EmfPlus { records, .. }) => {
        for record in records {
          if eof {
            return None;
          }
          get_dc = false;
          match record.as_ref().parse_data_relaxed().ok()? {
            EmfPlusRecordData::Header(_) if !header => header = true,
            _ if !header => return None,
            EmfPlusRecordData::Eof => eof = true,
            EmfPlusRecordData::GetDc => get_dc = true,
            EmfPlusRecordData::Object(fragment) => {
              if let Some(object) = assembler.push_relaxed(fragment).ok()? {
                objects.insert(object.object_id, object.parse_object_data_relaxed().ok()?);
              }
            }
            EmfPlusRecordData::SetPageTransform(value) => {
              pixel_units = record.flags & 0xff == 2 && value.page_scale == 1.0;
            }
            EmfPlusRecordData::SetWorldTransform(value)
              if value
                == (XForm {
                  m11: 1.0,
                  m22: 1.0,
                  ..XForm::default()
                }) => {}
            EmfPlusRecordData::SetPixelOffsetMode(value) => {
              half_pixel_offset = match value.value {
                0 | 1 | 3 => false,
                2 | 4 => true,
                _ => return None,
              };
            }
            EmfPlusRecordData::ResetWorldTransform
            | EmfPlusRecordData::SetAntiAliasMode(_)
            | EmfPlusRecordData::SetInterpolationMode(_)
            | EmfPlusRecordData::SetCompositingQuality(_) => {}
            EmfPlusRecordData::DrawImagePoints(value)
              if !value.apply_effect && value.src_unit == 2 && pixel_units =>
            {
              let points = image_points(&value.points)?;
              if points[0].y != points[1].y || points[0].x != points[2].x {
                return None;
              }
              let rect = RectF {
                x: points[0].x,
                y: points[0].y,
                width: points[1].x - points[0].x,
                height: points[2].y - points[0].y,
              };
              let mut layer = bitmap_layer(
                &objects,
                value.image_id,
                value.image_attributes_id,
                value.src_rect,
                rect,
                surface,
              )?;
              layer.half_pixel_offset = half_pixel_offset;
              if picture.replace(layer).is_some() {
                return None;
              }
            }
            _ => return None,
          }
        }
      }
      EmfRecordData::Comment(EmrComment::Public {
        comment: EmrPublicComment::BeginGroup(_) | EmrPublicComment::EndGroup,
        ..
      }) => {}
      // [MS-EMFPLUS] GetDC ends at the next EMF+ record. Do not mistake a
      // Dual stream's redundant classic bitmap for a second visible picture.
      _ if header && !get_dc => {}
      _ => return None,
    }
  }
  assembler.finish().ok()?;
  (header && eof).then_some(picture).flatten()
}

fn image_points(points: &EmfPlusPointData) -> Option<[PointF; 3]> {
  if points.len() != 3 {
    return None;
  }
  let mut result = [PointF { x: 0.0, y: 0.0 }; 3];
  match points {
    EmfPlusPointData::Float(points) => result.copy_from_slice(points),
    EmfPlusPointData::Compressed(points) => {
      for (out, point) in result.iter_mut().zip(points) {
        *out = PointF {
          x: f32::from(point.x),
          y: f32::from(point.y),
        };
      }
    }
    EmfPlusPointData::Relative(_) => return None,
  }
  Some(result)
}

fn bitmap_layer(
  objects: &HashMap<u8, EmfPlusObjectData>,
  image_id: u8,
  attributes_id: u32,
  source: RectF,
  destination: RectF,
  surface: super::EmfPlaybackGeometry,
) -> Option<MetafileBitmapPicture> {
  let EmfPlusObjectData::Image(image) = objects.get(&image_id)? else {
    return None;
  };
  let EmfPlusImageData::Bitmap(bitmap) = image.parse_image_data().ok()? else {
    return None;
  };
  let EmfPlusBitmapPayload::Compressed(bitmap) = bitmap.parse_bitmap_data().ok()? else {
    return None;
  };
  if attributes_id != u32::MAX {
    let EmfPlusObjectData::ImageAttributes(attributes) =
      objects.get(&u8::try_from(attributes_id).ok()?)?
    else {
      return None;
    };
    // Full-source pictures need no edge tiling. Keep clamp/color policies
    // outside this path rather than discarding a potentially visible border.
    if attributes.wrap_mode > 3 || attributes.object_clamp != 0 {
      return None;
    }
  }
  let format = image::guess_format(&bitmap.compressed_image_data).ok()?;
  let content_type = match format {
    image::ImageFormat::Png => "image/png",
    image::ImageFormat::Jpeg => "image/jpeg",
    _ => return None,
  };
  let (width, height) =
    image::ImageReader::with_format(Cursor::new(&bitmap.compressed_image_data), format)
      .into_dimensions()
      .ok()?;
  if width == 0
    || height == 0
    || source
      != (RectF {
        x: 0.0,
        y: 0.0,
        width: width as f32,
        height: height as f32,
      })
    || ![
      destination.x,
      destination.y,
      destination.width,
      destination.height,
    ]
    .into_iter()
    .all(f32::is_finite)
    || destination.width <= 0.0
    || destination.height <= 0.0
  {
    return None;
  }
  Some(MetafileBitmapPicture {
    source_size: [width, height],
    half_pixel_offset: false,
    bitmap: MetafileBitmapLayer {
      data: bitmap.compressed_image_data,
      content_type,
      x: (destination.x - surface.origin_x) / surface.width as f32,
      y: (destination.y - surface.origin_y) / surface.height as f32,
      width: destination.width / surface.width as f32,
      height: destination.height / surface.height as f32,
      flip_horizontal: false,
      flip_vertical: false,
    },
  })
}
