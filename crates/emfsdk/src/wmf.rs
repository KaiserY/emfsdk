use std::io::Cursor;

use emfsdk_derive::{SdkEnum, SdkObject};

use crate::common::{Error, Reader, Result, SdkEnumValue, SdkRead, SdkSize, SdkWrite, Writer};
use crate::types::{ColorRef, PointS};

pub const META_EOF: u16 = 0x0000;
pub const PLACEABLE_KEY: u32 = 0x9AC6_CDD7;
pub const PLACEABLE_HEADER_SIZE: usize = 22;
pub const WMF_HEADER_SIZE: usize = 18;

#[derive(Clone, Copy, Debug, PartialEq, Eq, SdkEnum)]
#[sdk(repr = "u16")]
pub enum WmfRecordFunction {
    Eof = 0x0000,
    SaveDc = 0x001E,
    RealizePalette = 0x0035,
    SetPalEntries = 0x0037,
    CreatePalette = 0x00F7,
    SetBkMode = 0x0102,
    SetMapMode = 0x0103,
    SetRop2 = 0x0104,
    SetRelabs = 0x0105,
    SetPolyFillMode = 0x0106,
    SetStretchBltMode = 0x0107,
    SetTextCharExtra = 0x0108,
    RestoreDc = 0x0127,
    InvertRegion = 0x012A,
    PaintRegion = 0x012B,
    SelectClipRegion = 0x012C,
    SelectObject = 0x012D,
    SetTextAlign = 0x012E,
    ResizePalette = 0x0139,
    DibCreatePatternBrush = 0x0142,
    SetLayout = 0x0149,
    DeleteObject = 0x01F0,
    CreatePatternBrush = 0x01F9,
    CreatePenIndirect = 0x02FA,
    CreateFontIndirect = 0x02FB,
    CreateBrushIndirect = 0x02FC,
    SetBkColor = 0x0201,
    SetTextColor = 0x0209,
    SetTextJustification = 0x020A,
    SetWindowOrg = 0x020B,
    SetWindowExt = 0x020C,
    SetViewportOrg = 0x020D,
    SetViewportExt = 0x020E,
    OffsetWindowOrg = 0x020F,
    OffsetViewportOrg = 0x0211,
    LineTo = 0x0213,
    MoveTo = 0x0214,
    OffsetClipRgn = 0x0220,
    FillRegion = 0x0228,
    SetMapperFlags = 0x0231,
    SelectPalette = 0x0234,
    Polygon = 0x0324,
    Polyline = 0x0325,
    AnimatePalette = 0x0436,
    SetPixel = 0x041F,
    ExcludeClipRect = 0x0415,
    IntersectClipRect = 0x0416,
    Ellipse = 0x0418,
    FloodFill = 0x0419,
    Rectangle = 0x041B,
    ScaleWindowExt = 0x0410,
    ScaleViewportExt = 0x0412,
    FrameRegion = 0x0429,
    TextOut = 0x0521,
    PolyPolygon = 0x0538,
    ExtFloodFill = 0x0548,
    RoundRect = 0x061C,
    PatBlt = 0x061D,
    Escape = 0x0626,
    CreateRegion = 0x06FF,
    Arc = 0x0817,
    Pie = 0x081A,
    Chord = 0x0830,
    BitBlt = 0x0922,
    DibBitBlt = 0x0940,
    ExtTextOut = 0x0A32,
    StretchBlt = 0x0B23,
    DibStretchBlt = 0x0B41,
    SetDibToDev = 0x0D33,
    StretchDib = 0x0F43,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WmfMetafile {
    pub placeable_header: Option<WmfPlaceableHeader>,
    pub header: WmfHeader,
    pub records: Vec<WmfRecord>,
}

impl WmfMetafile {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(Cursor::new(bytes));
        let placeable_header = if has_placeable_header(bytes) {
            Some(WmfPlaceableHeader::read_from(&mut reader)?)
        } else {
            None
        };
        let header = WmfHeader::read_from(&mut reader)?;
        let mut records = Vec::new();

        while reader.position()? < bytes.len() as u64 {
            let record = WmfRecord::read_from(&mut reader, bytes.len() as u64)?;
            let is_eof = record.function == META_EOF;
            records.push(record);
            if is_eof {
                break;
            }
        }

        Ok(Self {
            placeable_header,
            header,
            records,
        })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        if let Some(header) = &self.placeable_header {
            header.write_to(&mut writer)?;
        }
        self.header.write_to(&mut writer)?;
        for record in &self.records {
            record.write_to(&mut writer)?;
        }
        Ok(writer.into_inner().into_inner())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WmfPlaceableHeader {
    pub key: u32,
    pub handle: u16,
    pub left: i16,
    pub top: i16,
    pub right: i16,
    pub bottom: i16,
    pub inch: u16,
    pub reserved: u32,
    pub checksum: u16,
}

impl WmfPlaceableHeader {
    pub fn read_from<R: std::io::Read + std::io::Seek>(reader: &mut Reader<R>) -> Result<Self> {
        let offset = reader.position()?;
        let key = reader.read_u32()?;
        if key != PLACEABLE_KEY {
            return Err(Error::invalid(offset, "invalid WMF placeable header key"));
        }
        Ok(Self {
            key,
            handle: reader.read_u16()?,
            left: reader.read_i16()?,
            top: reader.read_i16()?,
            right: reader.read_i16()?,
            bottom: reader.read_i16()?,
            inch: reader.read_u16()?,
            reserved: reader.read_u32()?,
            checksum: reader.read_u16()?,
        })
    }

