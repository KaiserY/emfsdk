use std::io::Cursor;

use emfsdk_derive::{SdkEnum, SdkObject};

use crate::bitmap::{DeviceIndependentBitmap, DibBitmapInfo, DibColorUsage};
use crate::common::{Error, Reader, Result, SdkEnumValue, SdkRead, SdkSize, SdkWrite, Writer};
use crate::string::{SdkEncoding, SdkString};
use crate::types::{ColorRef, PointL, RectL, SizeL, XForm};

pub const EMR_HEADER: u32 = 0x0000_0001;
pub const EMR_EOF: u32 = 0x0000_000E;
pub const EMF_HEADER_MIN_SIZE: u32 = 88;
pub const EMF_SIGNATURE: u32 = 0x464D_4520;
pub const EMR_COMMENT: u32 = 0x0000_0046;
pub const EMR_COMMENT_EMFPLUS: u32 = 0x2B46_4D45;
pub const ENHMETA_STOCK_OBJECT: u32 = 0x8000_0000;
pub const LOGFONT_FACE_NAME_CHARS: usize = 32;

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct ExtTextOutOptions: u32 {
        const OPAQUE = 0x0000_0002;
        const CLIPPED = 0x0000_0004;
        const GLYPH_INDEX = 0x0000_0010;
        const RTL_READING = 0x0000_0080;
        const NUMERICS_LOCAL = 0x0000_0400;
        const NUMERICS_LATIN = 0x0000_0800;
        const PDY = 0x0000_2000;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, SdkEnum)]
#[sdk(repr = "u32")]
pub enum EmfRecordType {
    Header = 0x0000_0001,
    PolyBezier = 0x0000_0002,
    Polygon = 0x0000_0003,
    Polyline = 0x0000_0004,
    PolyBezierTo = 0x0000_0005,
    PolylineTo = 0x0000_0006,
    PolyPolyline = 0x0000_0007,
    PolyPolygon = 0x0000_0008,
    SetWindowExtEx = 0x0000_0009,
    SetWindowOrgEx = 0x0000_000A,
    SetViewportExtEx = 0x0000_000B,
    SetViewportOrgEx = 0x0000_000C,
    SetBrushOrgEx = 0x0000_000D,
    Eof = 0x0000_000E,
    SetPixelV = 0x0000_000F,
    SetMapperFlags = 0x0000_0010,
    SetMapMode = 0x0000_0011,
    SetBkMode = 0x0000_0012,
    SetPolyfillMode = 0x0000_0013,
    SetRop2 = 0x0000_0014,
    SetStretchBltMode = 0x0000_0015,
    SetTextAlign = 0x0000_0016,
    SetColorAdjustment = 0x0000_0017,
    SetTextColor = 0x0000_0018,
    SetBkColor = 0x0000_0019,
    OffsetClipRgn = 0x0000_001A,
    MoveToEx = 0x0000_001B,
    SetMetaRgn = 0x0000_001C,
    ExcludeClipRect = 0x0000_001D,
    IntersectClipRect = 0x0000_001E,
    ScaleViewportExtEx = 0x0000_001F,
    ScaleWindowExtEx = 0x0000_0020,
    SaveDc = 0x0000_0021,
    RestoreDc = 0x0000_0022,
    SetWorldTransform = 0x0000_0023,
    ModifyWorldTransform = 0x0000_0024,
    SelectObject = 0x0000_0025,
    CreatePen = 0x0000_0026,
    CreateBrushIndirect = 0x0000_0027,
    DeleteObject = 0x0000_0028,
    AngleArc = 0x0000_0029,
    Ellipse = 0x0000_002A,
    Rectangle = 0x0000_002B,
    RoundRect = 0x0000_002C,
    Arc = 0x0000_002D,
    Chord = 0x0000_002E,
    Pie = 0x0000_002F,
    SelectPalette = 0x0000_0030,
    CreatePalette = 0x0000_0031,
    SetPaletteEntries = 0x0000_0032,
    ResizePalette = 0x0000_0033,
    RealizePalette = 0x0000_0034,
    ExtFloodFill = 0x0000_0035,
    LineTo = 0x0000_0036,
    ArcTo = 0x0000_0037,
    PolyDraw = 0x0000_0038,
    SetArcDirection = 0x0000_0039,
    SetMiterLimit = 0x0000_003A,
    BeginPath = 0x0000_003B,
    EndPath = 0x0000_003C,
    CloseFigure = 0x0000_003D,
    FillPath = 0x0000_003E,
    StrokeAndFillPath = 0x0000_003F,
    StrokePath = 0x0000_0040,
    FlattenPath = 0x0000_0041,
    WidenPath = 0x0000_0042,
    SelectClipPath = 0x0000_0043,
    AbortPath = 0x0000_0044,
    Comment = 0x0000_0046,
    FillRgn = 0x0000_0047,
    FrameRgn = 0x0000_0048,
    InvertRgn = 0x0000_0049,
    PaintRgn = 0x0000_004A,
    ExtSelectClipRgn = 0x0000_004B,
    BitBlt = 0x0000_004C,
    StretchBlt = 0x0000_004D,
    MaskBlt = 0x0000_004E,
    PlgBlt = 0x0000_004F,
    SetDiBitsToDevice = 0x0000_0050,
    StretchDiBits = 0x0000_0051,
    ExtCreateFontIndirectW = 0x0000_0052,
    ExtTextOutA = 0x0000_0053,
    ExtTextOutW = 0x0000_0054,
    PolyBezier16 = 0x0000_0055,
    Polygon16 = 0x0000_0056,
    Polyline16 = 0x0000_0057,
    PolyBezierTo16 = 0x0000_0058,
    PolylineTo16 = 0x0000_0059,
    PolyPolyline16 = 0x0000_005A,
    PolyPolygon16 = 0x0000_005B,
    PolyDraw16 = 0x0000_005C,
    CreateMonoBrush = 0x0000_005D,
    CreateDibPatternBrushPt = 0x0000_005E,
    ExtCreatePen = 0x0000_005F,
    PolyTextOutA = 0x0000_0060,
    PolyTextOutW = 0x0000_0061,
    SetIcmMode = 0x0000_0062,
    CreateColorSpace = 0x0000_0063,
    SetColorSpace = 0x0000_0064,
    DeleteColorSpace = 0x0000_0065,
    GlsRecord = 0x0000_0066,
    GlsBoundedRecord = 0x0000_0067,
    PixelFormat = 0x0000_0068,
    DrawEscape = 0x0000_0069,
    ExtEscape = 0x0000_006A,
    SmallTextOut = 0x0000_006C,
    ForceUfiMapping = 0x0000_006D,
    NamedEscape = 0x0000_006E,
    ColorCorrectPalette = 0x0000_006F,
    SetIcmProfileA = 0x0000_0070,
    SetIcmProfileW = 0x0000_0071,
    AlphaBlend = 0x0000_0072,
    SetLayout = 0x0000_0073,
    TransparentBlt = 0x0000_0074,
    GradientFill = 0x0000_0076,
    SetLinkedUfis = 0x0000_0077,
    SetTextJustification = 0x0000_0078,
    ColorMatchToTargetW = 0x0000_0079,
    CreateColorSpaceW = 0x0000_007A,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmfMetafile {
    pub records: Vec<EmfRecord>,
}

impl EmfMetafile {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(Cursor::new(bytes));
        let mut records = Vec::new();

        while reader.position()? < bytes.len() as u64 {
            let record = EmfRecord::read_from(&mut reader, bytes.len() as u64)?;
            let is_eof = record.record_type == EMR_EOF;
            records.push(record);
            if is_eof {
                break;
            }
        }

        if !matches!(records.first(), Some(record) if record.record_type == EMR_HEADER) {
            return Err(Error::invalid(0, "EMF metafile must start with EMR_HEADER"));
        }

        Ok(Self { records })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        for record in &self.records {
            record.write_to(&mut writer)?;
        }
        Ok(writer.into_inner().into_inner())
    }

