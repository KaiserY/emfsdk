use emfsdk::{
    BitmapCompression, DeviceIndependentBitmap, EmfMetafile, EmfRecordData, EmfRecordType, Error,
    Format, detect_format,
};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedMetafile {
    pub data: Vec<u8>,
    pub content_type: &'static str,
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("metafile parse failed")]
    Sdk(#[from] Error),
    #[error("multiple EMF bitmap records are not supported yet")]
    MultipleBitmapRecords,
    #[error("unsupported DIB compression {compression:?}")]
    UnsupportedDibCompression {
        compression: Option<BitmapCompression>,
    },
}

pub type Result<T> = std::result::Result<T, RenderError>;

pub fn decode_metafile_as_raster(
    data: &[u8],
    content_type: Option<&str>,
) -> Result<Option<DecodedMetafile>> {
    if !looks_like_metafile(data, content_type) {
        return Ok(None);
    }

    match detect_format(data).or_else(|| content_type_format(content_type)) {
        Some(Format::Emf) => decode_emf_as_raster(data),
        Some(Format::Wmf) | None => Ok(None),
    }
}

pub fn looks_like_metafile(data: &[u8], content_type: Option<&str>) -> bool {
    is_metafile_content_type(content_type) || detect_format(data).is_some()
}

fn content_type_format(content_type: Option<&str>) -> Option<Format> {
    match content_type {
        Some("image/x-emf" | "image/emf") => Some(Format::Emf),
        Some("image/x-wmf" | "image/wmf") => Some(Format::Wmf),
        Some("application/x-msmetafile") => None,
        _ => None,
    }
}

fn is_metafile_content_type(content_type: Option<&str>) -> bool {
    matches!(
        content_type,
        Some(
            "image/x-emf" | "image/emf" | "image/x-wmf" | "image/wmf" | "application/x-msmetafile"
        )
    )
}

fn decode_emf_as_raster(data: &[u8]) -> Result<Option<DecodedMetafile>> {
    let metafile = EmfMetafile::from_bytes(data)?;
    let mut bitmap = None;

    for record in &metafile.records {
        if matches!(
            record.record_kind(),
            Some(EmfRecordType::SetDiBitsToDevice | EmfRecordType::StretchDiBits)
        ) {
            if bitmap.replace(record).is_some() {
                return Err(RenderError::MultipleBitmapRecords);
            }
        }
    }

    let Some(record) = bitmap else {
        return Ok(None);
    };

    let dib = match record.parse_data()? {
        EmfRecordData::SetDiBitsToDevice(value) => value.bitmap.device_independent_bitmap()?,
        EmfRecordData::StretchDiBits(value) => value.bitmap.device_independent_bitmap()?,
        _ => return Ok(None),
    };

    decode_dib_bitmap(&dib)
}

fn decode_dib_bitmap(dib: &DeviceIndependentBitmap) -> Result<Option<DecodedMetafile>> {
    if let Some(format) = dib.embedded_format() {
        return Ok(Some(DecodedMetafile {
            data: dib.bits.clone(),
            content_type: format.content_type(),
        }));
    }

    Err(RenderError::UnsupportedDibCompression {
        compression: dib.info.compression_kind(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use emfsdk::{
        BitmapSourceBounds, DibColorUsage, EMR_EOF, EMR_HEADER, EmfRecord, EmrBitmapBuffer,
        EmrStretchDiBits, RectL, SdkEnumValue, SizeL,
    };

    fn minimal_header_record() -> EmfRecord {
        let mut data = vec![0; 80];
        data[32..36].copy_from_slice(&emfsdk::emf::EMF_SIGNATURE.to_le_bytes());
        EmfRecord::new(EMR_HEADER, data)
    }

    fn eof_record() -> EmfRecord {
        EmfRecord::new(EMR_EOF, vec![0; 12])
    }

    fn png_bitmap_info() -> Vec<u8> {
        vec![
            40, 0, 0, 0, // HeaderSize
            2, 0, 0, 0, // Width
            2, 0, 0, 0, // Height
            1, 0, // Planes
            0, 0, // BitCount
            5, 0, 0, 0, // BI_PNG
            4, 0, 0, 0, // ImageSize
            0, 0, 0, 0, // XPelsPerMeter
            0, 0, 0, 0, // YPelsPerMeter
            0, 0, 0, 0, // ColorUsed
            0, 0, 0, 0, // ColorImportant
        ]
    }

    #[test]
    fn decode_emf_embedded_png_bitmap() {
        let stretch = EmfRecordData::StretchDiBits(EmrStretchDiBits {
            bounds: RectL::default(),
            dest: emfsdk::PointL { x: 0, y: 0 },
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
                bitmap_info: png_bitmap_info(),
                bitmap_bits: vec![0x89, b'P', b'N', b'G'],
            },
        })
        .to_record()
        .unwrap();
        let emf = EmfMetafile {
            records: vec![minimal_header_record(), stretch, eof_record()],
        }
        .to_bytes()
        .unwrap();

        let decoded = decode_metafile_as_raster(&emf, Some("image/x-emf"))
            .unwrap()
            .unwrap();
        assert_eq!(decoded.content_type, "image/png");
        assert_eq!(decoded.data, [0x89, b'P', b'N', b'G']);
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