    pub fn write_to<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut Writer<W>,
    ) -> Result<()> {
        writer.write_u32(self.key)?;
        writer.write_u16(self.handle)?;
        writer.write_i16(self.left)?;
        writer.write_i16(self.top)?;
        writer.write_i16(self.right)?;
        writer.write_i16(self.bottom)?;
        writer.write_u16(self.inch)?;
        writer.write_u32(self.reserved)?;
        writer.write_u16(self.checksum)
    }
}

impl SdkRead for WmfPlaceableHeader {
    fn read_from<R: std::io::Read + std::io::Seek>(reader: &mut Reader<R>) -> Result<Self> {
        Self::read_from(reader)
    }
}

impl SdkWrite for WmfPlaceableHeader {
    fn write_to<W: std::io::Write + std::io::Seek>(&self, writer: &mut Writer<W>) -> Result<()> {
        self.write_to(writer)
    }
}

impl SdkSize for WmfPlaceableHeader {
    fn sdk_size(&self) -> u64 {
        PLACEABLE_HEADER_SIZE as u64
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WmfHeader {
    pub metafile_type: u16,
    pub header_size_words: u16,
    pub version: u16,
    pub file_size_words: u32,
    pub number_of_objects: u16,
    pub max_record_words: u32,
    pub number_of_parameters: u16,
}

impl WmfHeader {
    pub fn read_from<R: std::io::Read + std::io::Seek>(reader: &mut Reader<R>) -> Result<Self> {
        let offset = reader.position()?;
        let header = Self {
            metafile_type: reader.read_u16()?,
            header_size_words: reader.read_u16()?,
            version: reader.read_u16()?,
            file_size_words: reader.read_u32()?,
            number_of_objects: reader.read_u16()?,
            max_record_words: reader.read_u32()?,
            number_of_parameters: reader.read_u16()?,
        };
        if header.header_size_words != 9 {
            return Err(Error::invalid(offset, "WMF header size must be 9 WORDs"));
        }
        Ok(header)
    }

    pub fn write_to<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut Writer<W>,
    ) -> Result<()> {
        writer.write_u16(self.metafile_type)?;
        writer.write_u16(self.header_size_words)?;
        writer.write_u16(self.version)?;
        writer.write_u32(self.file_size_words)?;
        writer.write_u16(self.number_of_objects)?;
        writer.write_u32(self.max_record_words)?;
        writer.write_u16(self.number_of_parameters)
    }
}

impl SdkRead for WmfHeader {
    fn read_from<R: std::io::Read + std::io::Seek>(reader: &mut Reader<R>) -> Result<Self> {
        Self::read_from(reader)
    }
}

impl SdkWrite for WmfHeader {
    fn write_to<W: std::io::Write + std::io::Seek>(&self, writer: &mut Writer<W>) -> Result<()> {
        self.write_to(writer)
    }
}

impl SdkSize for WmfHeader {
    fn sdk_size(&self) -> u64 {
        WMF_HEADER_SIZE as u64
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WmfRecord {
    pub function: u16,
    pub data: Vec<u8>,
}

impl WmfRecord {
    pub fn new(function: u16, data: Vec<u8>) -> Self {
        Self { function, data }
    }

    pub fn function_kind(&self) -> Option<WmfRecordFunction> {
        WmfRecordFunction::from_raw(self.function)
    }

    pub fn parse_data(&self) -> Result<WmfRecordData<'_>> {
        WmfRecordData::from_record(self)
    }

    pub fn read_from<R: std::io::Read + std::io::Seek>(
        reader: &mut Reader<R>,
        file_len: u64,
    ) -> Result<Self> {
        let offset = reader.position()?;
        let size_words = reader.read_u32()?;
        let function = reader.read_u16()?;
        let size_bytes = size_words
            .checked_mul(2)
            .ok_or_else(|| Error::invalid(offset, "WMF record size overflows"))?;
        if size_bytes < 6 {
            return Err(Error::invalid(
                offset,
                "WMF record size is smaller than its header",
            ));
        }
        let end = offset
            .checked_add(size_bytes as u64)
            .ok_or_else(|| Error::invalid(offset, "WMF record size overflows"))?;
        if end > file_len {
            return Err(Error::invalid(
                offset,
                "WMF record extends past end of file",
            ));
        }
        let data = reader.read_vec(size_bytes as usize - 6)?;
        Ok(Self { function, data })
    }

    pub fn write_to<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut Writer<W>,
    ) -> Result<()> {
        let size_bytes = self.data.len().checked_add(6).ok_or_else(|| {
            Error::invalid(writer.position().unwrap_or(0), "WMF record is too large")
        })?;
        if size_bytes % 2 != 0 {
            return Err(Error::invalid(
                writer.position()?,
                "WMF record data must include WORD alignment padding",
            ));
        }
        let size_words = size_bytes / 2;
        if size_words > u32::MAX as usize {
            return Err(Error::invalid(
                writer.position()?,
                "WMF record size exceeds u32::MAX WORDs",
            ));
        }
        writer.write_u32(size_words as u32)?;
        writer.write_u16(self.function)?;
        writer.write_all(&self.data)
    }
}

impl SdkSize for WmfRecord {
    fn sdk_size(&self) -> u64 {
        6 + self.data.len() as u64
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WmfRecordData<'a> {
    Eof,
    RealizePalette,
    SaveDc,
    SetRelabs,
    SetBkMode(WmfU16Record),
    SetMapMode(WmfU16Record),
    SetRop2(WmfU16Record),
    SetPolyFillMode(WmfU16Record),
    SetStretchBltMode(WmfU16Record),
    SetTextAlign(WmfU16Record),
    SetTextCharExtra(WmfU16Record),
    SetLayout(WmfU16Record),
    ResizePalette(WmfU16Record),
    RestoreDc(WmfI16Record),
    SetMapperFlags(WmfU32Record),
    SetTextJustification(WmfTextJustificationRecord),
    SetBkColor(WmfColorRecord),
    SetTextColor(WmfColorRecord),
    SetWindowOrg(WmfPointRecord),
    SetWindowExt(WmfPointRecord),
    SetViewportOrg(WmfPointRecord),
    SetViewportExt(WmfPointRecord),
    OffsetWindowOrg(WmfPointRecord),
    OffsetViewportOrg(WmfPointRecord),
    OffsetClipRgn(WmfPointRecord),
    MoveTo(WmfPointRecord),
    LineTo(WmfPointRecord),
    ScaleWindowExt(WmfScaleExtRecord),
    ScaleViewportExt(WmfScaleExtRecord),
    ExcludeClipRect(WmfRectRecord),
    IntersectClipRect(WmfRectRecord),
    Ellipse(WmfRectRecord),
    Rectangle(WmfRectRecord),
    RoundRect(WmfRoundRectRecord),
    Arc(WmfArcRecord),
    Chord(WmfArcRecord),
    Pie(WmfArcRecord),
    FloodFill(WmfFloodFillRecord),
    ExtFloodFill(WmfExtFloodFillRecord),
    SetPixel(WmfSetPixelRecord),
    PatBlt(WmfPatBltRecord),
    Polygon(WmfPolyPointsRecord),
    Polyline(WmfPolyPointsRecord),
    FillRegion(WmfRegionBrushRecord),
    FrameRegion(WmfFrameRegionRecord),
    InvertRegion(WmfObjectIndexRecord),
    PaintRegion(WmfObjectIndexRecord),
    SelectClipRegion(WmfObjectIndexRecord),
    SelectObject(WmfObjectIndexRecord),
    SelectPalette(WmfObjectIndexRecord),
    DeleteObject(WmfObjectIndexRecord),
    TextOut(WmfTextOutRecord),
    Escape(WmfEscapeRecord),
    Unknown(&'a WmfRecord),
}

impl<'a> WmfRecordData<'a> {
    pub fn from_record(record: &'a WmfRecord) -> Result<Self> {
        let data = &record.data;
        Ok(match record.function_kind() {
            Some(WmfRecordFunction::Eof) => {
                ensure_no_data(data, "META_EOF")?;
                Self::Eof
            }
            Some(WmfRecordFunction::RealizePalette) => {
                ensure_no_data(data, "META_REALIZEPALETTE")?;
                Self::RealizePalette
            }
            Some(WmfRecordFunction::SaveDc) => {
                ensure_no_data(data, "META_SAVEDC")?;
                Self::SaveDc
            }
            Some(WmfRecordFunction::SetRelabs) => {
                ensure_no_data(data, "META_SETRELABS")?;
                Self::SetRelabs
            }
            Some(WmfRecordFunction::SetBkMode) => Self::SetBkMode(WmfU16Record::read_data(data)?),
            Some(WmfRecordFunction::SetMapMode) => Self::SetMapMode(WmfU16Record::read_data(data)?),
            Some(WmfRecordFunction::SetRop2) => Self::SetRop2(WmfU16Record::read_data(data)?),
            Some(WmfRecordFunction::SetPolyFillMode) => {
                Self::SetPolyFillMode(WmfU16Record::read_data(data)?)
            }
            Some(WmfRecordFunction::SetStretchBltMode) => {
                Self::SetStretchBltMode(WmfU16Record::read_data(data)?)
            }
            Some(WmfRecordFunction::SetTextAlign) => {
                Self::SetTextAlign(WmfU16Record::read_data(data)?)
            }
            Some(WmfRecordFunction::SetTextCharExtra) => {
                Self::SetTextCharExtra(WmfU16Record::read_data(data)?)
            }
            Some(WmfRecordFunction::SetLayout) => Self::SetLayout(WmfU16Record::read_data(data)?),
            Some(WmfRecordFunction::ResizePalette) => {
                Self::ResizePalette(read_object(data, "META_RESIZEPALETTE")?)
            }
            Some(WmfRecordFunction::RestoreDc) => {
                Self::RestoreDc(read_object(data, "META_RESTOREDC")?)
            }
            Some(WmfRecordFunction::SetMapperFlags) => {
                Self::SetMapperFlags(read_object(data, "META_SETMAPPERFLAGS")?)
            }
            Some(WmfRecordFunction::SetTextJustification) => {
                Self::SetTextJustification(read_object(data, "META_SETTEXTJUSTIFICATION")?)
            }
            Some(WmfRecordFunction::SetBkColor) => {
                Self::SetBkColor(read_object(data, "META_SETBKCOLOR")?)
            }
            Some(WmfRecordFunction::SetTextColor) => {
                Self::SetTextColor(read_object(data, "META_SETTEXTCOLOR")?)
            }
            Some(WmfRecordFunction::SetWindowOrg) => {
                Self::SetWindowOrg(read_object(data, "META_SETWINDOWORG")?)
            }
            Some(WmfRecordFunction::SetWindowExt) => {
                Self::SetWindowExt(read_object(data, "META_SETWINDOWEXT")?)
            }
            Some(WmfRecordFunction::SetViewportOrg) => {
                Self::SetViewportOrg(read_object(data, "META_SETVIEWPORTORG")?)
            }
            Some(WmfRecordFunction::SetViewportExt) => {
                Self::SetViewportExt(read_object(data, "META_SETVIEWPORTEXT")?)
            }
            Some(WmfRecordFunction::OffsetWindowOrg) => {
                Self::OffsetWindowOrg(read_object(data, "META_OFFSETWINDOWORG")?)
            }
            Some(WmfRecordFunction::OffsetViewportOrg) => {
                Self::OffsetViewportOrg(read_object(data, "META_OFFSETVIEWPORTORG")?)
            }
            Some(WmfRecordFunction::OffsetClipRgn) => {
                Self::OffsetClipRgn(read_object(data, "META_OFFSETCLIPRGN")?)
            }
            Some(WmfRecordFunction::MoveTo) => Self::MoveTo(read_object(data, "META_MOVETO")?),
            Some(WmfRecordFunction::LineTo) => Self::LineTo(read_object(data, "META_LINETO")?),
            Some(WmfRecordFunction::ScaleWindowExt) => {
                Self::ScaleWindowExt(read_object(data, "META_SCALEWINDOWEXT")?)
            }
            Some(WmfRecordFunction::ScaleViewportExt) => {
                Self::ScaleViewportExt(read_object(data, "META_SCALEVIEWPORTEXT")?)
            }
            Some(WmfRecordFunction::ExcludeClipRect) => {
                Self::ExcludeClipRect(read_object(data, "META_EXCLUDECLIPRECT")?)
            }
            Some(WmfRecordFunction::IntersectClipRect) => {
                Self::IntersectClipRect(read_object(data, "META_INTERSECTCLIPRECT")?)
            }
            Some(WmfRecordFunction::Ellipse) => Self::Ellipse(read_object(data, "META_ELLIPSE")?),
            Some(WmfRecordFunction::Rectangle) => {
                Self::Rectangle(read_object(data, "META_RECTANGLE")?)
            }
            Some(WmfRecordFunction::RoundRect) => {
                Self::RoundRect(read_object(data, "META_ROUNDRECT")?)
            }
            Some(WmfRecordFunction::Arc) => Self::Arc(read_object(data, "META_ARC")?),
            Some(WmfRecordFunction::Chord) => Self::Chord(read_object(data, "META_CHORD")?),
            Some(WmfRecordFunction::Pie) => Self::Pie(read_object(data, "META_PIE")?),
            Some(WmfRecordFunction::FloodFill) => {
                Self::FloodFill(read_object(data, "META_FLOODFILL")?)
            }
            Some(WmfRecordFunction::ExtFloodFill) => {
                Self::ExtFloodFill(read_object(data, "META_EXTFLOODFILL")?)
            }
            Some(WmfRecordFunction::SetPixel) => {
                Self::SetPixel(read_object(data, "META_SETPIXEL")?)
            }
            Some(WmfRecordFunction::PatBlt) => Self::PatBlt(read_object(data, "META_PATBLT")?),
            Some(WmfRecordFunction::Polygon) => {
                Self::Polygon(WmfPolyPointsRecord::read_data(data, "META_POLYGON")?)
            }
            Some(WmfRecordFunction::Polyline) => {
                Self::Polyline(WmfPolyPointsRecord::read_data(data, "META_POLYLINE")?)
            }
            Some(WmfRecordFunction::FillRegion) => {
                Self::FillRegion(read_object(data, "META_FILLREGION")?)
            }
            Some(WmfRecordFunction::FrameRegion) => {
                Self::FrameRegion(read_object(data, "META_FRAMEREGION")?)
            }
            Some(WmfRecordFunction::InvertRegion) => {
                Self::InvertRegion(read_object(data, "META_INVERTREGION")?)
            }
            Some(WmfRecordFunction::PaintRegion) => {
                Self::PaintRegion(read_object(data, "META_PAINTREGION")?)
            }
            Some(WmfRecordFunction::SelectClipRegion) => {
                Self::SelectClipRegion(read_object(data, "META_SELECTCLIPREGION")?)
            }
            Some(WmfRecordFunction::SelectObject) => {
                Self::SelectObject(read_object(data, "META_SELECTOBJECT")?)
            }
            Some(WmfRecordFunction::SelectPalette) => {
                Self::SelectPalette(read_object(data, "META_SELECTPALETTE")?)
            }
            Some(WmfRecordFunction::DeleteObject) => {
                Self::DeleteObject(read_object(data, "META_DELETEOBJECT")?)
            }
            Some(WmfRecordFunction::TextOut) => Self::TextOut(WmfTextOutRecord::read_data(data)?),
            Some(WmfRecordFunction::Escape) => Self::Escape(WmfEscapeRecord::read_data(data)?),
            _ => Self::Unknown(record),
        })
    }

    pub fn to_record(&self) -> Result<WmfRecord> {
        Ok(match self {
            Self::Eof => no_data_record(WmfRecordFunction::Eof),
            Self::RealizePalette => no_data_record(WmfRecordFunction::RealizePalette),
            Self::SaveDc => no_data_record(WmfRecordFunction::SaveDc),
            Self::SetRelabs => no_data_record(WmfRecordFunction::SetRelabs),
            Self::SetBkMode(value) => u16_record(WmfRecordFunction::SetBkMode, value)?,
            Self::SetMapMode(value) => u16_record(WmfRecordFunction::SetMapMode, value)?,
            Self::SetRop2(value) => u16_record(WmfRecordFunction::SetRop2, value)?,
            Self::SetPolyFillMode(value) => u16_record(WmfRecordFunction::SetPolyFillMode, value)?,
            Self::SetStretchBltMode(value) => {
                u16_record(WmfRecordFunction::SetStretchBltMode, value)?
            }
            Self::SetTextAlign(value) => u16_record(WmfRecordFunction::SetTextAlign, value)?,
            Self::SetTextCharExtra(value) => {
                object_record(WmfRecordFunction::SetTextCharExtra, value)?
            }
            Self::SetLayout(value) => u16_record(WmfRecordFunction::SetLayout, value)?,
            Self::ResizePalette(value) => object_record(WmfRecordFunction::ResizePalette, value)?,
            Self::RestoreDc(value) => object_record(WmfRecordFunction::RestoreDc, value)?,
            Self::SetMapperFlags(value) => object_record(WmfRecordFunction::SetMapperFlags, value)?,
            Self::SetTextJustification(value) => {
                object_record(WmfRecordFunction::SetTextJustification, value)?
            }
            Self::SetBkColor(value) => object_record(WmfRecordFunction::SetBkColor, value)?,
            Self::SetTextColor(value) => object_record(WmfRecordFunction::SetTextColor, value)?,
            Self::SetWindowOrg(value) => object_record(WmfRecordFunction::SetWindowOrg, value)?,
            Self::SetWindowExt(value) => object_record(WmfRecordFunction::SetWindowExt, value)?,
            Self::SetViewportOrg(value) => object_record(WmfRecordFunction::SetViewportOrg, value)?,
            Self::SetViewportExt(value) => object_record(WmfRecordFunction::SetViewportExt, value)?,
            Self::OffsetWindowOrg(value) => {
                object_record(WmfRecordFunction::OffsetWindowOrg, value)?
            }
            Self::OffsetViewportOrg(value) => {
                object_record(WmfRecordFunction::OffsetViewportOrg, value)?
            }
            Self::OffsetClipRgn(value) => object_record(WmfRecordFunction::OffsetClipRgn, value)?,
            Self::MoveTo(value) => object_record(WmfRecordFunction::MoveTo, value)?,
            Self::LineTo(value) => object_record(WmfRecordFunction::LineTo, value)?,
            Self::ScaleWindowExt(value) => object_record(WmfRecordFunction::ScaleWindowExt, value)?,
            Self::ScaleViewportExt(value) => {
                object_record(WmfRecordFunction::ScaleViewportExt, value)?
            }
            Self::ExcludeClipRect(value) => {
                object_record(WmfRecordFunction::ExcludeClipRect, value)?
            }
            Self::IntersectClipRect(value) => {
                object_record(WmfRecordFunction::IntersectClipRect, value)?
            }
            Self::Ellipse(value) => object_record(WmfRecordFunction::Ellipse, value)?,
            Self::Rectangle(value) => object_record(WmfRecordFunction::Rectangle, value)?,
            Self::RoundRect(value) => object_record(WmfRecordFunction::RoundRect, value)?,
            Self::Arc(value) => object_record(WmfRecordFunction::Arc, value)?,
            Self::Chord(value) => object_record(WmfRecordFunction::Chord, value)?,
            Self::Pie(value) => object_record(WmfRecordFunction::Pie, value)?,
            Self::FloodFill(value) => object_record(WmfRecordFunction::FloodFill, value)?,
            Self::ExtFloodFill(value) => object_record(WmfRecordFunction::ExtFloodFill, value)?,
            Self::SetPixel(value) => object_record(WmfRecordFunction::SetPixel, value)?,
            Self::PatBlt(value) => object_record(WmfRecordFunction::PatBlt, value)?,
            Self::Polygon(value) => WmfRecord::new(
                WmfRecordFunction::Polygon.raw(),
                value.write_data("META_POLYGON")?,
            ),
            Self::Polyline(value) => WmfRecord::new(
                WmfRecordFunction::Polyline.raw(),
                value.write_data("META_POLYLINE")?,
            ),
            Self::FillRegion(value) => object_record(WmfRecordFunction::FillRegion, value)?,
            Self::FrameRegion(value) => object_record(WmfRecordFunction::FrameRegion, value)?,
            Self::InvertRegion(value) => object_record(WmfRecordFunction::InvertRegion, value)?,
            Self::PaintRegion(value) => object_record(WmfRecordFunction::PaintRegion, value)?,
            Self::SelectClipRegion(value) => {
                object_record(WmfRecordFunction::SelectClipRegion, value)?
            }
            Self::SelectObject(value) => object_record(WmfRecordFunction::SelectObject, value)?,
            Self::SelectPalette(value) => object_record(WmfRecordFunction::SelectPalette, value)?,
            Self::DeleteObject(value) => object_record(WmfRecordFunction::DeleteObject, value)?,
            Self::TextOut(value) => {
                WmfRecord::new(WmfRecordFunction::TextOut.raw(), value.write_data()?)
            }
            Self::Escape(value) => {
                WmfRecord::new(WmfRecordFunction::Escape.raw(), value.write_data()?)
            }
            Self::Unknown(record) => (*record).clone(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WmfU16Record {
    pub value: u16,
    pub reserved: Vec<u8>,
}

impl WmfU16Record {
    fn read_data(data: &[u8]) -> Result<Self> {
        if data.len() < 2 {
            return Err(Error::invalid(0, "WMF u16 record is too short"));
        }
        let mut reader = Reader::new(Cursor::new(data));
        let value = reader.read_u16()?;
        let reserved = reader.read_vec(data.len() - 2)?;
        Ok(Self { value, reserved })
    }

    fn write_data(&self) -> Result<Vec<u8>> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        writer.write_u16(self.value)?;
        writer.write_all(&self.reserved)?;
        Ok(writer.into_inner().into_inner())
    }
}

impl SdkRead for WmfU16Record {
    fn read_from<R: std::io::Read + std::io::Seek>(reader: &mut Reader<R>) -> Result<Self> {
        Ok(Self {
            value: reader.read_u16()?,
            reserved: Vec::new(),
        })
    }
}

impl SdkWrite for WmfU16Record {
    fn write_to<W: std::io::Write + std::io::Seek>(&self, writer: &mut Writer<W>) -> Result<()> {
        writer.write_u16(self.value)?;
        writer.write_all(&self.reserved)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "wmf")]
pub struct WmfI16Record {
    pub value: i16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "wmf")]
pub struct WmfU32Record {
    pub value: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "wmf")]
pub struct WmfColorRecord {
    pub color: ColorRef,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "wmf")]
pub struct WmfPointRecord {
    pub y: i16,
    pub x: i16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "wmf")]
pub struct WmfRectRecord {
    pub bottom: i16,
    pub right: i16,
    pub top: i16,
    pub left: i16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "wmf")]
pub struct WmfScaleExtRecord {
    pub y_denom: i16,
    pub y_num: i16,
    pub x_denom: i16,
    pub x_num: i16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "wmf")]
pub struct WmfTextJustificationRecord {
    pub break_count: u16,
    pub break_extra: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "wmf")]
pub struct WmfRoundRectRecord {
    pub height: i16,
    pub width: i16,
    pub bottom: i16,
    pub right: i16,
    pub top: i16,
    pub left: i16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "wmf")]
