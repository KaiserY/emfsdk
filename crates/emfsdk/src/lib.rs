extern crate self as emfsdk;

pub mod bitmap;
pub mod common;
pub mod emf;
pub mod emfplus;
#[cfg(feature = "render")]
pub mod render;
pub mod string;
pub mod types;
pub mod wmf;

pub use emfsdk_derive::{SdkEnum, SdkObject};

pub use crate::bitmap::{
    BITMAP_CORE_HEADER_SIZE, BITMAP_INFO_HEADER_SIZE, BITMAP_V4_HEADER_SIZE, BITMAP_V5_HEADER_SIZE,
    BitmapBitCount, BitmapCompression, BitmapCoreHeader, BitmapInfoHeader, DeviceIndependentBitmap,
    DibBitmapInfo, DibColorUsage, DibHeader, EmbeddedBitmapFormat,
};
pub use crate::common::{
    Error, Format, Reader, Result, SdkEnumValue, SdkRead, SdkSize, SdkWrite, UnknownRecord, Writer,
};
pub use crate::emf::{
    BitmapSourceBounds, EMR_EOF, EMR_HEADER, EmfHeader, EmfMetafile, EmfRecord, EmfRecordData,
    EmfRecordType, EmrBitmapBuffer, EmrComment, EmrCreateBrushIndirect, EmrCreateDibPatternBrushPt,
    EmrCreateMonoBrush, EmrCreatePen, EmrDeleteObject, EmrEllipse, EmrExcludeClipRect,
    EmrExtCreateFontIndirectW, EmrExtCreatePen, EmrExtTextOut, EmrIntersectClipRect, EmrLineTo,
    EmrModifyWorldTransform, EmrMoveToEx, EmrPolyPointsL, EmrPolyPointsS, EmrPolyPolygonL,
    EmrPolyPolygonS, EmrRectangle, EmrSelectObject, EmrSetBkColor, EmrSetBrushOrgEx,
    EmrSetDiBitsToDevice, EmrSetTextColor, EmrSetViewportExtEx, EmrSetViewportOrgEx,
    EmrSetWindowExtEx, EmrSetWindowOrgEx, EmrSetWorldTransform, EmrStretchDiBits, EmrText,
    ExtTextOutOptions, LogFontW,
};
pub use crate::emfplus::{
    EmfPlusBrushRef, EmfPlusDrawRectsData, EmfPlusFillRectsData, EmfPlusGraphicsVersion,
    EmfPlusGraphicsVersionValue, EmfPlusHeaderData, EmfPlusRecord, EmfPlusRecordData,
    EmfPlusRecordFlags, EmfPlusRecordType, EmfPlusRect, EmfPlusRectS,
    EmfPlusScaleWorldTransformData, EmfPlusTranslateWorldTransformData,
};
pub use crate::string::{SdkEncoding, SdkString};
pub use crate::types::{
    ColorRef, EmfPlusArgb, PointF, PointL, PointS, RectF, RectL, RectS, SizeF, SizeL, SizeS,
    TriVertex, XForm,
};
pub use crate::wmf::{
    META_EOF, WmfHeader, WmfMetafile, WmfPlaceableHeader, WmfRecord, WmfRecordFunction,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Metafile {
    Emf(EmfMetafile),
    Wmf(WmfMetafile),
}

impl Metafile {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        match detect_format(bytes) {
            Some(Format::Emf) => Ok(Self::Emf(EmfMetafile::from_bytes(bytes)?)),
            Some(Format::Wmf) => Ok(Self::Wmf(WmfMetafile::from_bytes(bytes)?)),
            None => Err(Error::UnsupportedFormat),
        }
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        match self {
            Self::Emf(value) => value.to_bytes(),
            Self::Wmf(value) => value.to_bytes(),
        }
    }

    pub fn format(&self) -> Format {
        match self {
            Self::Emf(_) => Format::Emf,
            Self::Wmf(_) => Format::Wmf,
        }
    }
}

pub fn detect_format(bytes: &[u8]) -> Option<Format> {
    if emf::looks_like_emf(bytes) {
        Some(Format::Emf)
    } else if wmf::looks_like_wmf(bytes) {
        Some(Format::Wmf)
    } else {
        None
    }
}