    pub fn header(&self) -> Option<&EmfRecord> {
        self.records
            .first()
            .filter(|record| record.record_type == EMR_HEADER)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmfRecord {
    pub record_type: u32,
    pub data: Vec<u8>,
}

impl EmfRecord {
    pub fn new(record_type: u32, data: Vec<u8>) -> Self {
        Self { record_type, data }
    }

    pub fn as_header(&self) -> Result<Option<EmfHeader>> {
        if self.record_type != EMR_HEADER {
            return Ok(None);
        }
        Ok(Some(EmfHeader::from_record_data(&self.data)?))
    }

    pub fn record_kind(&self) -> Option<EmfRecordType> {
        EmfRecordType::from_raw(self.record_type)
    }

    pub fn parse_data(&self) -> Result<EmfRecordData<'_>> {
        EmfRecordData::from_record(self)
    }

    pub fn emf_plus_payload(&self) -> Option<&[u8]> {
        if self.record_type != EMR_COMMENT || self.data.len() < 8 {
            return None;
        }
        let data_size = u32::from_le_bytes(self.data[0..4].try_into().ok()?) as usize;
        let identifier = u32::from_le_bytes(self.data[4..8].try_into().ok()?);
        if identifier != EMR_COMMENT_EMFPLUS || data_size < 4 {
            return None;
        }
        let payload_len = data_size - 4;
        self.data.get(8..8 + payload_len)
    }

    fn read_from<R: std::io::Read + std::io::Seek>(
        reader: &mut Reader<R>,
        file_len: u64,
    ) -> Result<Self> {
        let offset = reader.position()?;
        let record_type = reader.read_u32()?;
        let size = reader.read_u32()?;
        if size < 8 {
            return Err(Error::invalid(
                offset,
                "EMF record size is smaller than its header",
            ));
        }
        if size % 4 != 0 {
            return Err(Error::invalid(
                offset,
                "EMF record size is not 32-bit aligned",
            ));
        }
        let end = offset
            .checked_add(size as u64)
            .ok_or_else(|| Error::invalid(offset, "EMF record size overflows"))?;
        if end > file_len {
            return Err(Error::invalid(
                offset,
                "EMF record extends past end of file",
            ));
        }
        let data = reader.read_vec(size as usize - 8)?;
        Ok(Self { record_type, data })
    }

    fn write_to<W: std::io::Write + std::io::Seek>(&self, writer: &mut Writer<W>) -> Result<()> {
        let size = self.data.len().checked_add(8).ok_or_else(|| {
            Error::invalid(writer.position().unwrap_or(0), "EMF record is too large")
        })?;
        if size > u32::MAX as usize {
            return Err(Error::invalid(
                writer.position()?,
                "EMF record size exceeds u32::MAX",
            ));
        }
        if size % 4 != 0 {
            return Err(Error::invalid(
                writer.position()?,
                "EMF record data must include any required 32-bit alignment padding",
            ));
        }
        writer.write_u32(self.record_type)?;
        writer.write_u32(size as u32)?;
        writer.write_all(&self.data)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum EmfRecordData<'a> {
    Header(EmfHeader),
    Eof(EmrEof),
    PolyBezier(EmrPolyPointsL),
    SetWindowExtEx(EmrSetWindowExtEx),
    SetWindowOrgEx(EmrSetWindowOrgEx),
    SetViewportExtEx(EmrSetViewportExtEx),
    SetViewportOrgEx(EmrSetViewportOrgEx),
    SetBrushOrgEx(EmrSetBrushOrgEx),
    SetPixelV(EmrSetPixelV),
    SetMapperFlags(EmrSetMapperFlags),
    SetMapMode(EmrSetMapMode),
    SetBkMode(EmrSetBkMode),
    SetPolyFillMode(EmrSetPolyFillMode),
    SetRop2(EmrSetRop2),
    SetStretchBltMode(EmrSetStretchBltMode),
    SetTextAlign(EmrSetTextAlign),
    SetTextColor(EmrSetTextColor),
    SetBkColor(EmrSetBkColor),
    OffsetClipRgn(EmrOffsetClipRgn),
    SetMetaRgn,
    ExcludeClipRect(EmrExcludeClipRect),
    IntersectClipRect(EmrIntersectClipRect),
    ScaleViewportExtEx(EmrScaleViewportExtEx),
    ScaleWindowExtEx(EmrScaleWindowExtEx),
    SaveDc,
    RestoreDc(EmrRestoreDc),
    SetWorldTransform(EmrSetWorldTransform),
    ModifyWorldTransform(EmrModifyWorldTransform),
    SelectObject(EmrSelectObject),
    SelectPalette(EmrSelectPalette),
    ResizePalette(EmrResizePalette),
    DeleteObject(EmrDeleteObject),
    MoveToEx(EmrMoveToEx),
    LineTo(EmrLineTo),
    AngleArc(EmrAngleArc),
    RoundRect(EmrRoundRect),
    Arc(EmrArc),
    ArcTo(EmrArc),
    Chord(EmrArc),
    Pie(EmrArc),
    ExtFloodFill(EmrExtFloodFill),
    SetArcDirection(EmrSetArcDirection),
    SetMiterLimit(EmrSetMiterLimit),
    BeginPath,
    EndPath,
    CloseFigure,
    FillPath(EmrFillPath),
    StrokeAndFillPath(EmrStrokeAndFillPath),
    StrokePath(EmrStrokePath),
    FlattenPath,
    WidenPath,
    SelectClipPath(EmrSelectClipPath),
    AbortPath,
    CreatePen(EmrCreatePen),
    CreateBrushIndirect(EmrCreateBrushIndirect),
    CreatePalette(EmrCreatePalette),
    SetPaletteEntries(EmrSetPaletteEntries),
    ExtCreatePen(EmrExtCreatePen),
    ExtCreateFontIndirectW(EmrExtCreateFontIndirectW),
    CreateMonoBrush(EmrCreateMonoBrush),
    CreateDibPatternBrushPt(EmrCreateDibPatternBrushPt),
    Polygon(EmrPolyPointsL),
    Polyline(EmrPolyPointsL),
    PolyBezierTo(EmrPolyPointsL),
    PolylineTo(EmrPolyPointsL),
    Polygon16(EmrPolyPointsS),
    Polyline16(EmrPolyPointsS),
    PolyBezier16(EmrPolyPointsS),
    PolyBezierTo16(EmrPolyPointsS),
    PolylineTo16(EmrPolyPointsS),
    PolyPolyline(EmrPolyPolygonL),
    PolyPolygon(EmrPolyPolygonL),
    PolyPolyline16(EmrPolyPolygonS),
    PolyPolygon16(EmrPolyPolygonS),
    Rectangle(EmrRectangle),
    Ellipse(EmrEllipse),
    RealizePalette,
    ExtTextOutA(EmrExtTextOut),
    ExtTextOutW(EmrExtTextOut),
    SetDiBitsToDevice(EmrSetDiBitsToDevice),
    StretchDiBits(EmrStretchDiBits),
    SetIcmMode(EmrSetIcmMode),
    SetColorSpace(EmrSetColorSpace),
    DeleteColorSpace(EmrDeleteColorSpace),
    SetLayout(EmrSetLayout),
    SetTextJustification(EmrSetTextJustification),
    Comment(EmrComment),
    Unknown(&'a EmfRecord),
}

impl<'a> EmfRecordData<'a> {
    pub fn from_record(record: &'a EmfRecord) -> Result<Self> {
        let data = record.data.as_slice();
        Ok(match record.record_kind() {
            Some(EmfRecordType::Header) => Self::Header(EmfHeader::from_record_data(data)?),
            Some(EmfRecordType::Eof) => Self::Eof(EmrEof::read_data(data)?),
            Some(EmfRecordType::PolyBezier) => Self::PolyBezier(EmrPolyPointsL::read_data(data)?),
            Some(EmfRecordType::SetWindowExtEx) => Self::SetWindowExtEx(read_object(data)?),
            Some(EmfRecordType::SetWindowOrgEx) => Self::SetWindowOrgEx(read_object(data)?),
            Some(EmfRecordType::SetViewportExtEx) => Self::SetViewportExtEx(read_object(data)?),
            Some(EmfRecordType::SetViewportOrgEx) => Self::SetViewportOrgEx(read_object(data)?),
            Some(EmfRecordType::SetBrushOrgEx) => Self::SetBrushOrgEx(read_object(data)?),
            Some(EmfRecordType::SetPixelV) => Self::SetPixelV(read_object(data)?),
            Some(EmfRecordType::SetMapperFlags) => Self::SetMapperFlags(read_object(data)?),
            Some(EmfRecordType::SetMapMode) => Self::SetMapMode(read_object(data)?),
            Some(EmfRecordType::SetBkMode) => Self::SetBkMode(read_object(data)?),
            Some(EmfRecordType::SetPolyfillMode) => Self::SetPolyFillMode(read_object(data)?),
            Some(EmfRecordType::SetRop2) => Self::SetRop2(read_object(data)?),
            Some(EmfRecordType::SetStretchBltMode) => Self::SetStretchBltMode(read_object(data)?),
            Some(EmfRecordType::SetTextAlign) => Self::SetTextAlign(read_object(data)?),
            Some(EmfRecordType::SetTextColor) => Self::SetTextColor(read_object(data)?),
            Some(EmfRecordType::SetBkColor) => Self::SetBkColor(read_object(data)?),
            Some(EmfRecordType::OffsetClipRgn) => Self::OffsetClipRgn(read_object(data)?),
            Some(EmfRecordType::SetMetaRgn) => {
                ensure_no_data(data, "EMR_SETMETARGN")?;
                Self::SetMetaRgn
            }
            Some(EmfRecordType::ExcludeClipRect) => Self::ExcludeClipRect(read_object(data)?),
            Some(EmfRecordType::IntersectClipRect) => Self::IntersectClipRect(read_object(data)?),
            Some(EmfRecordType::ScaleViewportExtEx) => Self::ScaleViewportExtEx(read_object(data)?),
            Some(EmfRecordType::ScaleWindowExtEx) => Self::ScaleWindowExtEx(read_object(data)?),
            Some(EmfRecordType::SaveDc) => {
                ensure_no_data(data, "EMR_SAVEDC")?;
                Self::SaveDc
            }
            Some(EmfRecordType::RestoreDc) => Self::RestoreDc(read_object(data)?),
            Some(EmfRecordType::SetWorldTransform) => Self::SetWorldTransform(read_object(data)?),
            Some(EmfRecordType::ModifyWorldTransform) => {
                Self::ModifyWorldTransform(read_object(data)?)
            }
            Some(EmfRecordType::SelectObject) => Self::SelectObject(read_object(data)?),
            Some(EmfRecordType::SelectPalette) => Self::SelectPalette(read_object(data)?),
            Some(EmfRecordType::ResizePalette) => Self::ResizePalette(read_object(data)?),
            Some(EmfRecordType::DeleteObject) => Self::DeleteObject(read_object(data)?),
            Some(EmfRecordType::MoveToEx) => Self::MoveToEx(read_object(data)?),
            Some(EmfRecordType::LineTo) => Self::LineTo(read_object(data)?),
            Some(EmfRecordType::AngleArc) => Self::AngleArc(read_object(data)?),
            Some(EmfRecordType::RoundRect) => Self::RoundRect(read_object(data)?),
            Some(EmfRecordType::Arc) => Self::Arc(read_object(data)?),
            Some(EmfRecordType::ArcTo) => Self::ArcTo(read_object(data)?),
            Some(EmfRecordType::Chord) => Self::Chord(read_object(data)?),
            Some(EmfRecordType::Pie) => Self::Pie(read_object(data)?),
            Some(EmfRecordType::ExtFloodFill) => Self::ExtFloodFill(read_object(data)?),
            Some(EmfRecordType::SetArcDirection) => Self::SetArcDirection(read_object(data)?),
            Some(EmfRecordType::SetMiterLimit) => Self::SetMiterLimit(read_object(data)?),
            Some(EmfRecordType::BeginPath) => {
                ensure_no_data(data, "EMR_BEGINPATH")?;
                Self::BeginPath
            }
            Some(EmfRecordType::EndPath) => {
                ensure_no_data(data, "EMR_ENDPATH")?;
                Self::EndPath
            }
            Some(EmfRecordType::CloseFigure) => {
                ensure_no_data(data, "EMR_CLOSEFIGURE")?;
                Self::CloseFigure
            }
            Some(EmfRecordType::FillPath) => Self::FillPath(read_object(data)?),
            Some(EmfRecordType::StrokeAndFillPath) => Self::StrokeAndFillPath(read_object(data)?),
            Some(EmfRecordType::StrokePath) => Self::StrokePath(read_object(data)?),
            Some(EmfRecordType::FlattenPath) => {
                ensure_no_data(data, "EMR_FLATTENPATH")?;
                Self::FlattenPath
            }
            Some(EmfRecordType::WidenPath) => {
                ensure_no_data(data, "EMR_WIDENPATH")?;
                Self::WidenPath
            }
            Some(EmfRecordType::SelectClipPath) => Self::SelectClipPath(read_object(data)?),
            Some(EmfRecordType::AbortPath) => {
                ensure_no_data(data, "EMR_ABORTPATH")?;
                Self::AbortPath
            }
            Some(EmfRecordType::CreatePen) => Self::CreatePen(read_object(data)?),
            Some(EmfRecordType::CreateBrushIndirect) => {
                Self::CreateBrushIndirect(read_object(data)?)
            }
            Some(EmfRecordType::CreatePalette) => {
                Self::CreatePalette(EmrCreatePalette::read_data(data)?)
            }
            Some(EmfRecordType::SetPaletteEntries) => {
                Self::SetPaletteEntries(EmrSetPaletteEntries::read_data(data)?)
            }
            Some(EmfRecordType::ExtCreatePen) => {
                Self::ExtCreatePen(EmrExtCreatePen::read_data(data)?)
            }
            Some(EmfRecordType::ExtCreateFontIndirectW) => {
                Self::ExtCreateFontIndirectW(EmrExtCreateFontIndirectW::read_data(data)?)
            }
            Some(EmfRecordType::CreateMonoBrush) => {
                Self::CreateMonoBrush(EmrCreateMonoBrush::read_data(data)?)
            }
            Some(EmfRecordType::CreateDibPatternBrushPt) => {
                Self::CreateDibPatternBrushPt(EmrCreateDibPatternBrushPt::read_data(data)?)
            }
            Some(EmfRecordType::Polygon) => Self::Polygon(EmrPolyPointsL::read_data(data)?),
            Some(EmfRecordType::Polyline) => Self::Polyline(EmrPolyPointsL::read_data(data)?),
            Some(EmfRecordType::PolyBezierTo) => {
                Self::PolyBezierTo(EmrPolyPointsL::read_data(data)?)
            }
            Some(EmfRecordType::PolylineTo) => Self::PolylineTo(EmrPolyPointsL::read_data(data)?),
            Some(EmfRecordType::PolyPolyline) => {
                Self::PolyPolyline(EmrPolyPolygonL::read_data(data)?)
            }
            Some(EmfRecordType::PolyBezier16) => {
                Self::PolyBezier16(EmrPolyPointsS::read_data(data)?)
            }
            Some(EmfRecordType::Polygon16) => Self::Polygon16(EmrPolyPointsS::read_data(data)?),
            Some(EmfRecordType::Polyline16) => Self::Polyline16(EmrPolyPointsS::read_data(data)?),
            Some(EmfRecordType::PolyBezierTo16) => {
                Self::PolyBezierTo16(EmrPolyPointsS::read_data(data)?)
            }
            Some(EmfRecordType::PolylineTo16) => {
                Self::PolylineTo16(EmrPolyPointsS::read_data(data)?)
            }
            Some(EmfRecordType::PolyPolygon) => {
                Self::PolyPolygon(EmrPolyPolygonL::read_data(data)?)
            }
            Some(EmfRecordType::PolyPolyline16) => {
                Self::PolyPolyline16(EmrPolyPolygonS::read_data(data)?)
            }
            Some(EmfRecordType::PolyPolygon16) => {
                Self::PolyPolygon16(EmrPolyPolygonS::read_data(data)?)
            }
            Some(EmfRecordType::Rectangle) => Self::Rectangle(read_object(data)?),
            Some(EmfRecordType::Ellipse) => Self::Ellipse(read_object(data)?),
            Some(EmfRecordType::RealizePalette) => {
                ensure_no_data(data, "EMR_REALIZEPALETTE")?;
                Self::RealizePalette
            }
            Some(EmfRecordType::ExtTextOutA) => {
                Self::ExtTextOutA(EmrExtTextOut::read_data(data, false)?)
            }
            Some(EmfRecordType::ExtTextOutW) => {
                Self::ExtTextOutW(EmrExtTextOut::read_data(data, true)?)
            }
            Some(EmfRecordType::SetDiBitsToDevice) => {
                Self::SetDiBitsToDevice(EmrSetDiBitsToDevice::read_data(data)?)
            }
            Some(EmfRecordType::StretchDiBits) => {
                Self::StretchDiBits(EmrStretchDiBits::read_data(data)?)
            }
            Some(EmfRecordType::SetIcmMode) => Self::SetIcmMode(read_object(data)?),
            Some(EmfRecordType::SetColorSpace) => Self::SetColorSpace(read_object(data)?),
            Some(EmfRecordType::DeleteColorSpace) => Self::DeleteColorSpace(read_object(data)?),
            Some(EmfRecordType::SetLayout) => Self::SetLayout(read_object(data)?),
            Some(EmfRecordType::SetTextJustification) => {
                Self::SetTextJustification(read_object(data)?)
            }
            Some(EmfRecordType::Comment) => Self::Comment(EmrComment::read_data(data)?),
            _ => Self::Unknown(record),
        })
    }

    pub fn to_record(&self) -> Result<EmfRecord> {
        match self {
            Self::Header(value) => Ok(EmfRecord::new(EMR_HEADER, value.to_record_data()?)),
            Self::Eof(value) => Ok(EmfRecord::new(EMR_EOF, value.to_data()?)),
            Self::PolyBezier(value) => Ok(EmfRecord::new(
                EmfRecordType::PolyBezier.raw(),
                value.to_data()?,
            )),
            Self::SetWindowExtEx(value) => object_record(EmfRecordType::SetWindowExtEx, value),
            Self::SetWindowOrgEx(value) => object_record(EmfRecordType::SetWindowOrgEx, value),
            Self::SetViewportExtEx(value) => object_record(EmfRecordType::SetViewportExtEx, value),
            Self::SetViewportOrgEx(value) => object_record(EmfRecordType::SetViewportOrgEx, value),
            Self::SetBrushOrgEx(value) => object_record(EmfRecordType::SetBrushOrgEx, value),
            Self::SetPixelV(value) => object_record(EmfRecordType::SetPixelV, value),
            Self::SetMapperFlags(value) => object_record(EmfRecordType::SetMapperFlags, value),
            Self::SetMapMode(value) => object_record(EmfRecordType::SetMapMode, value),
            Self::SetBkMode(value) => object_record(EmfRecordType::SetBkMode, value),
            Self::SetPolyFillMode(value) => object_record(EmfRecordType::SetPolyfillMode, value),
            Self::SetRop2(value) => object_record(EmfRecordType::SetRop2, value),
            Self::SetStretchBltMode(value) => {
                object_record(EmfRecordType::SetStretchBltMode, value)
            }
            Self::SetTextAlign(value) => object_record(EmfRecordType::SetTextAlign, value),
            Self::SetTextColor(value) => object_record(EmfRecordType::SetTextColor, value),
            Self::SetBkColor(value) => object_record(EmfRecordType::SetBkColor, value),
            Self::OffsetClipRgn(value) => object_record(EmfRecordType::OffsetClipRgn, value),
            Self::SetMetaRgn => Ok(no_data_record(EmfRecordType::SetMetaRgn)),
            Self::ExcludeClipRect(value) => object_record(EmfRecordType::ExcludeClipRect, value),
            Self::IntersectClipRect(value) => {
                object_record(EmfRecordType::IntersectClipRect, value)
            }
            Self::ScaleViewportExtEx(value) => {
                object_record(EmfRecordType::ScaleViewportExtEx, value)
            }
            Self::ScaleWindowExtEx(value) => object_record(EmfRecordType::ScaleWindowExtEx, value),
            Self::SaveDc => Ok(no_data_record(EmfRecordType::SaveDc)),
            Self::RestoreDc(value) => object_record(EmfRecordType::RestoreDc, value),
            Self::SetWorldTransform(value) => {
                object_record(EmfRecordType::SetWorldTransform, value)
            }
            Self::ModifyWorldTransform(value) => {
                object_record(EmfRecordType::ModifyWorldTransform, value)
            }
            Self::SelectObject(value) => object_record(EmfRecordType::SelectObject, value),
            Self::SelectPalette(value) => object_record(EmfRecordType::SelectPalette, value),
            Self::ResizePalette(value) => object_record(EmfRecordType::ResizePalette, value),
            Self::DeleteObject(value) => object_record(EmfRecordType::DeleteObject, value),
            Self::MoveToEx(value) => object_record(EmfRecordType::MoveToEx, value),
            Self::LineTo(value) => object_record(EmfRecordType::LineTo, value),
            Self::AngleArc(value) => object_record(EmfRecordType::AngleArc, value),
            Self::RoundRect(value) => object_record(EmfRecordType::RoundRect, value),
            Self::Arc(value) => object_record(EmfRecordType::Arc, value),
            Self::ArcTo(value) => object_record(EmfRecordType::ArcTo, value),
            Self::Chord(value) => object_record(EmfRecordType::Chord, value),
            Self::Pie(value) => object_record(EmfRecordType::Pie, value),
            Self::ExtFloodFill(value) => object_record(EmfRecordType::ExtFloodFill, value),
            Self::SetArcDirection(value) => object_record(EmfRecordType::SetArcDirection, value),
            Self::SetMiterLimit(value) => object_record(EmfRecordType::SetMiterLimit, value),
            Self::BeginPath => Ok(no_data_record(EmfRecordType::BeginPath)),
            Self::EndPath => Ok(no_data_record(EmfRecordType::EndPath)),
            Self::CloseFigure => Ok(no_data_record(EmfRecordType::CloseFigure)),
            Self::FillPath(value) => object_record(EmfRecordType::FillPath, value),
            Self::StrokeAndFillPath(value) => {
                object_record(EmfRecordType::StrokeAndFillPath, value)
            }
            Self::StrokePath(value) => object_record(EmfRecordType::StrokePath, value),
            Self::FlattenPath => Ok(no_data_record(EmfRecordType::FlattenPath)),
            Self::WidenPath => Ok(no_data_record(EmfRecordType::WidenPath)),
            Self::SelectClipPath(value) => object_record(EmfRecordType::SelectClipPath, value),
            Self::AbortPath => Ok(no_data_record(EmfRecordType::AbortPath)),
            Self::CreatePen(value) => object_record(EmfRecordType::CreatePen, value),
            Self::CreateBrushIndirect(value) => {
                object_record(EmfRecordType::CreateBrushIndirect, value)
            }
            Self::CreatePalette(value) => Ok(EmfRecord::new(
                EmfRecordType::CreatePalette.raw(),
                value.to_data()?,
            )),
            Self::SetPaletteEntries(value) => Ok(EmfRecord::new(
                EmfRecordType::SetPaletteEntries.raw(),
                value.to_data()?,
            )),
            Self::ExtCreatePen(value) => Ok(EmfRecord::new(
                EmfRecordType::ExtCreatePen.raw(),
                value.to_data()?,
            )),
            Self::ExtCreateFontIndirectW(value) => Ok(EmfRecord::new(
                EmfRecordType::ExtCreateFontIndirectW.raw(),
                value.to_data()?,
            )),
            Self::CreateMonoBrush(value) => Ok(EmfRecord::new(
                EmfRecordType::CreateMonoBrush.raw(),
                value.to_data()?,
            )),
            Self::CreateDibPatternBrushPt(value) => Ok(EmfRecord::new(
                EmfRecordType::CreateDibPatternBrushPt.raw(),
                value.to_data()?,
            )),
            Self::Polygon(value) => Ok(EmfRecord::new(
                EmfRecordType::Polygon.raw(),
                value.to_data()?,
            )),
            Self::Polyline(value) => Ok(EmfRecord::new(
                EmfRecordType::Polyline.raw(),
                value.to_data()?,
            )),
            Self::PolyBezierTo(value) => Ok(EmfRecord::new(
                EmfRecordType::PolyBezierTo.raw(),
                value.to_data()?,
            )),
            Self::PolylineTo(value) => Ok(EmfRecord::new(
                EmfRecordType::PolylineTo.raw(),
                value.to_data()?,
            )),
            Self::Polygon16(value) => Ok(EmfRecord::new(
                EmfRecordType::Polygon16.raw(),
                value.to_data()?,
            )),
            Self::Polyline16(value) => Ok(EmfRecord::new(
                EmfRecordType::Polyline16.raw(),
                value.to_data()?,
            )),
            Self::PolyBezier16(value) => Ok(EmfRecord::new(
                EmfRecordType::PolyBezier16.raw(),
                value.to_data()?,
            )),
            Self::PolyBezierTo16(value) => Ok(EmfRecord::new(
                EmfRecordType::PolyBezierTo16.raw(),
                value.to_data()?,
            )),
            Self::PolylineTo16(value) => Ok(EmfRecord::new(
                EmfRecordType::PolylineTo16.raw(),
                value.to_data()?,
            )),
            Self::PolyPolyline(value) => Ok(EmfRecord::new(
                EmfRecordType::PolyPolyline.raw(),
                value.to_data()?,
            )),
            Self::PolyPolygon(value) => Ok(EmfRecord::new(
                EmfRecordType::PolyPolygon.raw(),
                value.to_data()?,
            )),
            Self::PolyPolyline16(value) => Ok(EmfRecord::new(
                EmfRecordType::PolyPolyline16.raw(),
                value.to_data()?,
            )),
            Self::PolyPolygon16(value) => Ok(EmfRecord::new(
                EmfRecordType::PolyPolygon16.raw(),
                value.to_data()?,
            )),
            Self::Rectangle(value) => object_record(EmfRecordType::Rectangle, value),
            Self::Ellipse(value) => object_record(EmfRecordType::Ellipse, value),
            Self::RealizePalette => Ok(no_data_record(EmfRecordType::RealizePalette)),
            Self::ExtTextOutA(value) => Ok(EmfRecord::new(
                EmfRecordType::ExtTextOutA.raw(),
                value.to_data(false)?,
            )),
            Self::ExtTextOutW(value) => Ok(EmfRecord::new(
                EmfRecordType::ExtTextOutW.raw(),
                value.to_data(true)?,
            )),
            Self::SetDiBitsToDevice(value) => Ok(EmfRecord::new(
                EmfRecordType::SetDiBitsToDevice.raw(),
                value.to_data()?,
            )),
            Self::StretchDiBits(value) => Ok(EmfRecord::new(
                EmfRecordType::StretchDiBits.raw(),
                value.to_data()?,
            )),
            Self::SetIcmMode(value) => object_record(EmfRecordType::SetIcmMode, value),
            Self::SetColorSpace(value) => object_record(EmfRecordType::SetColorSpace, value),
            Self::DeleteColorSpace(value) => object_record(EmfRecordType::DeleteColorSpace, value),
            Self::SetLayout(value) => object_record(EmfRecordType::SetLayout, value),
            Self::SetTextJustification(value) => {
                object_record(EmfRecordType::SetTextJustification, value)
            }
            Self::Comment(value) => Ok(EmfRecord::new(
                EmfRecordType::Comment.raw(),
                value.to_data()?,
            )),
            Self::Unknown(record) => Ok((*record).clone()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct LogPaletteEntry {
    pub reserved: u8,
    pub blue: u8,
    pub green: u8,
    pub red: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogPalette {
    pub version: u16,
    pub entries: Vec<LogPaletteEntry>,
}

impl LogPalette {
    pub fn read_from<R: std::io::Read + std::io::Seek>(reader: &mut Reader<R>) -> Result<Self> {
        let version = reader.read_u16()?;
        let entry_count = reader.read_u16()? as usize;
        let mut entries = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            entries.push(LogPaletteEntry::read_from(reader)?);
        }
        Ok(Self { version, entries })
    }

    pub fn write_to<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut Writer<W>,
    ) -> Result<()> {
        writer.write_u16(self.version)?;
        writer.write_u16(
            u16::try_from(self.entries.len())
                .map_err(|_| Error::invalid(0, "LogPalette entry count exceeds u16::MAX"))?,
        )?;
        for entry in &self.entries {
            entry.write_to(writer)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmrEof {
    pub palette_entries_offset: u32,
    pub palette_prefix: Vec<u8>,
    pub palette_entries: Vec<LogPaletteEntry>,
    pub palette_suffix: Vec<u8>,
    pub size_last: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmrCreatePalette {
    pub palette_index: u32,
    pub log_palette: LogPalette,
}

impl EmrCreatePalette {
    pub fn read_data(data: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(Cursor::new(data));
        Ok(Self {
            palette_index: reader.read_u32()?,
            log_palette: LogPalette::read_from(&mut reader)?,
        })
    }

    pub fn to_data(&self) -> Result<Vec<u8>> {
        let mut writer = Writer::new(Cursor::new(Vec::with_capacity(
            8 + self.log_palette.entries.len() * 4,
        )));
        writer.write_u32(self.palette_index)?;
        self.log_palette.write_to(&mut writer)?;
        Ok(writer.into_inner().into_inner())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmrSetPaletteEntries {
    pub palette_index: u32,
    pub start: u32,
    pub entries: Vec<LogPaletteEntry>,
}

impl EmrSetPaletteEntries {
    pub fn read_data(data: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(Cursor::new(data));
        let palette_index = reader.read_u32()?;
        let start = reader.read_u32()?;
        let entry_count = reader.read_u32()? as usize;
        let mut entries = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            entries.push(LogPaletteEntry::read_from(&mut reader)?);
        }
        Ok(Self {
            palette_index,
            start,
            entries,
        })
    }

    pub fn to_data(&self) -> Result<Vec<u8>> {
        let mut writer = Writer::new(Cursor::new(Vec::with_capacity(12 + self.entries.len() * 4)));
        writer.write_u32(self.palette_index)?;
        writer.write_u32(self.start)?;
        writer.write_u32(usize_to_u32(
            self.entries.len(),
            "EMR_SETPALETTEENTRIES entry count",
        )?)?;
        for entry in &self.entries {
            entry.write_to(&mut writer)?;
        }
        Ok(writer.into_inner().into_inner())
    }
}

impl EmrEof {
    pub fn read_data(data: &[u8]) -> Result<Self> {
        if data.len() < 12 {
            return Err(Error::invalid(8, "EMR_EOF record data is too small"));
        }
        let mut reader = Reader::new(Cursor::new(data));
        let palette_entry_count = reader.read_u32()? as usize;
        let palette_entries_offset = reader.read_u32()?;
        let size_last_start = data
            .len()
            .checked_sub(4)
            .ok_or_else(|| Error::invalid(8, "EMR_EOF record data is too small"))?;
        let size_last = u32::from_le_bytes(
            data[size_last_start..]
                .try_into()
                .expect("slice length checked"),
        );

        if palette_entry_count == 0 {
            return Ok(Self {
                palette_entries_offset,
                palette_prefix: data[8..size_last_start].to_vec(),
                palette_entries: Vec::new(),
                palette_suffix: Vec::new(),
                size_last,
            });
        }

        let entries_start = record_relative_data_offset(palette_entries_offset as usize)?;
        if entries_start < 8 {
            return Err(Error::invalid(
                8,
                "EMR_EOF palette entries overlap fixed fields",
            ));
        }
        let entries_len = palette_entry_count
            .checked_mul(4)
            .ok_or_else(|| Error::invalid(8, "EMR_EOF palette entry size overflows"))?;
        let entries_end = entries_start
            .checked_add(entries_len)
            .ok_or_else(|| Error::invalid(8, "EMR_EOF palette entry range overflows"))?;
        if entries_end > size_last_start {
            return Err(Error::invalid(
                8,
                "EMR_EOF palette entries exceed record payload",
            ));
        }

        let mut entries_reader = Reader::new(Cursor::new(&data[entries_start..entries_end]));
        let mut palette_entries = Vec::with_capacity(palette_entry_count);
        for _ in 0..palette_entry_count {
            palette_entries.push(LogPaletteEntry::read_from(&mut entries_reader)?);
        }

        Ok(Self {
            palette_entries_offset,
            palette_prefix: data[8..entries_start].to_vec(),
            palette_entries,
            palette_suffix: data[entries_end..size_last_start].to_vec(),
            size_last,
        })
    }

    pub fn to_data(&self) -> Result<Vec<u8>> {
        let mut writer = Writer::new(Cursor::new(Vec::with_capacity(
            12 + self.palette_prefix.len() + self.palette_entries.len() * 4,
        )));
        writer.write_u32(usize_to_u32(
            self.palette_entries.len(),
            "EMR_EOF palette entry count",
        )?)?;
        writer.write_u32(self.palette_entries_offset)?;

        if self.palette_entries.is_empty() {
            writer.write_all(&self.palette_prefix)?;
        } else {
            let entries_start = record_relative_data_offset(self.palette_entries_offset as usize)?;
            if entries_start < 8 {
                return Err(Error::invalid(
                    8,
                    "EMR_EOF palette entries overlap fixed fields",
                ));
            }
            let prefix_target_len = entries_start - 8;
            if self.palette_prefix.len() > prefix_target_len {
                return Err(Error::invalid(
                    8,
                    "EMR_EOF palette prefix exceeds palette entry offset",
                ));
            }
            writer.write_all(&self.palette_prefix)?;
            writer.write_all(&vec![0; prefix_target_len - self.palette_prefix.len()])?;
            for entry in &self.palette_entries {
                entry.write_to(&mut writer)?;
            }
        }

        writer.write_all(&self.palette_suffix)?;
        writer.write_u32(self.size_last)?;
        Ok(writer.into_inner().into_inner())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSetWindowExtEx {
    pub size: SizeL,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSetWindowOrgEx {
    pub origin: PointL,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSetViewportExtEx {
    pub size: SizeL,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSetViewportOrgEx {
    pub origin: PointL,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSetBrushOrgEx {
    pub origin: PointL,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSetPixelV {
    pub pixel: PointL,
    pub color: ColorRef,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSetMapperFlags {
    pub flags: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSetMapMode {
    pub map_mode: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSetBkMode {
    pub background_mode: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSetPolyFillMode {
    pub polygon_fill_mode: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSetRop2 {
    pub rop2_mode: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSetStretchBltMode {
    pub stretch_mode: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSetTextAlign {
    pub text_alignment_mode: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSetTextColor {
    pub color: ColorRef,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSetBkColor {
    pub color: ColorRef,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrOffsetClipRgn {
    pub offset: PointL,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrExcludeClipRect {
    pub rect: RectL,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrIntersectClipRect {
    pub rect: RectL,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrScaleViewportExtEx {
    pub x_num: i32,
    pub x_denom: i32,
    pub y_num: i32,
    pub y_denom: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrScaleWindowExtEx {
    pub x_num: i32,
    pub x_denom: i32,
    pub y_num: i32,
    pub y_denom: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrRestoreDc {
    pub saved_dc: i32,
}

#[derive(Clone, Debug, PartialEq, SdkObject)]
pub struct EmrSetWorldTransform {
    pub transform: XForm,
}

#[derive(Clone, Debug, PartialEq, SdkObject)]
pub struct EmrModifyWorldTransform {
    pub transform: XForm,
    pub mode: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSelectObject {
    pub object_index: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSelectPalette {
    pub palette_index: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrResizePalette {
    pub palette_index: u32,
    pub number_of_entries: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrDeleteObject {
    pub object_index: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrLineTo {
    pub point: PointL,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrMoveToEx {
    pub point: PointL,
}

#[derive(Clone, Debug, PartialEq, SdkObject)]
pub struct EmrAngleArc {
    pub center: PointL,
    pub radius: u32,
    pub start_angle: f32,
    pub sweep_angle: f32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrRoundRect {
    pub bounds: RectL,
    pub corner: SizeL,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrArc {
    pub box_bounds: RectL,
    pub start: PointL,
    pub end: PointL,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrExtFloodFill {
    pub start: PointL,
    pub color: ColorRef,
    pub flood_fill_mode: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSetArcDirection {
    pub arc_direction: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSetMiterLimit {
    pub miter_limit: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSelectClipPath {
    pub region_mode: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrFillPath {
    pub bounds: RectL,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrStrokeAndFillPath {
    pub bounds: RectL,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrStrokePath {
    pub bounds: RectL,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrCreatePen {
    pub object_index: u32,
    pub pen_style: u32,
    pub width: PointL,
    pub color: ColorRef,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrCreateBrushIndirect {
    pub object_index: u32,
    pub brush_style: u32,
    pub color: ColorRef,
    pub brush_hatch: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmrExtCreatePen {
    pub object_index: u32,
    pub bitmap_info_offset: u32,
    pub bitmap_info_size: u32,
    pub bitmap_bits_offset: u32,
    pub bitmap_bits_size: u32,
    pub pen_style: u32,
    pub width: u32,
    pub brush_style: u32,
    pub color: ColorRef,
    pub brush_hatch: u32,
    pub extension: Vec<u8>,
}

impl EmrExtCreatePen {
    pub fn read_data(data: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(Cursor::new(data));
        let value = Self {
            object_index: reader.read_u32()?,
            bitmap_info_offset: reader.read_u32()?,
            bitmap_info_size: reader.read_u32()?,
            bitmap_bits_offset: reader.read_u32()?,
            bitmap_bits_size: reader.read_u32()?,
            pen_style: reader.read_u32()?,
            width: reader.read_u32()?,
            brush_style: reader.read_u32()?,
            color: ColorRef::read_from(&mut reader)?,
            brush_hatch: reader.read_u32()?,
            extension: Vec::new(),
        };
        let position = reader.position()? as usize;
        Ok(Self {
            extension: data[position..].to_vec(),
            ..value
        })
    }

    pub fn to_data(&self) -> Result<Vec<u8>> {
        let mut writer = Writer::new(Cursor::new(Vec::with_capacity(40 + self.extension.len())));
        writer.write_u32(self.object_index)?;
        writer.write_u32(self.bitmap_info_offset)?;
        writer.write_u32(self.bitmap_info_size)?;
        writer.write_u32(self.bitmap_bits_offset)?;
        writer.write_u32(self.bitmap_bits_size)?;
        writer.write_u32(self.pen_style)?;
        writer.write_u32(self.width)?;
        writer.write_u32(self.brush_style)?;
        self.color.write_to(&mut writer)?;
        writer.write_u32(self.brush_hatch)?;
        writer.write_all(&self.extension)?;
        Ok(writer.into_inner().into_inner())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrRectangle {
    pub bounds: RectL,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrEllipse {
    pub bounds: RectL,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogFontW {
    pub height: i32,
    pub width: i32,
    pub escapement: i32,
    pub orientation: i32,
    pub weight: i32,
    pub italic: u8,
    pub underline: u8,
    pub strike_out: u8,
    pub char_set: u8,
    pub out_precision: u8,
    pub clip_precision: u8,
    pub quality: u8,
    pub pitch_and_family: u8,
    pub face_name: SdkString,
}

impl LogFontW {
    pub fn read_from<R: std::io::Read + std::io::Seek>(reader: &mut Reader<R>) -> Result<Self> {
        Ok(Self {
            height: reader.read_i32()?,
            width: reader.read_i32()?,
            escapement: reader.read_i32()?,
            orientation: reader.read_i32()?,
            weight: reader.read_i32()?,
            italic: reader.read_u8()?,
            underline: reader.read_u8()?,
            strike_out: reader.read_u8()?,
            char_set: reader.read_u8()?,
            out_precision: reader.read_u8()?,
            clip_precision: reader.read_u8()?,
            quality: reader.read_u8()?,
            pitch_and_family: reader.read_u8()?,
            face_name: SdkString::read_bytes(
                reader,
                LOGFONT_FACE_NAME_CHARS * 2,
                SdkEncoding::Utf16Le,
            )?,
        })
    }

    pub fn write_to<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut Writer<W>,
    ) -> Result<()> {
        writer.write_i32(self.height)?;
        writer.write_i32(self.width)?;
        writer.write_i32(self.escapement)?;
        writer.write_i32(self.orientation)?;
        writer.write_i32(self.weight)?;
        writer.write_u8(self.italic)?;
        writer.write_u8(self.underline)?;
        writer.write_u8(self.strike_out)?;
        writer.write_u8(self.char_set)?;
        writer.write_u8(self.out_precision)?;
        writer.write_u8(self.clip_precision)?;
        writer.write_u8(self.quality)?;
        writer.write_u8(self.pitch_and_family)?;
        let bytes = self.face_name.encoded_bytes()?;
        write_fixed_bytes(writer, &bytes, LOGFONT_FACE_NAME_CHARS * 2)
    }

    pub fn sdk_size(&self) -> u64 {
        92
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmrExtCreateFontIndirectW {
    pub object_index: u32,
    pub log_font: LogFontW,
    pub extension: Vec<u8>,
}

impl EmrExtCreateFontIndirectW {
    pub fn read_data(data: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(Cursor::new(data));
        let object_index = reader.read_u32()?;
        let log_font = LogFontW::read_from(&mut reader)?;
        let position = reader.position()? as usize;
        Ok(Self {
            object_index,
            log_font,
            extension: data[position..].to_vec(),
        })
    }

    pub fn to_data(&self) -> Result<Vec<u8>> {
        let mut writer = Writer::new(Cursor::new(Vec::with_capacity(
            4 + self.log_font.sdk_size() as usize + self.extension.len(),
        )));
        writer.write_u32(self.object_index)?;
        self.log_font.write_to(&mut writer)?;
        writer.write_all(&self.extension)?;
        Ok(writer.into_inner().into_inner())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmrPolyPointsL {
    pub bounds: RectL,
    pub points: Vec<PointL>,
}

impl EmrPolyPointsL {
    pub fn read_data(data: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(Cursor::new(data));
        let bounds = RectL::read_from(&mut reader)?;
        let count = reader.read_u32()? as usize;
        let mut points = Vec::with_capacity(count);
        for _ in 0..count {
            points.push(PointL::read_from(&mut reader)?);
        }
        Ok(Self { bounds, points })
    }

    pub fn to_data(&self) -> Result<Vec<u8>> {
        let mut writer = Writer::new(Cursor::new(Vec::with_capacity(20 + self.points.len() * 8)));
        self.bounds.write_to(&mut writer)?;
        writer.write_u32(usize_to_u32(self.points.len(), "EMF point count")?)?;
        for point in &self.points {
            point.write_to(&mut writer)?;
        }
        Ok(writer.into_inner().into_inner())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmrPolyPointsS {
    pub bounds: RectL,
    pub points: Vec<crate::types::PointS>,
}

impl EmrPolyPointsS {
    pub fn read_data(data: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(Cursor::new(data));
        let bounds = RectL::read_from(&mut reader)?;
        let count = reader.read_u32()? as usize;
        let mut points = Vec::with_capacity(count);
        for _ in 0..count {
            points.push(crate::types::PointS::read_from(&mut reader)?);
        }
        Ok(Self { bounds, points })
    }

    pub fn to_data(&self) -> Result<Vec<u8>> {
        let mut writer = Writer::new(Cursor::new(Vec::with_capacity(20 + self.points.len() * 4)));
        self.bounds.write_to(&mut writer)?;
        writer.write_u32(usize_to_u32(self.points.len(), "EMF point count")?)?;
        for point in &self.points {
            point.write_to(&mut writer)?;
        }
        Ok(writer.into_inner().into_inner())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmrPolyPolygonL {
    pub bounds: RectL,
    pub counts: Vec<u32>,
    pub points: Vec<PointL>,
}

impl EmrPolyPolygonL {
    pub fn read_data(data: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(Cursor::new(data));
        let bounds = RectL::read_from(&mut reader)?;
        let polygon_count = reader.read_u32()? as usize;
        let point_count = reader.read_u32()? as usize;
        let mut counts = Vec::with_capacity(polygon_count);
        for _ in 0..polygon_count {
            counts.push(reader.read_u32()?);
        }
        let mut points = Vec::with_capacity(point_count);
        for _ in 0..point_count {
            points.push(PointL::read_from(&mut reader)?);
        }
        Ok(Self {
            bounds,
            counts,
            points,
        })
    }

    pub fn to_data(&self) -> Result<Vec<u8>> {
        let mut writer = Writer::new(Cursor::new(Vec::with_capacity(
            24 + self.counts.len() * 4 + self.points.len() * 8,
        )));
        self.bounds.write_to(&mut writer)?;
        writer.write_u32(usize_to_u32(self.counts.len(), "EMF polygon count")?)?;
        writer.write_u32(usize_to_u32(self.points.len(), "EMF total point count")?)?;
        for count in &self.counts {
            writer.write_u32(*count)?;
        }
        for point in &self.points {
            point.write_to(&mut writer)?;
        }
        Ok(writer.into_inner().into_inner())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmrPolyPolygonS {
    pub bounds: RectL,
    pub counts: Vec<u32>,
    pub points: Vec<crate::types::PointS>,
}

impl EmrPolyPolygonS {
    pub fn read_data(data: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(Cursor::new(data));
        let bounds = RectL::read_from(&mut reader)?;
        let polygon_count = reader.read_u32()? as usize;
        let point_count = reader.read_u32()? as usize;
        let mut counts = Vec::with_capacity(polygon_count);
        for _ in 0..polygon_count {
            counts.push(reader.read_u32()?);
        }
        let mut points = Vec::with_capacity(point_count);
        for _ in 0..point_count {
            points.push(crate::types::PointS::read_from(&mut reader)?);
        }
        Ok(Self {
            bounds,
            counts,
            points,
        })
    }

    pub fn to_data(&self) -> Result<Vec<u8>> {
        let mut writer = Writer::new(Cursor::new(Vec::with_capacity(
            24 + self.counts.len() * 4 + self.points.len() * 4,
        )));
        self.bounds.write_to(&mut writer)?;
        writer.write_u32(usize_to_u32(self.counts.len(), "EMF polygon count")?)?;
        writer.write_u32(usize_to_u32(self.points.len(), "EMF total point count")?)?;
        for count in &self.counts {
            writer.write_u32(*count)?;
        }
        for point in &self.points {
            point.write_to(&mut writer)?;
        }
        Ok(writer.into_inner().into_inner())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmrText {
    pub reference: PointL,
    pub options: ExtTextOutOptions,
    pub rectangle: Option<RectL>,
    pub text: SdkString,
    pub dx: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmrExtTextOut {
    pub bounds: RectL,
    pub graphics_mode: u32,
    pub ex_scale: f32,
    pub ey_scale: f32,
    pub text: EmrText,
}

impl EmrExtTextOut {
    pub fn read_data(data: &[u8], wide: bool) -> Result<Self> {
        let mut reader = Reader::new(Cursor::new(data));
        let bounds = RectL::read_from(&mut reader)?;
        let graphics_mode = reader.read_u32()?;
        let ex_scale = reader.read_f32()?;
        let ey_scale = reader.read_f32()?;
        let reference = PointL::read_from(&mut reader)?;
        let chars = reader.read_u32()? as usize;
        let string_offset = reader.read_u32()? as usize;
        let options = ExtTextOutOptions::from_bits_retain(reader.read_u32()?);
        let rectangle =
            if options.intersects(ExtTextOutOptions::OPAQUE | ExtTextOutOptions::CLIPPED) {
                Some(RectL::read_from(&mut reader)?)
            } else {
                None
            };
        let dx_offset = reader.read_u32()? as usize;

        let string_len = chars
            .checked_mul(if wide { 2 } else { 1 })
            .ok_or_else(|| Error::invalid(0, "EMR_EXTTEXTOUT string length overflows"))?;
        let string_start = record_relative_data_offset(string_offset)?;
        let string_end = string_start
            .checked_add(string_len)
            .ok_or_else(|| Error::invalid(0, "EMR_EXTTEXTOUT string range overflows"))?;
        let text = SdkString::raw(
            data.get(string_start..string_end)
                .ok_or_else(|| Error::invalid(0, "EMR_EXTTEXTOUT string range is out of bounds"))?
                .to_vec(),
            if wide {
                SdkEncoding::Utf16Le
            } else {
                SdkEncoding::Windows1252
            },
        );

        let dx = if dx_offset == 0 {
            Vec::new()
        } else {
            let dx_count = chars
                .checked_mul(if options.contains(ExtTextOutOptions::PDY) {
                    2
                } else {
                    1
                })
                .ok_or_else(|| Error::invalid(0, "EMR_EXTTEXTOUT dx count overflows"))?;
            let mut dx_reader = Reader::new(Cursor::new(
                data.get(record_relative_data_offset(dx_offset)?..)
                    .ok_or_else(|| {
                        Error::invalid(0, "EMR_EXTTEXTOUT dx offset is out of bounds")
                    })?,
            ));
            let mut values = Vec::with_capacity(dx_count);
            for _ in 0..dx_count {
                values.push(dx_reader.read_u32()?);
            }
            values
        };

        Ok(Self {
            bounds,
            graphics_mode,
            ex_scale,
            ey_scale,
            text: EmrText {
                reference,
                options,
                rectangle,
                text,
                dx,
            },
        })
    }

    pub fn to_data(&self, wide: bool) -> Result<Vec<u8>> {
        let text_bytes = self.text.text.encoded_bytes()?;
        let has_rect = self.text.rectangle.is_some();
        let fixed_size = 16 + 4 + 4 + 4 + 8 + 4 + 4 + 4 + if has_rect { 16 } else { 0 } + 4;
        let string_offset = 8 + fixed_size;
        let dx_offset = if self.text.dx.is_empty() {
            0
        } else {
            align_to_u32(string_offset + text_bytes.len())
        };
        let mut writer = Writer::new(Cursor::new(Vec::with_capacity(
            fixed_size + text_bytes.len() + self.text.dx.len() * 4 + 4,
        )));
        self.bounds.write_to(&mut writer)?;
        writer.write_u32(self.graphics_mode)?;
        writer.write_f32(self.ex_scale)?;
        writer.write_f32(self.ey_scale)?;
        self.text.reference.write_to(&mut writer)?;
        let char_count = if wide {
            text_bytes.len() / 2
        } else {
            text_bytes.len()
        };
        writer.write_u32(usize_to_u32(char_count, "EMR_EXTTEXTOUT character count")?)?;
        writer.write_u32(usize_to_u32(string_offset, "EMR_EXTTEXTOUT string offset")?)?;
        writer.write_u32(self.text.options.bits())?;
        if let Some(rectangle) = &self.text.rectangle {
            rectangle.write_to(&mut writer)?;
        }
        writer.write_u32(usize_to_u32(dx_offset, "EMR_EXTTEXTOUT dx offset")?)?;
        writer.write_all(&text_bytes)?;
        if dx_offset != 0 {
            pad_writer_to_record_offset(&mut writer, dx_offset)?;
            for value in &self.text.dx {
                writer.write_u32(*value)?;
            }
        }
        pad_writer_to_4(&mut writer)?;
        Ok(writer.into_inner().into_inner())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmrBitmapBuffer {
    pub bitmap_info: Vec<u8>,
    pub bitmap_bits: Vec<u8>,
}

impl EmrBitmapBuffer {
    pub fn dib_info(&self) -> Result<DibBitmapInfo> {
        DibBitmapInfo::read_from_slice(&self.bitmap_info)
    }

    pub fn device_independent_bitmap(&self) -> Result<DeviceIndependentBitmap> {
        DeviceIndependentBitmap::from_parts(&self.bitmap_info, &self.bitmap_bits)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmrCreateMonoBrush {
    pub brush_index: u32,
    pub color_usage: u32,
    pub bitmap: EmrBitmapBuffer,
}

impl EmrCreateMonoBrush {
    pub fn color_usage_kind(&self) -> Option<DibColorUsage> {
        DibColorUsage::from_raw(self.color_usage)
    }

    pub fn read_data(data: &[u8]) -> Result<Self> {
        let (brush_index, color_usage, bitmap) = read_dib_brush_data(data)?;
        Ok(Self {
            brush_index,
            color_usage,
            bitmap,
        })
    }

    pub fn to_data(&self) -> Result<Vec<u8>> {
        write_dib_brush_data(
            self.brush_index,
            self.color_usage,
            &self.bitmap,
            "EMR_CREATEMONOBRUSH",
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmrCreateDibPatternBrushPt {
    pub brush_index: u32,
    pub color_usage: u32,
    pub bitmap: EmrBitmapBuffer,
}

impl EmrCreateDibPatternBrushPt {
    pub fn color_usage_kind(&self) -> Option<DibColorUsage> {
        DibColorUsage::from_raw(self.color_usage)
    }

    pub fn read_data(data: &[u8]) -> Result<Self> {
        let (brush_index, color_usage, bitmap) = read_dib_brush_data(data)?;
        Ok(Self {
            brush_index,
            color_usage,
            bitmap,
        })
    }

    pub fn to_data(&self) -> Result<Vec<u8>> {
        write_dib_brush_data(
            self.brush_index,
            self.color_usage,
            &self.bitmap,
            "EMR_CREATEDIBPATTERNBRUSHPT",
        )
    }
}

fn read_dib_brush_data(data: &[u8]) -> Result<(u32, u32, EmrBitmapBuffer)> {
    let mut reader = Reader::new(Cursor::new(data));
    let brush_index = reader.read_u32()?;
    let color_usage = reader.read_u32()?;
    let off_bmi = reader.read_u32()? as usize;
    let cb_bmi = reader.read_u32()? as usize;
    let off_bits = reader.read_u32()? as usize;
    let cb_bits = reader.read_u32()? as usize;
    Ok((
        brush_index,
        color_usage,
        read_bitmap_buffer(data, off_bmi, cb_bmi, off_bits, cb_bits)?,
    ))
}

fn write_dib_brush_data(
    brush_index: u32,
    color_usage: u32,
    bitmap: &EmrBitmapBuffer,
    record_name: &str,
) -> Result<Vec<u8>> {
    let fixed = 24usize;
    let off_bmi = 8 + fixed;
    let off_bits = align_to_u32(off_bmi + bitmap.bitmap_info.len());
    let mut writer = Writer::new(Cursor::new(Vec::with_capacity(
        fixed + bitmap.bitmap_info.len() + bitmap.bitmap_bits.len() + 4,
    )));
    writer.write_u32(brush_index)?;
    writer.write_u32(color_usage)?;
    writer.write_u32(usize_to_u32(
        off_bmi,
        format!("{record_name} bitmap info offset"),
    )?)?;
    writer.write_u32(usize_to_u32(bitmap.bitmap_info.len(), "bitmap info size")?)?;
    writer.write_u32(usize_to_u32(
        off_bits,
        format!("{record_name} bitmap bits offset"),
    )?)?;
    writer.write_u32(usize_to_u32(bitmap.bitmap_bits.len(), "bitmap bits size")?)?;
    writer.write_all(&bitmap.bitmap_info)?;
    pad_writer_to_record_offset(&mut writer, off_bits)?;
    writer.write_all(&bitmap.bitmap_bits)?;
    pad_writer_to_4(&mut writer)?;
    Ok(writer.into_inner().into_inner())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmrSetDiBitsToDevice {
    pub bounds: RectL,
    pub dest: PointL,
    pub source: BitmapSourceBounds,
    pub color_usage: u32,
    pub start_scan: u32,
    pub scan_lines: u32,
    pub bitmap: EmrBitmapBuffer,
}

impl EmrSetDiBitsToDevice {
    pub fn color_usage_kind(&self) -> Option<DibColorUsage> {
        DibColorUsage::from_raw(self.color_usage)
    }

    pub fn read_data(data: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(Cursor::new(data));
        let bounds = RectL::read_from(&mut reader)?;
        let dest = PointL::read_from(&mut reader)?;
        let source = BitmapSourceBounds::read_from(&mut reader)?;
        let off_bmi = reader.read_u32()? as usize;
        let cb_bmi = reader.read_u32()? as usize;
        let off_bits = reader.read_u32()? as usize;
        let cb_bits = reader.read_u32()? as usize;
        let color_usage = reader.read_u32()?;
        let start_scan = reader.read_u32()?;
        let scan_lines = reader.read_u32()?;
        Ok(Self {
            bounds,
            dest,
            source,
            color_usage,
            start_scan,
            scan_lines,
            bitmap: read_bitmap_buffer(data, off_bmi, cb_bmi, off_bits, cb_bits)?,
        })
    }

    pub fn to_data(&self) -> Result<Vec<u8>> {
        let fixed = 68usize;
        let off_bmi = 8 + fixed;
        let off_bits = align_to_u32(off_bmi + self.bitmap.bitmap_info.len());
        let mut writer = Writer::new(Cursor::new(Vec::with_capacity(
            fixed + self.bitmap.bitmap_info.len() + self.bitmap.bitmap_bits.len() + 4,
        )));
        self.bounds.write_to(&mut writer)?;
        self.dest.write_to(&mut writer)?;
        self.source.write_to(&mut writer)?;
        writer.write_u32(usize_to_u32(
            off_bmi,
            "EMR_SETDIBITSTODEVICE bitmap info offset",
        )?)?;
        writer.write_u32(usize_to_u32(
            self.bitmap.bitmap_info.len(),
            "bitmap info size",
        )?)?;
        writer.write_u32(usize_to_u32(
            off_bits,
            "EMR_SETDIBITSTODEVICE bitmap bits offset",
        )?)?;
        writer.write_u32(usize_to_u32(
            self.bitmap.bitmap_bits.len(),
            "bitmap bits size",
        )?)?;
        writer.write_u32(self.color_usage)?;
        writer.write_u32(self.start_scan)?;
        writer.write_u32(self.scan_lines)?;
        writer.write_all(&self.bitmap.bitmap_info)?;
        pad_writer_to_record_offset(&mut writer, off_bits)?;
        writer.write_all(&self.bitmap.bitmap_bits)?;
        pad_writer_to_4(&mut writer)?;
        Ok(writer.into_inner().into_inner())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmrStretchDiBits {
    pub bounds: RectL,
    pub dest: PointL,
    pub source: BitmapSourceBounds,
    pub color_usage: u32,
    pub raster_operation: u32,
    pub dest_size: SizeL,
    pub bitmap: EmrBitmapBuffer,
}

impl EmrStretchDiBits {
    pub fn color_usage_kind(&self) -> Option<DibColorUsage> {
        DibColorUsage::from_raw(self.color_usage)
    }

    pub fn read_data(data: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(Cursor::new(data));
        let bounds = RectL::read_from(&mut reader)?;
        let dest = PointL::read_from(&mut reader)?;
        let source = BitmapSourceBounds::read_from(&mut reader)?;
        let off_bmi = reader.read_u32()? as usize;
        let cb_bmi = reader.read_u32()? as usize;
        let off_bits = reader.read_u32()? as usize;
        let cb_bits = reader.read_u32()? as usize;
        let color_usage = reader.read_u32()?;
        let raster_operation = reader.read_u32()?;
        let dest_size = SizeL::read_from(&mut reader)?;
        Ok(Self {
            bounds,
            dest,
            source,
            color_usage,
            raster_operation,
            dest_size,
            bitmap: read_bitmap_buffer(data, off_bmi, cb_bmi, off_bits, cb_bits)?,
        })
    }

    pub fn to_data(&self) -> Result<Vec<u8>> {
        let fixed = 72usize;
        let off_bmi = 8 + fixed;
        let off_bits = align_to_u32(off_bmi + self.bitmap.bitmap_info.len());
        let mut writer = Writer::new(Cursor::new(Vec::with_capacity(
            fixed + self.bitmap.bitmap_info.len() + self.bitmap.bitmap_bits.len() + 4,
        )));
        self.bounds.write_to(&mut writer)?;
        self.dest.write_to(&mut writer)?;
        self.source.write_to(&mut writer)?;
        writer.write_u32(usize_to_u32(
            off_bmi,
            "EMR_STRETCHDIBITS bitmap info offset",
        )?)?;
        writer.write_u32(usize_to_u32(
            self.bitmap.bitmap_info.len(),
            "bitmap info size",
        )?)?;
        writer.write_u32(usize_to_u32(
            off_bits,
            "EMR_STRETCHDIBITS bitmap bits offset",
        )?)?;
        writer.write_u32(usize_to_u32(
            self.bitmap.bitmap_bits.len(),
            "bitmap bits size",
        )?)?;
        writer.write_u32(self.color_usage)?;
        writer.write_u32(self.raster_operation)?;
        self.dest_size.write_to(&mut writer)?;
        writer.write_all(&self.bitmap.bitmap_info)?;
        pad_writer_to_record_offset(&mut writer, off_bits)?;
        writer.write_all(&self.bitmap.bitmap_bits)?;
        pad_writer_to_4(&mut writer)?;
        Ok(writer.into_inner().into_inner())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "emf")]
pub struct BitmapSourceBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSetIcmMode {
    pub icm_mode: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSetColorSpace {
    pub color_space_index: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrDeleteColorSpace {
    pub color_space_index: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSetLayout {
    pub layout_mode: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
pub struct EmrSetTextJustification {
    pub break_extra: i32,
    pub break_count: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EmrComment {
    EmfPlus {
        records: Vec<crate::emfplus::EmfPlusRecord>,
    },
    Raw {
        data_size: u32,
        identifier: u32,
        data: Vec<u8>,
    },
}

impl EmrComment {
    pub fn read_data(data: &[u8]) -> Result<Self> {
        if data.len() < 8 {
            return Err(Error::invalid(0, "EMR_COMMENT data is too small"));
        }
        let mut reader = Reader::new(Cursor::new(data));
        let data_size = reader.read_u32()?;
        let identifier = reader.read_u32()?;
        if data_size < 4 {
            return Err(Error::invalid(
                0,
                "EMR_COMMENT data size is smaller than its identifier",
            ));
        }
        let payload_len = data_size as usize - 4;
        let payload = data
            .get(8..8 + payload_len)
            .ok_or_else(|| Error::invalid(0, "EMR_COMMENT payload is out of bounds"))?;
        if identifier == EMR_COMMENT_EMFPLUS {
            Ok(Self::EmfPlus {
                records: crate::emfplus::read_records(payload)?,
            })
        } else {
            Ok(Self::Raw {
                data_size,
                identifier,
                data: payload.to_vec(),
            })
        }
    }

    pub fn to_data(&self) -> Result<Vec<u8>> {
        match self {
            Self::EmfPlus { records } => {
                let mut payload = Vec::new();
                for record in records {
                    let mut writer = Writer::new(Cursor::new(Vec::new()));
                    record.write_to(&mut writer)?;
                    payload.extend_from_slice(&writer.into_inner().into_inner());
                }
                let mut writer = Writer::new(Cursor::new(Vec::with_capacity(8 + payload.len())));
                writer.write_u32(usize_to_u32(payload.len() + 4, "EMR_COMMENT data size")?)?;
                writer.write_u32(EMR_COMMENT_EMFPLUS)?;
                writer.write_all(&payload)?;
                pad_writer_to_4(&mut writer)?;
                Ok(writer.into_inner().into_inner())
            }
            Self::Raw {
                data_size,
                identifier,
                data,
            } => {
                let mut writer = Writer::new(Cursor::new(Vec::with_capacity(8 + data.len())));
                writer.write_u32(*data_size)?;
                writer.write_u32(*identifier)?;
                writer.write_all(data)?;
                pad_writer_to_4(&mut writer)?;
                Ok(writer.into_inner().into_inner())
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmfHeader {
    pub bounds: RectL,
    pub frame: RectL,
    pub signature: u32,
    pub version: u32,
    pub bytes: u32,
    pub records: u32,
    pub handles: u16,
    pub reserved: u16,
    pub description_chars: u32,
    pub description_offset: u32,
    pub palette_entries: u32,
    pub device: SizeL,
    pub millimeters: SizeL,
    pub extension: Vec<u8>,
}

impl EmfHeader {
    pub fn from_record_data(data: &[u8]) -> Result<Self> {
        if data.len() < (EMF_HEADER_MIN_SIZE as usize - 8) {
            return Err(Error::invalid(8, "EMR_HEADER record data is too small"));
        }
        let mut reader = Reader::new(Cursor::new(data));
        let mut header = Self::read_from(&mut reader)?;
        let position = reader.position()? as usize;
        header.extension = data[position..].to_vec();
        Ok(header)
    }

    pub fn to_record_data(&self) -> Result<Vec<u8>> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        self.write_to(&mut writer)?;
        Ok(writer.into_inner().into_inner())
    }
}

impl SdkRead for EmfHeader {
    fn read_from<R: std::io::Read + std::io::Seek>(reader: &mut Reader<R>) -> Result<Self> {
        Ok(Self {
            bounds: RectL::read_from(reader)?,
            frame: RectL::read_from(reader)?,
            signature: reader.read_u32()?,
            version: reader.read_u32()?,
            bytes: reader.read_u32()?,
            records: reader.read_u32()?,
            handles: reader.read_u16()?,
            reserved: reader.read_u16()?,
            description_chars: reader.read_u32()?,
            description_offset: reader.read_u32()?,
            palette_entries: reader.read_u32()?,
            device: SizeL::read_from(reader)?,
            millimeters: SizeL::read_from(reader)?,
            extension: Vec::new(),
        })
    }
}

impl SdkWrite for EmfHeader {
    fn write_to<W: std::io::Write + std::io::Seek>(&self, writer: &mut Writer<W>) -> Result<()> {
        self.bounds.write_to(writer)?;
        self.frame.write_to(writer)?;
        writer.write_u32(self.signature)?;
        writer.write_u32(self.version)?;
        writer.write_u32(self.bytes)?;
        writer.write_u32(self.records)?;
        writer.write_u16(self.handles)?;
        writer.write_u16(self.reserved)?;
        writer.write_u32(self.description_chars)?;
        writer.write_u32(self.description_offset)?;
        writer.write_u32(self.palette_entries)?;
        self.device.write_to(writer)?;
        self.millimeters.write_to(writer)?;
        writer.write_all(&self.extension)
    }
}

impl SdkSize for EmfHeader {
    fn sdk_size(&self) -> u64 {
        80 + self.extension.len() as u64
    }
}

fn read_object<T: SdkRead>(data: &[u8]) -> Result<T> {
    let mut reader = Reader::new(Cursor::new(data));
    T::read_from(&mut reader)
}

fn object_record<T: SdkWrite>(record_type: EmfRecordType, value: &T) -> Result<EmfRecord> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    value.write_to(&mut writer)?;
    Ok(EmfRecord::new(
        record_type.raw(),
        writer.into_inner().into_inner(),
    ))
}

fn no_data_record(record_type: EmfRecordType) -> EmfRecord {
    EmfRecord::new(record_type.raw(), Vec::new())
}

fn ensure_no_data(data: &[u8], record_name: &str) -> Result<()> {
    if data.is_empty() {
        Ok(())
    } else {
        Err(Error::invalid(
            8,
            format!("{record_name} record data must be empty"),
        ))
    }
}

fn write_fixed_bytes<W: std::io::Write + std::io::Seek>(
    writer: &mut Writer<W>,
    bytes: &[u8],
    len: usize,
) -> Result<()> {
    if bytes.len() >= len {
        writer.write_all(&bytes[..len])
    } else {
        writer.write_all(bytes)?;
        writer.write_all(&vec![0; len - bytes.len()])
    }
}

fn record_relative_data_offset(offset: usize) -> Result<usize> {
    offset
        .checked_sub(8)
        .ok_or_else(|| Error::invalid(0, "record-relative offset points into record header"))
}

fn read_bitmap_buffer(
    data: &[u8],
    off_bmi: usize,
    cb_bmi: usize,
    off_bits: usize,
    cb_bits: usize,
) -> Result<EmrBitmapBuffer> {
    let bmi_start = record_relative_data_offset(off_bmi)?;
    let bits_start = record_relative_data_offset(off_bits)?;
    let bmi_end = bmi_start
        .checked_add(cb_bmi)
        .ok_or_else(|| Error::invalid(0, "bitmap info range overflows"))?;
    let bits_end = bits_start
        .checked_add(cb_bits)
        .ok_or_else(|| Error::invalid(0, "bitmap bits range overflows"))?;
    Ok(EmrBitmapBuffer {
        bitmap_info: data
            .get(bmi_start..bmi_end)
            .ok_or_else(|| Error::invalid(0, "bitmap info range is out of bounds"))?
            .to_vec(),
        bitmap_bits: data
            .get(bits_start..bits_end)
            .ok_or_else(|| Error::invalid(0, "bitmap bits range is out of bounds"))?
            .to_vec(),
    })
}

fn align_to_u32(value: usize) -> usize {
    (value + 3) & !3
}

fn pad_writer_to_4<W: std::io::Write + std::io::Seek>(writer: &mut Writer<W>) -> Result<()> {
    let padding = (4 - (writer.position()? as usize % 4)) % 4;
    if padding != 0 {
        writer.write_all(&[0; 3][..padding])?;
    }
    Ok(())
}

fn pad_writer_to_record_offset<W: std::io::Write + std::io::Seek>(
    writer: &mut Writer<W>,
    record_offset: usize,
) -> Result<()> {
    let current_record_offset = writer
        .position()?
        .checked_add(8)
        .ok_or_else(|| Error::invalid(0, "writer record offset overflows"))?
        as usize;
    if current_record_offset > record_offset {
        return Err(Error::invalid(
            writer.position()?,
            "writer has passed requested record offset",
        ));
    }
    writer.write_all(&vec![0; record_offset - current_record_offset])
}

fn usize_to_u32(value: usize, context: impl std::fmt::Display) -> Result<u32> {
    u32::try_from(value).map_err(|_| Error::invalid(0, format!("{context} exceeds u32::MAX")))
}

pub fn looks_like_emf(bytes: &[u8]) -> bool {
    if bytes.len() < 44 {
        return false;
    }
    let record_type = u32::from_le_bytes(bytes[0..4].try_into().expect("slice length checked"));
    let header_size = u32::from_le_bytes(bytes[4..8].try_into().expect("slice length checked"));
    let signature = u32::from_le_bytes(bytes[40..44].try_into().expect("slice length checked"));
    record_type == EMR_HEADER && header_size >= EMF_HEADER_MIN_SIZE && signature == EMF_SIGNATURE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_emf() -> Vec<u8> {
        let mut bytes = vec![0; 88];
        bytes[0..4].copy_from_slice(&EMR_HEADER.to_le_bytes());
        bytes[4..8].copy_from_slice(&88u32.to_le_bytes());
        bytes[40..44].copy_from_slice(&EMF_SIGNATURE.to_le_bytes());
        bytes.extend_from_slice(&EMR_EOF.to_le_bytes());
        bytes.extend_from_slice(&20u32.to_le_bytes());
        bytes.extend_from_slice(&[0; 12]);
        bytes
    }

    #[test]
    fn emf_roundtrip_preserves_bytes() {
        let bytes = minimal_emf();
        let metafile = EmfMetafile::from_bytes(&bytes).unwrap();
        assert_eq!(metafile.records.len(), 2);
        assert_eq!(metafile.to_bytes().unwrap(), bytes);
    }

    #[test]
    fn detects_emf_signature() {
        assert!(looks_like_emf(&minimal_emf()));
    }

    #[test]
    fn parses_typed_emf_header() {
        let metafile = EmfMetafile::from_bytes(&minimal_emf()).unwrap();
        let header = metafile.header().unwrap().as_header().unwrap().unwrap();
        assert_eq!(header.signature, EMF_SIGNATURE);
        assert_eq!(header.sdk_size(), 80);
        assert_eq!(
            header.to_record_data().unwrap(),
            metafile.header().unwrap().data
        );
    }

    #[test]
    fn maps_emf_record_type_enum() {
        let record = EmfRecord::new(EMR_HEADER, Vec::new());
        assert_eq!(record.record_kind(), Some(EmfRecordType::Header));
        assert_eq!(EmfRecordType::ExtTextOutW.raw(), 0x0000_0054);
    }

    #[test]
    fn derived_emf_object_roundtrips() {
        let value = EmrSetWindowOrgEx {
            origin: PointL { x: -10, y: 20 },
        };
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        value.write_to(&mut writer).unwrap();
        let bytes = writer.into_inner().into_inner();
        assert_eq!(bytes, [246, 255, 255, 255, 20, 0, 0, 0]);
        assert_eq!(value.sdk_size(), 8);

        let mut reader = Reader::new(Cursor::new(bytes));
        let parsed = EmrSetWindowOrgEx::read_from(&mut reader).unwrap();
        assert_eq!(parsed, value);
    }

    #[test]
    fn typed_polygon_record_roundtrips() {
        let value = EmfRecordData::Polygon(EmrPolyPointsL {
            bounds: RectL {
                left: 1,
                top: 2,
                right: 9,
                bottom: 10,
            },
            points: vec![PointL { x: 1, y: 2 }, PointL { x: 3, y: 4 }],
        });

        let record = value.to_record().unwrap();
        assert_eq!(record.record_type, EmfRecordType::Polygon.raw());
        assert_eq!(record.parse_data().unwrap(), value);
    }

    #[test]
    fn typed_simple_state_records_roundtrip() {
        let values = [
            EmfRecordData::SetMapMode(EmrSetMapMode { map_mode: 8 }),
            EmfRecordData::SetBkMode(EmrSetBkMode { background_mode: 2 }),
            EmfRecordData::SetRop2(EmrSetRop2 { rop2_mode: 13 }),
            EmfRecordData::SetArcDirection(EmrSetArcDirection { arc_direction: 2 }),
            EmfRecordData::SetIcmMode(EmrSetIcmMode { icm_mode: 1 }),
            EmfRecordData::SetLayout(EmrSetLayout { layout_mode: 9 }),
        ];

        for value in values {
            let record = value.to_record().unwrap();
            assert_eq!(record.data.len(), 4);
            assert_eq!(record.parse_data().unwrap(), value);
        }
    }

    #[test]
    fn typed_eof_record_preserves_palette_spacing() {
        let value = EmfRecordData::Eof(EmrEof {
            palette_entries_offset: 20,
            palette_prefix: vec![0xAA, 0xBB, 0xCC, 0xDD],
            palette_entries: vec![LogPaletteEntry {
                reserved: 1,
                blue: 2,
                green: 3,
                red: 4,
            }],
            palette_suffix: vec![0xEE, 0xFF, 0x11, 0x22],
            size_last: 32,
        });

        let record = value.to_record().unwrap();
        assert_eq!(record.record_type, EMR_EOF);
        assert_eq!(
            record.data,
            vec![
                1, 0, 0, 0, // nPalEntries
                20, 0, 0, 0, // offPalEntries
                0xAA, 0xBB, 0xCC, 0xDD, // UndefinedSpace1
                1, 2, 3, 4, // LogPaletteEntry
                0xEE, 0xFF, 0x11, 0x22, // UndefinedSpace2
                32, 0, 0, 0, // SizeLast
            ]
        );
        assert_eq!(record.parse_data().unwrap(), value);
    }

    #[test]
    fn typed_palette_object_records_roundtrip() {
        let select = EmfRecordData::SelectPalette(EmrSelectPalette { palette_index: 2 });
        let record = select.to_record().unwrap();
        assert_eq!(record.record_type, EmfRecordType::SelectPalette.raw());
        assert_eq!(record.parse_data().unwrap(), select);

        let resize = EmfRecordData::ResizePalette(EmrResizePalette {
            palette_index: 2,
            number_of_entries: 256,
        });
        let record = resize.to_record().unwrap();
        assert_eq!(record.record_type, EmfRecordType::ResizePalette.raw());
        assert_eq!(record.parse_data().unwrap(), resize);
    }

    #[test]
    fn typed_palette_entry_records_roundtrip() {
        let entries = vec![
            LogPaletteEntry {
                reserved: 0,
                blue: 10,
                green: 20,
                red: 30,
            },
            LogPaletteEntry {
                reserved: 1,
                blue: 40,
                green: 50,
                red: 60,
            },
        ];

        let create = EmfRecordData::CreatePalette(EmrCreatePalette {
            palette_index: 3,
            log_palette: LogPalette {
                version: 0x0300,
                entries: entries.clone(),
            },
        });
        let record = create.to_record().unwrap();
        assert_eq!(record.record_type, EmfRecordType::CreatePalette.raw());
        assert_eq!(record.parse_data().unwrap(), create);

        let set_entries = EmfRecordData::SetPaletteEntries(EmrSetPaletteEntries {
            palette_index: 3,
            start: 4,
            entries,
        });
        let record = set_entries.to_record().unwrap();
        assert_eq!(record.record_type, EmfRecordType::SetPaletteEntries.raw());
        assert_eq!(record.parse_data().unwrap(), set_entries);
    }

    #[test]
    fn typed_scale_and_text_justification_records_roundtrip() {
        let scale = EmfRecordData::ScaleViewportExtEx(EmrScaleViewportExtEx {
            x_num: 2,
            x_denom: 3,
            y_num: -4,
            y_denom: 5,
        });
        let record = scale.to_record().unwrap();
        assert_eq!(record.record_type, EmfRecordType::ScaleViewportExtEx.raw());
        assert_eq!(record.parse_data().unwrap(), scale);

        let justification = EmfRecordData::SetTextJustification(EmrSetTextJustification {
            break_extra: -12,
            break_count: 4,
        });
        let record = justification.to_record().unwrap();
        assert_eq!(
            record.record_type,
            EmfRecordType::SetTextJustification.raw()
        );
        assert_eq!(record.parse_data().unwrap(), justification);
    }

    #[test]
    fn typed_no_parameter_records_roundtrip() {
        let values = [
            EmfRecordData::SaveDc,
            EmfRecordData::RealizePalette,
            EmfRecordData::BeginPath,
            EmfRecordData::EndPath,
            EmfRecordData::CloseFigure,
            EmfRecordData::FlattenPath,
            EmfRecordData::WidenPath,
            EmfRecordData::AbortPath,
        ];

        for value in values {
            let record = value.to_record().unwrap();
            assert!(record.data.is_empty());
            assert_eq!(record.parse_data().unwrap(), value);
        }
    }

    #[test]
    fn typed_poly_polyline16_record_roundtrips() {
        let value = EmfRecordData::PolyPolyline16(EmrPolyPolygonS {
            bounds: RectL {
                left: 0,
                top: 0,
                right: 10,
                bottom: 10,
            },
            counts: vec![2, 3],
            points: vec![
                crate::types::PointS { x: 1, y: 2 },
                crate::types::PointS { x: 3, y: 4 },
                crate::types::PointS { x: 5, y: 6 },
                crate::types::PointS { x: 7, y: 8 },
                crate::types::PointS { x: 9, y: 10 },
            ],
        });

        let record = value.to_record().unwrap();
        assert_eq!(record.record_type, EmfRecordType::PolyPolyline16.raw());
        assert_eq!(record.parse_data().unwrap(), value);
    }

    #[test]
    fn typed_fixed_drawing_records_roundtrip() {
        let arc = EmfRecordData::Arc(EmrArc {
            box_bounds: RectL {
                left: 1,
                top: 2,
                right: 30,
                bottom: 40,
            },
            start: PointL { x: 5, y: 6 },
            end: PointL { x: 7, y: 8 },
        });
        let record = arc.to_record().unwrap();
        assert_eq!(record.record_type, EmfRecordType::Arc.raw());
        assert_eq!(record.parse_data().unwrap(), arc);

        let angle_arc = EmfRecordData::AngleArc(EmrAngleArc {
            center: PointL { x: 10, y: 11 },
            radius: 12,
            start_angle: 45.0,
            sweep_angle: 90.0,
        });
        let record = angle_arc.to_record().unwrap();
        assert_eq!(record.record_type, EmfRecordType::AngleArc.raw());
        assert_eq!(record.parse_data().unwrap(), angle_arc);

        let round_rect = EmfRecordData::RoundRect(EmrRoundRect {
            bounds: RectL {
                left: 0,
                top: 0,
                right: 100,
                bottom: 50,
            },
            corner: SizeL { cx: 8, cy: 10 },
        });
        let record = round_rect.to_record().unwrap();
        assert_eq!(record.record_type, EmfRecordType::RoundRect.raw());
        assert_eq!(record.parse_data().unwrap(), round_rect);
    }

    #[test]
    fn typed_path_bounds_and_flood_fill_records_roundtrip() {
        let bounds = RectL {
            left: -1,
            top: -2,
            right: 20,
            bottom: 30,
        };
        let values = [
            EmfRecordData::FillPath(EmrFillPath { bounds }),
            EmfRecordData::StrokeAndFillPath(EmrStrokeAndFillPath { bounds }),
            EmfRecordData::StrokePath(EmrStrokePath { bounds }),
        ];

        for value in values {
            let record = value.to_record().unwrap();
            assert_eq!(record.data.len(), 16);
            assert_eq!(record.parse_data().unwrap(), value);
        }

        let flood_fill = EmfRecordData::ExtFloodFill(EmrExtFloodFill {
            start: PointL { x: 3, y: 4 },
            color: ColorRef {
                red: 10,
                green: 20,
                blue: 30,
                reserved: 0,
            },
            flood_fill_mode: 1,
        });
        let record = flood_fill.to_record().unwrap();
        assert_eq!(record.record_type, EmfRecordType::ExtFloodFill.raw());
        assert_eq!(record.parse_data().unwrap(), flood_fill);
    }

    #[test]
    fn typed_ext_text_out_w_record_roundtrips_without_decoding() {
        let text = SdkString::raw(vec![b'H', 0, b'i', 0], SdkEncoding::Utf16Le);
        let value = EmfRecordData::ExtTextOutW(EmrExtTextOut {
            bounds: RectL::default(),
            graphics_mode: 1,
            ex_scale: 1.0,
            ey_scale: 1.0,
            text: EmrText {
                reference: PointL { x: 12, y: 34 },
                options: ExtTextOutOptions::empty(),
                rectangle: None,
                text,
                dx: Vec::new(),
            },
        });

        let record = value.to_record().unwrap();
        assert_eq!(record.record_type, EmfRecordType::ExtTextOutW.raw());
        let parsed = record.parse_data().unwrap();
        assert_eq!(parsed, value);
    }

    #[test]
    fn typed_stretch_dibits_record_roundtrips() {
        let value = EmfRecordData::StretchDiBits(EmrStretchDiBits {
            bounds: RectL::default(),
            dest: PointL { x: 1, y: 2 },
            source: BitmapSourceBounds {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
            },
            color_usage: 0,
            raster_operation: 0x00CC_0020,
            dest_size: SizeL { cx: 2, cy: 2 },
            bitmap: EmrBitmapBuffer {
                bitmap_info: vec![
                    40, 0, 0, 0, // HeaderSize
                    2, 0, 0, 0, // Width
                    0xFE, 0xFF, 0xFF, 0xFF, // Height = -2
                    1, 0, // Planes
                    24, 0, // BitCount
                    0, 0, 0, 0, // BI_RGB
                    0, 0, 0, 0, // ImageSize
                    0, 0, 0, 0, // XPelsPerMeter
                    0, 0, 0, 0, // YPelsPerMeter
                    0, 0, 0, 0, // ColorUsed
                    0, 0, 0, 0, // ColorImportant
                ],
                bitmap_bits: vec![1, 2, 3, 4],
            },
        });

        let record = value.to_record().unwrap();
        assert_eq!(record.record_type, EmfRecordType::StretchDiBits.raw());
        let parsed = record.parse_data().unwrap();
        assert_eq!(parsed, value);
        let EmfRecordData::StretchDiBits(parsed) = parsed else {
            unreachable!();
        };
        assert_eq!(
            parsed.color_usage_kind(),
            Some(crate::bitmap::DibColorUsage::RgbColors)
        );
        let info = parsed.bitmap.dib_info().unwrap();
        assert_eq!(
            info.compression_kind(),
            Some(crate::bitmap::BitmapCompression::Rgb)
        );
        assert!(info.header.is_top_down());
    }

    #[test]
    fn typed_create_dib_pattern_brush_roundtrips() {
        let bitmap_info = vec![
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
        ];
        let value = EmfRecordData::CreateDibPatternBrushPt(EmrCreateDibPatternBrushPt {
            brush_index: 3,
            color_usage: crate::bitmap::DibColorUsage::RgbColors.raw(),
            bitmap: EmrBitmapBuffer {
                bitmap_info,
                bitmap_bits: vec![0x89, b'P', b'N', b'G'],
            },
        });

        let record = value.to_record().unwrap();
        assert_eq!(
            record.record_type,
            EmfRecordType::CreateDibPatternBrushPt.raw()
        );
        let parsed = record.parse_data().unwrap();
        assert_eq!(parsed, value);
        let EmfRecordData::CreateDibPatternBrushPt(parsed) = parsed else {
            unreachable!();
        };
        assert_eq!(
            parsed.color_usage_kind(),
            Some(crate::bitmap::DibColorUsage::RgbColors)
        );
        assert_eq!(
            parsed
                .bitmap
                .device_independent_bitmap()
                .unwrap()
                .embedded_format(),
            Some(crate::bitmap::EmbeddedBitmapFormat::Png)
        );
    }

    #[test]
    fn typed_create_mono_brush_roundtrips() {
        let value = EmfRecordData::CreateMonoBrush(EmrCreateMonoBrush {
            brush_index: 4,
            color_usage: crate::bitmap::DibColorUsage::RgbColors.raw(),
            bitmap: EmrBitmapBuffer {
                bitmap_info: vec![
                    40, 0, 0, 0, // HeaderSize
                    2, 0, 0, 0, // Width
                    2, 0, 0, 0, // Height
                    1, 0, // Planes
                    1, 0, // BitCount
                    0, 0, 0, 0, // BI_RGB
                    0, 0, 0, 0, // ImageSize
                    0, 0, 0, 0, // XPelsPerMeter
                    0, 0, 0, 0, // YPelsPerMeter
                    0, 0, 0, 0, // ColorUsed
                    0, 0, 0, 0, // ColorImportant
                    0, 0, 0, 0, // RGBQuad black
                    0xFF, 0xFF, 0xFF, 0, // RGBQuad white
                ],
                bitmap_bits: vec![0x80, 0, 0, 0, 0x40, 0, 0, 0],
            },
        });

        let record = value.to_record().unwrap();
        assert_eq!(record.record_type, EmfRecordType::CreateMonoBrush.raw());
        let parsed = record.parse_data().unwrap();
        assert_eq!(parsed, value);
        let EmfRecordData::CreateMonoBrush(parsed) = parsed else {
            unreachable!();
        };
        assert_eq!(
            parsed.bitmap.dib_info().unwrap().header.bit_count_kind(),
            Some(crate::bitmap::BitmapBitCount::One)
        );
    }

    #[test]
    fn typed_emf_plus_comment_roundtrips() {
        let value = EmfRecordData::Comment(EmrComment::EmfPlus {
            records: vec![crate::emfplus::EmfPlusRecord {
                record_type: crate::emfplus::EmfPlusRecordType::Header.raw(),
                flags: 0,
                data: vec![1, 2, 3, 4],
                padding: Vec::new(),
            }],
        });

        let record = value.to_record().unwrap();
        assert_eq!(record.record_type, EmfRecordType::Comment.raw());
        assert_eq!(record.parse_data().unwrap(), value);
    }
}