pub struct WmfArcRecord {
    pub y_radial_2: i16,
    pub x_radial_2: i16,
    pub y_radial_1: i16,
    pub x_radial_1: i16,
    pub bottom: i16,
    pub right: i16,
    pub top: i16,
    pub left: i16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "wmf")]
pub struct WmfFloodFillRecord {
    pub color: ColorRef,
    pub y_start: i16,
    pub x_start: i16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "wmf")]
pub struct WmfExtFloodFillRecord {
    pub mode: u16,
    pub color: ColorRef,
    pub y: i16,
    pub x: i16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "wmf")]
pub struct WmfSetPixelRecord {
    pub color: ColorRef,
    pub y: i16,
    pub x: i16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "wmf")]
pub struct WmfPatBltRecord {
    pub raster_operation: u32,
    pub height: i16,
    pub width: i16,
    pub y_left: i16,
    pub x_left: i16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WmfPolyPointsRecord {
    pub points: Vec<PointS>,
}

impl WmfPolyPointsRecord {
    fn read_data(data: &[u8], name: &str) -> Result<Self> {
        let mut reader = Reader::new(Cursor::new(data));
        let count = reader.read_i16()?;
        if count < 0 {
            return Err(Error::invalid(
                0,
                format!("{name} has negative point count"),
            ));
        }
        let mut points = Vec::with_capacity(count as usize);
        for _ in 0..count {
            points.push(PointS::read_from(&mut reader)?);
        }
        ensure_reader_end(&mut reader, data.len() as u64, name)?;
        Ok(Self { points })
    }

    fn write_data(&self, name: &str) -> Result<Vec<u8>> {
        if self.points.len() > i16::MAX as usize {
            return Err(Error::invalid(0, format!("{name} has too many points")));
        }
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        writer.write_i16(self.points.len() as i16)?;
        for point in &self.points {
            point.write_to(&mut writer)?;
        }
        Ok(writer.into_inner().into_inner())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "wmf")]
pub struct WmfRegionBrushRecord {
    pub region: u16,
    pub brush: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "wmf")]
pub struct WmfFrameRegionRecord {
    pub region: u16,
    pub brush: u16,
    pub height: i16,
    pub width: i16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "wmf")]
pub struct WmfObjectIndexRecord {
    pub index: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WmfTextOutRecord {
    pub string: Vec<u8>,
    pub string_padding: Vec<u8>,
    pub y_start: i16,
    pub x_start: i16,
}

impl WmfTextOutRecord {
    fn read_data(data: &[u8]) -> Result<Self> {
        if data.len() < 6 {
            return Err(Error::invalid(0, "META_TEXTOUT record is too short"));
        }
        let mut reader = Reader::new(Cursor::new(data));
        let string_length = reader.read_i16()?;
        if string_length < 0 {
            return Err(Error::invalid(0, "META_TEXTOUT has negative string length"));
        }
        let string_length = string_length as usize;
        let string_field_len = data.len() - 2 - 4;
        if string_field_len < string_length || !string_field_len.is_multiple_of(2) {
            return Err(Error::invalid(
                0,
                "META_TEXTOUT string field has invalid length",
            ));
        }
        let string = reader.read_vec(string_length)?;
        let string_padding = reader.read_vec(string_field_len - string_length)?;
        let y_start = reader.read_i16()?;
        let x_start = reader.read_i16()?;
        ensure_reader_end(&mut reader, data.len() as u64, "META_TEXTOUT")?;
        Ok(Self {
            string,
            string_padding,
            y_start,
            x_start,
        })
    }

    fn write_data(&self) -> Result<Vec<u8>> {
        if self.string.len() > i16::MAX as usize {
            return Err(Error::invalid(0, "META_TEXTOUT string is too long"));
        }
        if !(self.string.len() + self.string_padding.len()).is_multiple_of(2) {
            return Err(Error::invalid(
                0,
                "META_TEXTOUT string field must be WORD aligned",
            ));
        }
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        writer.write_i16(self.string.len() as i16)?;
        writer.write_all(&self.string)?;
        writer.write_all(&self.string_padding)?;
        writer.write_i16(self.y_start)?;
        writer.write_i16(self.x_start)?;
        Ok(writer.into_inner().into_inner())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WmfEscapeRecord {
    pub escape_function: u16,
    pub escape_data: Vec<u8>,
    pub padding: Vec<u8>,
}

impl WmfEscapeRecord {
    fn read_data(data: &[u8]) -> Result<Self> {
        if data.len() < 4 {
            return Err(Error::invalid(0, "META_ESCAPE record is too short"));
        }
        let mut reader = Reader::new(Cursor::new(data));
        let escape_function = reader.read_u16()?;
        let byte_count = reader.read_u16()? as usize;
        if data.len() < 4 + byte_count {
            return Err(Error::invalid(
                0,
                "META_ESCAPE byte count exceeds record data",
            ));
        }
        let escape_data = reader.read_vec(byte_count)?;
        let padding = reader.read_vec(data.len() - 4 - byte_count)?;
        Ok(Self {
            escape_function,
            escape_data,
            padding,
        })
    }

    fn write_data(&self) -> Result<Vec<u8>> {
        if self.escape_data.len() > u16::MAX as usize {
            return Err(Error::invalid(0, "META_ESCAPE data is too large"));
        }
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        writer.write_u16(self.escape_function)?;
        writer.write_u16(self.escape_data.len() as u16)?;
        writer.write_all(&self.escape_data)?;
        writer.write_all(&self.padding)?;
        Ok(writer.into_inner().into_inner())
    }
}

fn ensure_no_data(data: &[u8], name: &str) -> Result<()> {
    if data.is_empty() {
        Ok(())
    } else {
        Err(Error::invalid(
            0,
            format!("{name} record has unexpected payload"),
        ))
    }
}

fn ensure_reader_end<R: std::io::Read + std::io::Seek>(
    reader: &mut Reader<R>,
    end: u64,
    name: &str,
) -> Result<()> {
    let position = reader.position()?;
    if position == end {
        Ok(())
    } else {
        Err(Error::invalid(
            position,
            format!("{name} record has trailing data"),
        ))
    }
}

fn read_object<T: SdkRead>(data: &[u8], name: &str) -> Result<T> {
    let mut reader = Reader::new(Cursor::new(data));
    let value = T::read_from(&mut reader)?;
    ensure_reader_end(&mut reader, data.len() as u64, name)?;
    Ok(value)
}

fn object_record<T: SdkWrite>(function: WmfRecordFunction, value: &T) -> Result<WmfRecord> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    value.write_to(&mut writer)?;
    Ok(WmfRecord::new(
        function.raw(),
        writer.into_inner().into_inner(),
    ))
}

fn u16_record(function: WmfRecordFunction, value: &WmfU16Record) -> Result<WmfRecord> {
    Ok(WmfRecord::new(function.raw(), value.write_data()?))
}

fn no_data_record(function: WmfRecordFunction) -> WmfRecord {
    WmfRecord::new(function.raw(), Vec::new())
}

pub fn looks_like_wmf(bytes: &[u8]) -> bool {
    let offset = if has_placeable_header(bytes) {
        PLACEABLE_HEADER_SIZE
    } else {
        0
    };
    if bytes.len() < offset + WMF_HEADER_SIZE {
        return false;
    }
    let metafile_type = u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("slice length checked"),
    );
    let header_size = u16::from_le_bytes(
        bytes[offset + 2..offset + 4]
            .try_into()
            .expect("slice length checked"),
    );
    let version = u16::from_le_bytes(
        bytes[offset + 4..offset + 6]
            .try_into()
            .expect("slice length checked"),
    );
    matches!(metafile_type, 1 | 2) && header_size == 9 && matches!(version, 0x0100 | 0x0300)
}

fn has_placeable_header(bytes: &[u8]) -> bool {
    bytes.len() >= PLACEABLE_HEADER_SIZE
        && u32::from_le_bytes(bytes[0..4].try_into().expect("slice length checked"))
            == PLACEABLE_KEY
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_wmf() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&9u16.to_le_bytes());
        bytes.extend_from_slice(&0x0300u16.to_le_bytes());
        bytes.extend_from_slice(&12u32.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(&META_EOF.to_le_bytes());
        bytes
    }

    #[test]
    fn wmf_roundtrip_preserves_bytes() {
        let bytes = minimal_wmf();
        let metafile = WmfMetafile::from_bytes(&bytes).unwrap();
        assert_eq!(metafile.records.len(), 1);
        assert_eq!(metafile.to_bytes().unwrap(), bytes);
    }

    #[test]
    fn detects_wmf_header() {
        assert!(looks_like_wmf(&minimal_wmf()));
    }

    #[test]
    fn maps_wmf_record_function_enum() {
        let record = WmfRecord::new(META_EOF, Vec::new());
        assert_eq!(record.function_kind(), Some(WmfRecordFunction::Eof));
        assert_eq!(WmfRecordFunction::ExtTextOut.raw(), 0x0A32);
        assert_eq!(WmfRecordFunction::SaveDc.raw(), 0x001E);
    }

    fn assert_typed_roundtrip(record: WmfRecord) {
        let parsed = record.parse_data().unwrap();
        assert_eq!(parsed.to_record().unwrap(), record);
    }

    #[test]
    fn typed_wmf_no_data_records_roundtrip() {
        assert_eq!(
            WmfRecord::new(META_EOF, Vec::new()).parse_data().unwrap(),
            WmfRecordData::Eof
        );
        assert_typed_roundtrip(WmfRecord::new(WmfRecordFunction::SaveDc.raw(), Vec::new()));
        assert_typed_roundtrip(WmfRecord::new(
            WmfRecordFunction::RealizePalette.raw(),
            Vec::new(),
        ));
    }

    #[test]
    fn typed_wmf_state_records_roundtrip() {
        assert_typed_roundtrip(WmfRecord::new(
            WmfRecordFunction::SetBkMode.raw(),
            vec![0x02, 0x00, 0xAA, 0xBB],
        ));
        assert_typed_roundtrip(WmfRecord::new(
            WmfRecordFunction::RestoreDc.raw(),
            (-2i16).to_le_bytes().to_vec(),
        ));
        assert_typed_roundtrip(WmfRecord::new(
            WmfRecordFunction::SetWindowOrg.raw(),
            [10i16.to_le_bytes(), (-3i16).to_le_bytes()].concat(),
        ));
        assert_typed_roundtrip(WmfRecord::new(
            WmfRecordFunction::ScaleWindowExt.raw(),
            [
                2i16.to_le_bytes(),
                3i16.to_le_bytes(),
                4i16.to_le_bytes(),
                5i16.to_le_bytes(),
            ]
            .concat(),
        ));
    }

    #[test]
    fn typed_wmf_fixed_drawing_records_roundtrip() {
        assert_typed_roundtrip(WmfRecord::new(
            WmfRecordFunction::Ellipse.raw(),
            [
                40i16.to_le_bytes(),
                30i16.to_le_bytes(),
                20i16.to_le_bytes(),
                10i16.to_le_bytes(),
            ]
            .concat(),
        ));
        assert_typed_roundtrip(WmfRecord::new(
            WmfRecordFunction::Arc.raw(),
            [
                8i16.to_le_bytes(),
                7i16.to_le_bytes(),
                6i16.to_le_bytes(),
                5i16.to_le_bytes(),
                4i16.to_le_bytes(),
                3i16.to_le_bytes(),
                2i16.to_le_bytes(),
                1i16.to_le_bytes(),
            ]
            .concat(),
        ));
        assert_typed_roundtrip(WmfRecord::new(
            WmfRecordFunction::SetPixel.raw(),
            vec![1, 2, 3, 0, 9, 0, 8, 0],
        ));
    }

    #[test]
    fn typed_wmf_polygon_text_and_escape_records_roundtrip() {
        assert_typed_roundtrip(WmfRecord::new(
            WmfRecordFunction::Polygon.raw(),
            [
                2i16.to_le_bytes(),
                1i16.to_le_bytes(),
                2i16.to_le_bytes(),
                3i16.to_le_bytes(),
                4i16.to_le_bytes(),
            ]
            .concat(),
        ));
        assert_typed_roundtrip(WmfRecord::new(
            WmfRecordFunction::TextOut.raw(),
            [
                3i16.to_le_bytes().as_slice(),
                b"abc",
                &[0],
                9i16.to_le_bytes().as_slice(),
                8i16.to_le_bytes().as_slice(),
            ]
            .concat(),
        ));
        assert_typed_roundtrip(WmfRecord::new(
            WmfRecordFunction::Escape.raw(),
            [
                0x000Fu16.to_le_bytes().as_slice(),
                3u16.to_le_bytes().as_slice(),
                &[1, 2, 3, 0],
            ]
            .concat(),
        ));
    }
}
