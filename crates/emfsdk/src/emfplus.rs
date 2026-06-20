use bitflags::bitflags;
use emfsdk_derive::{SdkEnum, SdkObject};

use crate::common::{Error, Reader, Result, SdkEnumValue, SdkRead, SdkSize, SdkWrite, Writer};
use crate::types::{EmfPlusArgb, PointL, RectF, XForm};

pub const EMFPLUS_METAFILE_SIGNATURE: u32 = 0xDBC01;

bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct EmfPlusRecordFlags: u16 {
        const OBJECT_ID_MASK = 0x00FF;
        const POST_MULTIPLY = 0x2000;
        const COMPRESSED = 0x4000;
        const SOLID_COLOR = 0x8000;
    }
}

impl EmfPlusRecordFlags {
    pub fn object_id(self) -> u8 {
        (self.bits() & Self::OBJECT_ID_MASK.bits()) as u8
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, SdkEnum)]
#[sdk(repr = "u16")]
pub enum EmfPlusRecordType {
    Header = 0x4001,
    Eof = 0x4002,
    Comment = 0x4003,
    GetDc = 0x4004,
    MultiFormatStart = 0x4005,
    MultiFormatSection = 0x4006,
    MultiFormatEnd = 0x4007,
    Object = 0x4008,
    Clear = 0x4009,
    FillRects = 0x400A,
    DrawRects = 0x400B,
    FillPolygon = 0x400C,
    DrawLines = 0x400D,
    FillEllipse = 0x400E,
    DrawEllipse = 0x400F,
    FillPie = 0x4010,
    DrawPie = 0x4011,
    DrawArc = 0x4012,
    FillRegion = 0x4013,
    FillPath = 0x4014,
    DrawPath = 0x4015,
    FillClosedCurve = 0x4016,
    DrawClosedCurve = 0x4017,
    DrawCurve = 0x4018,
    DrawBeziers = 0x4019,
    DrawImage = 0x401A,
    DrawImagePoints = 0x401B,
    DrawString = 0x401C,
    SetRenderingOrigin = 0x401D,
    SetAntiAliasMode = 0x401E,
    SetTextRenderingHint = 0x401F,
    SetTextContrast = 0x4020,
    SetInterpolationMode = 0x4021,
    SetPixelOffsetMode = 0x4022,
    SetCompositingMode = 0x4023,
    SetCompositingQuality = 0x4024,
    Save = 0x4025,
    Restore = 0x4026,
    BeginContainer = 0x4027,
    BeginContainerNoParams = 0x4028,
    EndContainer = 0x4029,
    SetWorldTransform = 0x402A,
    ResetWorldTransform = 0x402B,
    MultiplyWorldTransform = 0x402C,
    TranslateWorldTransform = 0x402D,
    ScaleWorldTransform = 0x402E,
    RotateWorldTransform = 0x402F,
    SetPageTransform = 0x4030,
    ResetClip = 0x4031,
    SetClipRect = 0x4032,
    SetClipRegion = 0x4033,
    SetClipPath = 0x4034,
    OffsetClip = 0x4035,
    DrawDriverString = 0x4036,
    StrokeFillPath = 0x4037,
    SerializableObject = 0x4038,
    SetTsGraphics = 0x4039,
    SetTsClip = 0x403A,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, SdkEnum)]
#[sdk(repr = "u16")]
pub enum EmfPlusGraphicsVersionValue {
    Version1 = 0x0001,
    Version1_1 = 0x0002,
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
#[sdk(format = "emfplus")]
pub struct EmfPlusGraphicsVersion {
    pub value: u32,
}

impl EmfPlusGraphicsVersion {
    pub fn metafile_signature(&self) -> u32 {
        self.value >> 12
    }

    pub fn graphics_version_raw(&self) -> u16 {
        (self.value & 0x0FFF) as u16
    }

    pub fn graphics_version(&self) -> Option<EmfPlusGraphicsVersionValue> {
        EmfPlusGraphicsVersionValue::from_raw(self.graphics_version_raw())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, SdkObject)]
#[sdk(format = "emfplus", record_type = 0x4001)]
pub struct EmfPlusHeaderData {
    pub graphics_version: EmfPlusGraphicsVersion,
    pub emf_plus_flags: u32,
    pub logical_dpi_x: u32,
    pub logical_dpi_y: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "emfplus")]
pub struct EmfPlusRectS {
    pub x: i16,
    pub y: i16,
    pub width: i16,
    pub height: i16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EmfPlusRect {
    Compressed(EmfPlusRectS),
    Float(RectF),
}

impl EmfPlusRect {
    pub fn sdk_size(&self) -> u64 {
        match self {
            Self::Compressed(value) => value.sdk_size(),
            Self::Float(value) => value.sdk_size(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmfPlusBrushRef {
    ObjectId(u32),
    Color(EmfPlusArgb),
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmfPlusFillRectsData {
    pub brush: EmfPlusBrushRef,
    pub rects: Vec<EmfPlusRect>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmfPlusDrawRectsData {
    pub pen_id: u8,
    pub rects: Vec<EmfPlusRect>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmfPlusDrawRectShapeData {
    pub pen_id: u8,
    pub rect: EmfPlusRect,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmfPlusFillRectShapeData {
    pub brush: EmfPlusBrushRef,
    pub rect: EmfPlusRect,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmfPlusDrawArcData {
    pub pen_id: u8,
    pub start_angle: f32,
    pub sweep_angle: f32,
    pub rect: EmfPlusRect,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmfPlusFillPieData {
    pub brush: EmfPlusBrushRef,
    pub start_angle: f32,
    pub sweep_angle: f32,
    pub rect: EmfPlusRect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmfPlusBrushObjectData {
    pub brush: EmfPlusBrushRef,
}

#[derive(Clone, Copy, Debug, PartialEq, SdkObject)]
#[sdk(format = "emfplus")]
pub struct EmfPlusTranslateWorldTransformData {
    pub dx: f32,
    pub dy: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, SdkObject)]
#[sdk(format = "emfplus")]
pub struct EmfPlusScaleWorldTransformData {
    pub sx: f32,
    pub sy: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, SdkObject)]
#[sdk(format = "emfplus")]
pub struct EmfPlusRotateWorldTransformData {
    pub angle: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, SdkObject)]
#[sdk(format = "emfplus")]
pub struct EmfPlusSetPageTransformData {
    pub page_scale: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, SdkObject)]
#[sdk(format = "emfplus")]
pub struct EmfPlusSetClipRectData {
    pub clip_rect: RectF,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, SdkObject)]
#[sdk(format = "emfplus")]
pub struct EmfPlusClearData {
    pub color: EmfPlusArgb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, SdkObject)]
#[sdk(format = "emfplus")]
pub struct EmfPlusStackIndexData {
    pub stack_index: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, SdkObject)]
#[sdk(format = "emfplus")]
pub struct EmfPlusBeginContainerData {
    pub dest_rect: RectF,
    pub src_rect: RectF,
    pub stack_index: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EmfPlusRecordData<'a> {
    Header(EmfPlusHeaderData),
    Eof,
    Comment(Vec<u8>),
    GetDc,
    Clear(EmfPlusClearData),
    FillRects(EmfPlusFillRectsData),
    DrawRects(EmfPlusDrawRectsData),
    FillEllipse(EmfPlusFillRectShapeData),
    DrawEllipse(EmfPlusDrawRectShapeData),
    FillPie(EmfPlusFillPieData),
    DrawPie(EmfPlusDrawArcData),
    DrawArc(EmfPlusDrawArcData),
    FillRegion(EmfPlusBrushObjectData),
    FillPath(EmfPlusBrushObjectData),
    DrawPath,
    ResetClip,
    SetClipRect(EmfPlusSetClipRectData),
    SetClipPath,
    SetClipRegion,
    OffsetClip(EmfPlusTranslateWorldTransformData),
    SetRenderingOrigin(PointL),
    SetAntiAliasMode,
    SetTextRenderingHint,
    SetTextContrast,
    SetInterpolationMode,
    SetPixelOffsetMode,
    SetCompositingMode,
    SetCompositingQuality,
    Save(EmfPlusStackIndexData),
    Restore(EmfPlusStackIndexData),
    BeginContainer(EmfPlusBeginContainerData),
    BeginContainerNoParams(EmfPlusStackIndexData),
    EndContainer(EmfPlusStackIndexData),
    SetWorldTransform(XForm),
    ResetWorldTransform,
    MultiplyWorldTransform(XForm),
    TranslateWorldTransform(EmfPlusTranslateWorldTransformData),
    ScaleWorldTransform(EmfPlusScaleWorldTransformData),
    RotateWorldTransform(EmfPlusRotateWorldTransformData),
    SetPageTransform(EmfPlusSetPageTransformData),
    Unknown(&'a EmfPlusRecord),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmfPlusRecord {
    pub record_type: u16,
    pub flags: u16,
    pub data: Vec<u8>,
    pub padding: Vec<u8>,
}

impl EmfPlusRecord {
    pub fn flags(&self) -> EmfPlusRecordFlags {
        EmfPlusRecordFlags::from_bits_retain(self.flags)
    }

    pub fn record_kind(&self) -> Option<EmfPlusRecordType> {
        EmfPlusRecordType::from_raw(self.record_type)
    }

    pub fn parse_data(&self) -> Result<EmfPlusRecordData<'_>> {
        let mut reader = Reader::new(std::io::Cursor::new(self.data.as_slice()));
        let flags = self.flags();

        let data = match self.record_kind() {
            Some(EmfPlusRecordType::Header) if self.data.len() >= 16 => {
                EmfPlusRecordData::Header(EmfPlusHeaderData::read_from(&mut reader)?)
            }
            Some(EmfPlusRecordType::Eof) => {
                ensure_empty_data(&self.data, "EmfPlusEndOfFile")?;
                EmfPlusRecordData::Eof
            }
            Some(EmfPlusRecordType::Comment) => EmfPlusRecordData::Comment(self.data.clone()),
            Some(EmfPlusRecordType::GetDc) => {
                ensure_empty_data(&self.data, "EmfPlusGetDC")?;
                EmfPlusRecordData::GetDc
            }
            Some(EmfPlusRecordType::Clear) => {
                EmfPlusRecordData::Clear(read_exact_object(&self.data, "EmfPlusClear")?)
            }
            Some(EmfPlusRecordType::FillRects) if self.data.len() >= 8 => {
                let brush = if flags.contains(EmfPlusRecordFlags::SOLID_COLOR) {
                    EmfPlusBrushRef::Color(EmfPlusArgb::read_from(&mut reader)?)
                } else {
                    EmfPlusBrushRef::ObjectId(reader.read_u32()?)
                };
                let count = reader.read_u32()? as usize;
                let rects = read_rects(&mut reader, count, flags, self.data.len() as u64)?;
                EmfPlusRecordData::FillRects(EmfPlusFillRectsData { brush, rects })
            }
            Some(EmfPlusRecordType::DrawRects) if self.data.len() >= 4 => {
                let count = reader.read_u32()? as usize;
                let rects = read_rects(&mut reader, count, flags, self.data.len() as u64)?;
                EmfPlusRecordData::DrawRects(EmfPlusDrawRectsData {
                    pen_id: flags.object_id(),
                    rects,
                })
            }
            Some(EmfPlusRecordType::FillEllipse) if self.data.len() >= 12 => {
                let brush = read_brush_ref(&mut reader, flags)?;
                let rect = read_single_rect(&mut reader, flags, self.data.len() as u64)?;
                EmfPlusRecordData::FillEllipse(EmfPlusFillRectShapeData { brush, rect })
            }
            Some(EmfPlusRecordType::DrawEllipse) if self.data.len() >= 8 => {
                let rect = read_single_rect(&mut reader, flags, self.data.len() as u64)?;
                EmfPlusRecordData::DrawEllipse(EmfPlusDrawRectShapeData {
                    pen_id: flags.object_id(),
                    rect,
                })
            }
            Some(EmfPlusRecordType::FillPie) if self.data.len() >= 20 => {
                let brush = read_brush_ref(&mut reader, flags)?;
                let start_angle = reader.read_f32()?;
                let sweep_angle = reader.read_f32()?;
                let rect = read_single_rect(&mut reader, flags, self.data.len() as u64)?;
                EmfPlusRecordData::FillPie(EmfPlusFillPieData {
                    brush,
                    start_angle,
                    sweep_angle,
                    rect,
                })
            }
            Some(EmfPlusRecordType::DrawPie) if self.data.len() >= 16 => {
                let start_angle = reader.read_f32()?;
                let sweep_angle = reader.read_f32()?;
                let rect = read_single_rect(&mut reader, flags, self.data.len() as u64)?;
                EmfPlusRecordData::DrawPie(EmfPlusDrawArcData {
                    pen_id: flags.object_id(),
                    start_angle,
                    sweep_angle,
                    rect,
                })
            }
            Some(EmfPlusRecordType::DrawArc) if self.data.len() >= 16 => {
                let start_angle = reader.read_f32()?;
                let sweep_angle = reader.read_f32()?;
                let rect = read_single_rect(&mut reader, flags, self.data.len() as u64)?;
                EmfPlusRecordData::DrawArc(EmfPlusDrawArcData {
                    pen_id: flags.object_id(),
                    start_angle,
                    sweep_angle,
                    rect,
                })
            }
            Some(EmfPlusRecordType::FillRegion) if self.data.len() == 4 => {
                EmfPlusRecordData::FillRegion(EmfPlusBrushObjectData {
                    brush: read_brush_ref(&mut reader, flags)?,
                })
            }
            Some(EmfPlusRecordType::FillPath) if self.data.len() == 4 => {
                EmfPlusRecordData::FillPath(EmfPlusBrushObjectData {
                    brush: read_brush_ref(&mut reader, flags)?,
                })
            }
            Some(EmfPlusRecordType::DrawPath) => {
                ensure_empty_data(&self.data, "EmfPlusDrawPath")?;
                EmfPlusRecordData::DrawPath
            }
            Some(EmfPlusRecordType::ResetClip) => {
                ensure_empty_data(&self.data, "EmfPlusResetClip")?;
                EmfPlusRecordData::ResetClip
            }
            Some(EmfPlusRecordType::SetClipRect) => {
                EmfPlusRecordData::SetClipRect(read_exact_object(&self.data, "EmfPlusSetClipRect")?)
            }
            Some(EmfPlusRecordType::SetClipPath) => {
                ensure_empty_data(&self.data, "EmfPlusSetClipPath")?;
                EmfPlusRecordData::SetClipPath
            }
            Some(EmfPlusRecordType::SetClipRegion) => {
                ensure_empty_data(&self.data, "EmfPlusSetClipRegion")?;
                EmfPlusRecordData::SetClipRegion
            }
            Some(EmfPlusRecordType::OffsetClip) => {
                EmfPlusRecordData::OffsetClip(read_exact_object(&self.data, "EmfPlusOffsetClip")?)
            }
            Some(EmfPlusRecordType::SetRenderingOrigin) => EmfPlusRecordData::SetRenderingOrigin(
                read_exact_object(&self.data, "EmfPlusSetRenderingOrigin")?,
            ),
            Some(EmfPlusRecordType::SetAntiAliasMode) => {
                ensure_empty_data(&self.data, "EmfPlusSetAntiAliasMode")?;
                EmfPlusRecordData::SetAntiAliasMode
            }
            Some(EmfPlusRecordType::SetTextRenderingHint) => {
                ensure_empty_data(&self.data, "EmfPlusSetTextRenderingHint")?;
                EmfPlusRecordData::SetTextRenderingHint
            }
            Some(EmfPlusRecordType::SetTextContrast) => {
                ensure_empty_data(&self.data, "EmfPlusSetTextContrast")?;
                EmfPlusRecordData::SetTextContrast
            }
            Some(EmfPlusRecordType::SetInterpolationMode) => {
                ensure_empty_data(&self.data, "EmfPlusSetInterpolationMode")?;
                EmfPlusRecordData::SetInterpolationMode
            }
            Some(EmfPlusRecordType::SetPixelOffsetMode) => {
                ensure_empty_data(&self.data, "EmfPlusSetPixelOffsetMode")?;
                EmfPlusRecordData::SetPixelOffsetMode
            }
            Some(EmfPlusRecordType::SetCompositingMode) => {
                ensure_empty_data(&self.data, "EmfPlusSetCompositingMode")?;
                EmfPlusRecordData::SetCompositingMode
            }
            Some(EmfPlusRecordType::SetCompositingQuality) => {
                ensure_empty_data(&self.data, "EmfPlusSetCompositingQuality")?;
                EmfPlusRecordData::SetCompositingQuality
            }
            Some(EmfPlusRecordType::Save) => {
                EmfPlusRecordData::Save(read_exact_object(&self.data, "EmfPlusSave")?)
            }
            Some(EmfPlusRecordType::Restore) => {
                EmfPlusRecordData::Restore(read_exact_object(&self.data, "EmfPlusRestore")?)
            }
            Some(EmfPlusRecordType::BeginContainer) => EmfPlusRecordData::BeginContainer(
                read_exact_object(&self.data, "EmfPlusBeginContainer")?,
            ),
            Some(EmfPlusRecordType::BeginContainerNoParams) => {
                EmfPlusRecordData::BeginContainerNoParams(read_exact_object(
                    &self.data,
                    "EmfPlusBeginContainerNoParams",
                )?)
            }
            Some(EmfPlusRecordType::EndContainer) => EmfPlusRecordData::EndContainer(
                read_exact_object(&self.data, "EmfPlusEndContainer")?,
            ),
            Some(EmfPlusRecordType::SetWorldTransform) if self.data.len() >= 24 => {
                EmfPlusRecordData::SetWorldTransform(XForm::read_from(&mut reader)?)
            }
            Some(EmfPlusRecordType::ResetWorldTransform) => EmfPlusRecordData::ResetWorldTransform,
            Some(EmfPlusRecordType::MultiplyWorldTransform) if self.data.len() >= 24 => {
                EmfPlusRecordData::MultiplyWorldTransform(XForm::read_from(&mut reader)?)
            }
            Some(EmfPlusRecordType::TranslateWorldTransform) if self.data.len() >= 8 => {
                EmfPlusRecordData::TranslateWorldTransform(
                    EmfPlusTranslateWorldTransformData::read_from(&mut reader)?,
                )
            }
            Some(EmfPlusRecordType::ScaleWorldTransform) if self.data.len() >= 8 => {
                EmfPlusRecordData::ScaleWorldTransform(EmfPlusScaleWorldTransformData::read_from(
                    &mut reader,
                )?)
            }
            Some(EmfPlusRecordType::RotateWorldTransform) => {
                EmfPlusRecordData::RotateWorldTransform(read_exact_object(
                    &self.data,
                    "EmfPlusRotateWorldTransform",
                )?)
            }
            Some(EmfPlusRecordType::SetPageTransform) => EmfPlusRecordData::SetPageTransform(
                read_exact_object(&self.data, "EmfPlusSetPageTransform")?,
            ),
            _ => EmfPlusRecordData::Unknown(self),
        };

        Ok(data)
    }

    pub fn from_data(data: &EmfPlusRecordData<'_>, flags: EmfPlusRecordFlags) -> Result<Self> {
        let mut record_data = Vec::new();
        {
            let mut writer = Writer::new(std::io::Cursor::new(&mut record_data));
            match data {
                EmfPlusRecordData::Header(value) => value.write_to(&mut writer)?,
                EmfPlusRecordData::Eof => {}
                EmfPlusRecordData::Comment(value) => writer.write_all(value)?,
                EmfPlusRecordData::GetDc => {}
                EmfPlusRecordData::Clear(value) => value.write_to(&mut writer)?,
                EmfPlusRecordData::FillRects(value) => {
                    write_brush_ref(&mut writer, value.brush)?;
                    writer.write_u32(len_to_u32(value.rects.len(), "EMF+ rect count")?)?;
                    write_rects(&mut writer, &value.rects)?;
                }
                EmfPlusRecordData::DrawRects(value) => {
                    writer.write_u32(len_to_u32(value.rects.len(), "EMF+ rect count")?)?;
                    write_rects(&mut writer, &value.rects)?;
                }
                EmfPlusRecordData::FillEllipse(value) => {
                    write_brush_ref(&mut writer, value.brush)?;
                    write_rects(&mut writer, std::slice::from_ref(&value.rect))?;
                }
                EmfPlusRecordData::DrawEllipse(value) => {
                    write_rects(&mut writer, std::slice::from_ref(&value.rect))?;
                }
                EmfPlusRecordData::FillPie(value) => {
                    write_brush_ref(&mut writer, value.brush)?;
                    writer.write_f32(value.start_angle)?;
                    writer.write_f32(value.sweep_angle)?;
                    write_rects(&mut writer, std::slice::from_ref(&value.rect))?;
                }
                EmfPlusRecordData::DrawPie(value) | EmfPlusRecordData::DrawArc(value) => {
                    writer.write_f32(value.start_angle)?;
                    writer.write_f32(value.sweep_angle)?;
                    write_rects(&mut writer, std::slice::from_ref(&value.rect))?;
                }
                EmfPlusRecordData::FillRegion(value) | EmfPlusRecordData::FillPath(value) => {
                    write_brush_ref(&mut writer, value.brush)?;
                }
                EmfPlusRecordData::DrawPath => {}
                EmfPlusRecordData::ResetClip => {}
                EmfPlusRecordData::SetClipRect(value) => value.write_to(&mut writer)?,
                EmfPlusRecordData::SetClipPath => {}
                EmfPlusRecordData::SetClipRegion => {}
                EmfPlusRecordData::OffsetClip(value) => value.write_to(&mut writer)?,
                EmfPlusRecordData::SetRenderingOrigin(value) => value.write_to(&mut writer)?,
                EmfPlusRecordData::SetAntiAliasMode
                | EmfPlusRecordData::SetTextRenderingHint
                | EmfPlusRecordData::SetTextContrast
                | EmfPlusRecordData::SetInterpolationMode
                | EmfPlusRecordData::SetPixelOffsetMode
                | EmfPlusRecordData::SetCompositingMode
                | EmfPlusRecordData::SetCompositingQuality => {}
                EmfPlusRecordData::Save(value)
                | EmfPlusRecordData::Restore(value)
                | EmfPlusRecordData::BeginContainerNoParams(value)
                | EmfPlusRecordData::EndContainer(value) => value.write_to(&mut writer)?,
                EmfPlusRecordData::BeginContainer(value) => value.write_to(&mut writer)?,
                EmfPlusRecordData::SetWorldTransform(value) => value.write_to(&mut writer)?,
                EmfPlusRecordData::ResetWorldTransform => {}
                EmfPlusRecordData::MultiplyWorldTransform(value) => value.write_to(&mut writer)?,
                EmfPlusRecordData::TranslateWorldTransform(value) => value.write_to(&mut writer)?,
                EmfPlusRecordData::ScaleWorldTransform(value) => value.write_to(&mut writer)?,
                EmfPlusRecordData::RotateWorldTransform(value) => value.write_to(&mut writer)?,
                EmfPlusRecordData::SetPageTransform(value) => value.write_to(&mut writer)?,
                EmfPlusRecordData::Unknown(record) => {
                    return Ok((*record).clone());
                }
            }
        }

        Ok(Self {
            record_type: data.record_type(),
            flags: data.record_flags(flags).bits(),
            data: record_data,
            padding: Vec::new(),
        })
    }

    pub fn read_from<R: std::io::Read + std::io::Seek>(
        reader: &mut Reader<R>,
        stream_len: u64,
    ) -> Result<Self> {
        let offset = reader.position()?;
        let record_type = reader.read_u16()?;
        let flags = reader.read_u16()?;
        let size = reader.read_u32()?;
        let data_size = reader.read_u32()?;

        if size < 12 {
            return Err(Error::invalid(
                offset,
                "EMF+ record size is smaller than its header",
            ));
        }
        if data_size > size - 12 {
            return Err(Error::invalid(
                offset,
                "EMF+ record data size exceeds record size",
            ));
        }
        let end = offset
            .checked_add(size as u64)
            .ok_or_else(|| Error::invalid(offset, "EMF+ record size overflows"))?;
        if end > stream_len {
            return Err(Error::invalid(
                offset,
                "EMF+ record extends past end of stream",
            ));
        }

        let data = reader.read_vec(data_size as usize)?;
        let padding = reader.read_vec(size as usize - 12 - data_size as usize)?;

        Ok(Self {
            record_type,
            flags,
            data,
            padding,
        })
    }

    pub fn write_to<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut Writer<W>,
    ) -> Result<()> {
        let payload_size = self
            .data
            .len()
            .checked_add(self.padding.len())
            .ok_or_else(|| {
                Error::invalid(writer.position().unwrap_or(0), "EMF+ record is too large")
            })?;
        let size = payload_size.checked_add(12).ok_or_else(|| {
            Error::invalid(writer.position().unwrap_or(0), "EMF+ record is too large")
        })?;
        if size > u32::MAX as usize {
            return Err(Error::invalid(
                writer.position()?,
                "EMF+ record size exceeds u32::MAX",
            ));
        }

        writer.write_u16(self.record_type)?;
        writer.write_u16(self.flags)?;
        writer.write_u32(size as u32)?;
        writer.write_u32(self.data.len() as u32)?;
        writer.write_all(&self.data)?;
        writer.write_all(&self.padding)
    }
}

impl EmfPlusRecordData<'_> {
    pub fn record_type(&self) -> u16 {
        match self {
            Self::Header(_) => EmfPlusRecordType::Header.raw(),
            Self::Eof => EmfPlusRecordType::Eof.raw(),
            Self::Comment(_) => EmfPlusRecordType::Comment.raw(),
            Self::GetDc => EmfPlusRecordType::GetDc.raw(),
            Self::Clear(_) => EmfPlusRecordType::Clear.raw(),
            Self::FillRects(_) => EmfPlusRecordType::FillRects.raw(),
            Self::DrawRects(_) => EmfPlusRecordType::DrawRects.raw(),
            Self::FillEllipse(_) => EmfPlusRecordType::FillEllipse.raw(),
            Self::DrawEllipse(_) => EmfPlusRecordType::DrawEllipse.raw(),
            Self::FillPie(_) => EmfPlusRecordType::FillPie.raw(),
            Self::DrawPie(_) => EmfPlusRecordType::DrawPie.raw(),
            Self::DrawArc(_) => EmfPlusRecordType::DrawArc.raw(),
            Self::FillRegion(_) => EmfPlusRecordType::FillRegion.raw(),
            Self::FillPath(_) => EmfPlusRecordType::FillPath.raw(),
            Self::DrawPath => EmfPlusRecordType::DrawPath.raw(),
            Self::ResetClip => EmfPlusRecordType::ResetClip.raw(),
            Self::SetClipRect(_) => EmfPlusRecordType::SetClipRect.raw(),
            Self::SetClipPath => EmfPlusRecordType::SetClipPath.raw(),
            Self::SetClipRegion => EmfPlusRecordType::SetClipRegion.raw(),
            Self::OffsetClip(_) => EmfPlusRecordType::OffsetClip.raw(),
            Self::SetRenderingOrigin(_) => EmfPlusRecordType::SetRenderingOrigin.raw(),
            Self::SetAntiAliasMode => EmfPlusRecordType::SetAntiAliasMode.raw(),
            Self::SetTextRenderingHint => EmfPlusRecordType::SetTextRenderingHint.raw(),
            Self::SetTextContrast => EmfPlusRecordType::SetTextContrast.raw(),
            Self::SetInterpolationMode => EmfPlusRecordType::SetInterpolationMode.raw(),
            Self::SetPixelOffsetMode => EmfPlusRecordType::SetPixelOffsetMode.raw(),
            Self::SetCompositingMode => EmfPlusRecordType::SetCompositingMode.raw(),
            Self::SetCompositingQuality => EmfPlusRecordType::SetCompositingQuality.raw(),
            Self::Save(_) => EmfPlusRecordType::Save.raw(),
            Self::Restore(_) => EmfPlusRecordType::Restore.raw(),
            Self::BeginContainer(_) => EmfPlusRecordType::BeginContainer.raw(),
            Self::BeginContainerNoParams(_) => EmfPlusRecordType::BeginContainerNoParams.raw(),
            Self::EndContainer(_) => EmfPlusRecordType::EndContainer.raw(),
            Self::SetWorldTransform(_) => EmfPlusRecordType::SetWorldTransform.raw(),
            Self::ResetWorldTransform => EmfPlusRecordType::ResetWorldTransform.raw(),
            Self::MultiplyWorldTransform(_) => EmfPlusRecordType::MultiplyWorldTransform.raw(),
            Self::TranslateWorldTransform(_) => EmfPlusRecordType::TranslateWorldTransform.raw(),
            Self::ScaleWorldTransform(_) => EmfPlusRecordType::ScaleWorldTransform.raw(),
            Self::RotateWorldTransform(_) => EmfPlusRecordType::RotateWorldTransform.raw(),
            Self::SetPageTransform(_) => EmfPlusRecordType::SetPageTransform.raw(),
            Self::Unknown(record) => record.record_type,
        }
    }

    pub fn record_kind(&self) -> Option<EmfPlusRecordType> {
        match self {
            Self::Header(_) => Some(EmfPlusRecordType::Header),
            Self::Eof => Some(EmfPlusRecordType::Eof),
            Self::Comment(_) => Some(EmfPlusRecordType::Comment),
            Self::GetDc => Some(EmfPlusRecordType::GetDc),
            Self::Clear(_) => Some(EmfPlusRecordType::Clear),
            Self::FillRects(_) => Some(EmfPlusRecordType::FillRects),
            Self::DrawRects(_) => Some(EmfPlusRecordType::DrawRects),
            Self::FillEllipse(_) => Some(EmfPlusRecordType::FillEllipse),
            Self::DrawEllipse(_) => Some(EmfPlusRecordType::DrawEllipse),
            Self::FillPie(_) => Some(EmfPlusRecordType::FillPie),
            Self::DrawPie(_) => Some(EmfPlusRecordType::DrawPie),
            Self::DrawArc(_) => Some(EmfPlusRecordType::DrawArc),
            Self::FillRegion(_) => Some(EmfPlusRecordType::FillRegion),
            Self::FillPath(_) => Some(EmfPlusRecordType::FillPath),
            Self::DrawPath => Some(EmfPlusRecordType::DrawPath),
            Self::ResetClip => Some(EmfPlusRecordType::ResetClip),
            Self::SetClipRect(_) => Some(EmfPlusRecordType::SetClipRect),
            Self::SetClipPath => Some(EmfPlusRecordType::SetClipPath),
            Self::SetClipRegion => Some(EmfPlusRecordType::SetClipRegion),
            Self::OffsetClip(_) => Some(EmfPlusRecordType::OffsetClip),
            Self::SetRenderingOrigin(_) => Some(EmfPlusRecordType::SetRenderingOrigin),
            Self::SetAntiAliasMode => Some(EmfPlusRecordType::SetAntiAliasMode),
            Self::SetTextRenderingHint => Some(EmfPlusRecordType::SetTextRenderingHint),
            Self::SetTextContrast => Some(EmfPlusRecordType::SetTextContrast),
            Self::SetInterpolationMode => Some(EmfPlusRecordType::SetInterpolationMode),
            Self::SetPixelOffsetMode => Some(EmfPlusRecordType::SetPixelOffsetMode),
            Self::SetCompositingMode => Some(EmfPlusRecordType::SetCompositingMode),
            Self::SetCompositingQuality => Some(EmfPlusRecordType::SetCompositingQuality),
            Self::Save(_) => Some(EmfPlusRecordType::Save),
            Self::Restore(_) => Some(EmfPlusRecordType::Restore),
            Self::BeginContainer(_) => Some(EmfPlusRecordType::BeginContainer),
            Self::BeginContainerNoParams(_) => Some(EmfPlusRecordType::BeginContainerNoParams),
            Self::EndContainer(_) => Some(EmfPlusRecordType::EndContainer),
            Self::SetWorldTransform(_) => Some(EmfPlusRecordType::SetWorldTransform),
            Self::ResetWorldTransform => Some(EmfPlusRecordType::ResetWorldTransform),
            Self::MultiplyWorldTransform(_) => Some(EmfPlusRecordType::MultiplyWorldTransform),
            Self::TranslateWorldTransform(_) => Some(EmfPlusRecordType::TranslateWorldTransform),
            Self::ScaleWorldTransform(_) => Some(EmfPlusRecordType::ScaleWorldTransform),
            Self::RotateWorldTransform(_) => Some(EmfPlusRecordType::RotateWorldTransform),
            Self::SetPageTransform(_) => Some(EmfPlusRecordType::SetPageTransform),
            Self::Unknown(record) => record.record_kind(),
        }
    }

    pub fn sdk_size(&self) -> u64 {
        match self {
            Self::Header(value) => value.sdk_size(),
            Self::Eof | Self::GetDc => 0,
            Self::Comment(value) => value.len() as u64,
            Self::Clear(value) => value.sdk_size(),
            Self::FillRects(value) => {
                8 + value.rects.iter().map(EmfPlusRect::sdk_size).sum::<u64>()
            }
            Self::DrawRects(value) => {
                4 + value.rects.iter().map(EmfPlusRect::sdk_size).sum::<u64>()
            }
            Self::FillEllipse(value) => 4 + value.rect.sdk_size(),
            Self::DrawEllipse(value) => value.rect.sdk_size(),
            Self::FillPie(value) => 12 + value.rect.sdk_size(),
            Self::DrawPie(value) | Self::DrawArc(value) => 8 + value.rect.sdk_size(),
            Self::FillRegion(_) | Self::FillPath(_) => 4,
            Self::DrawPath => 0,
            Self::ResetClip | Self::SetClipPath | Self::SetClipRegion => 0,
            Self::SetClipRect(value) => value.sdk_size(),
            Self::OffsetClip(value) => value.sdk_size(),
            Self::SetRenderingOrigin(value) => value.sdk_size(),
            Self::SetAntiAliasMode
            | Self::SetTextRenderingHint
            | Self::SetTextContrast
            | Self::SetInterpolationMode
            | Self::SetPixelOffsetMode
            | Self::SetCompositingMode
            | Self::SetCompositingQuality => 0,
            Self::Save(value)
            | Self::Restore(value)
            | Self::BeginContainerNoParams(value)
            | Self::EndContainer(value) => value.sdk_size(),
            Self::BeginContainer(value) => value.sdk_size(),
            Self::SetWorldTransform(value) | Self::MultiplyWorldTransform(value) => {
                value.sdk_size()
            }
            Self::ResetWorldTransform => 0,
            Self::TranslateWorldTransform(value) => value.sdk_size(),
            Self::ScaleWorldTransform(value) => value.sdk_size(),
            Self::RotateWorldTransform(value) => value.sdk_size(),
            Self::SetPageTransform(value) => value.sdk_size(),
            Self::Unknown(record) => record.data.len() as u64,
        }
    }

    fn record_flags(&self, flags: EmfPlusRecordFlags) -> EmfPlusRecordFlags {
        match self {
            Self::FillRects(value) => {
                let mut next = flags;
                next.set(
                    EmfPlusRecordFlags::SOLID_COLOR,
                    matches!(value.brush, EmfPlusBrushRef::Color(_)),
                );
                set_rect_flags(next, &value.rects)
            }
            Self::DrawRects(value) => {
                let next = EmfPlusRecordFlags::from_bits_retain(
                    (flags.bits() & !EmfPlusRecordFlags::OBJECT_ID_MASK.bits())
                        | u16::from(value.pen_id),
                );
                set_rect_flags(next, &value.rects)
            }
            Self::FillEllipse(value) => set_brush_and_rect_flags(flags, value.brush, &value.rect),
            Self::DrawEllipse(value) => {
                let next = EmfPlusRecordFlags::from_bits_retain(
                    (flags.bits() & !EmfPlusRecordFlags::OBJECT_ID_MASK.bits())
                        | u16::from(value.pen_id),
                );
                set_rect_flags(next, std::slice::from_ref(&value.rect))
            }
            Self::FillPie(value) => set_brush_and_rect_flags(flags, value.brush, &value.rect),
            Self::DrawPie(value) | Self::DrawArc(value) => {
                let next = EmfPlusRecordFlags::from_bits_retain(
                    (flags.bits() & !EmfPlusRecordFlags::OBJECT_ID_MASK.bits())
                        | u16::from(value.pen_id),
                );
                set_rect_flags(next, std::slice::from_ref(&value.rect))
            }
            Self::FillRegion(value) | Self::FillPath(value) => {
                let mut next = flags;
                next.set(
                    EmfPlusRecordFlags::SOLID_COLOR,
                    matches!(value.brush, EmfPlusBrushRef::Color(_)),
                );
                next
            }
            _ => flags,
        }
    }
}

fn ensure_empty_data(data: &[u8], name: &str) -> Result<()> {
    if data.is_empty() {
        Ok(())
    } else {
        Err(Error::invalid(
            0,
            format!("{name} record has unexpected payload"),
        ))
    }
}

fn read_exact_object<T: SdkRead>(data: &[u8], name: &str) -> Result<T> {
    let mut reader = Reader::new(std::io::Cursor::new(data));
    let value = T::read_from(&mut reader)?;
    let position = reader.position()?;
    if position != data.len() as u64 {
        return Err(Error::invalid(
            position,
            format!("{name} record has trailing data"),
        ));
    }
    Ok(value)
}

fn read_rects<R: std::io::Read + std::io::Seek>(
    reader: &mut Reader<R>,
    count: usize,
    flags: EmfPlusRecordFlags,
    data_len: u64,
) -> Result<Vec<EmfPlusRect>> {
    let offset = reader.position()?;
    let rect_size = if flags.contains(EmfPlusRecordFlags::COMPRESSED) {
        8usize
    } else {
        16usize
    };
    let required = count
        .checked_mul(rect_size)
        .ok_or_else(|| Error::invalid(offset, "EMF+ rectangle payload size overflows usize"))?;
    if offset
        .checked_add(required as u64)
        .is_none_or(|end| end > data_len)
    {
        return Err(Error::invalid(
            offset,
            "EMF+ rectangle payload extends past record data",
        ));
    }

    let mut rects = Vec::with_capacity(count);
    for _ in 0..count {
        rects.push(if flags.contains(EmfPlusRecordFlags::COMPRESSED) {
            EmfPlusRect::Compressed(EmfPlusRectS::read_from(reader)?)
        } else {
            EmfPlusRect::Float(RectF::read_from(reader)?)
        });
    }
    Ok(rects)
}

fn read_single_rect<R: std::io::Read + std::io::Seek>(
    reader: &mut Reader<R>,
    flags: EmfPlusRecordFlags,
    data_len: u64,
) -> Result<EmfPlusRect> {
    let mut rects = read_rects(reader, 1, flags, data_len)?;
    Ok(rects.remove(0))
}

fn read_brush_ref<R: std::io::Read + std::io::Seek>(
    reader: &mut Reader<R>,
    flags: EmfPlusRecordFlags,
) -> Result<EmfPlusBrushRef> {
    if flags.contains(EmfPlusRecordFlags::SOLID_COLOR) {
        Ok(EmfPlusBrushRef::Color(EmfPlusArgb::read_from(reader)?))
    } else {
        Ok(EmfPlusBrushRef::ObjectId(reader.read_u32()?))
    }
}

fn write_brush_ref<W: std::io::Write + std::io::Seek>(
    writer: &mut Writer<W>,
    brush: EmfPlusBrushRef,
) -> Result<()> {
    match brush {
        EmfPlusBrushRef::ObjectId(value) => writer.write_u32(value),
        EmfPlusBrushRef::Color(value) => value.write_to(writer),
    }
}

fn write_rects<W: std::io::Write + std::io::Seek>(
    writer: &mut Writer<W>,
    rects: &[EmfPlusRect],
) -> Result<()> {
    validate_homogeneous_rects(rects)?;
    for rect in rects {
        match rect {
            EmfPlusRect::Compressed(value) => value.write_to(writer)?,
            EmfPlusRect::Float(value) => value.write_to(writer)?,
        }
    }
    Ok(())
}

fn validate_homogeneous_rects(rects: &[EmfPlusRect]) -> Result<()> {
    let Some(first) = rects.first() else {
        return Ok(());
    };
    let first_compressed = matches!(first, EmfPlusRect::Compressed(_));
    if rects
        .iter()
        .any(|rect| matches!(rect, EmfPlusRect::Compressed(_)) != first_compressed)
    {
        return Err(Error::invalid(
            0,
            "EMF+ rectangle payload mixes compressed and floating-point rectangles",
        ));
    }
    Ok(())
}

fn set_rect_flags(flags: EmfPlusRecordFlags, rects: &[EmfPlusRect]) -> EmfPlusRecordFlags {
    let compressed = rects
        .first()
        .is_some_and(|rect| matches!(rect, EmfPlusRect::Compressed(_)));
    let mut next = flags;
    next.set(EmfPlusRecordFlags::COMPRESSED, compressed);
    next
}

fn set_brush_and_rect_flags(
    flags: EmfPlusRecordFlags,
    brush: EmfPlusBrushRef,
    rect: &EmfPlusRect,
) -> EmfPlusRecordFlags {
    let mut next = flags;
    next.set(
        EmfPlusRecordFlags::SOLID_COLOR,
        matches!(brush, EmfPlusBrushRef::Color(_)),
    );
    set_rect_flags(next, std::slice::from_ref(rect))
}

fn len_to_u32(len: usize, name: &str) -> Result<u32> {
    if len > u32::MAX as usize {
        return Err(Error::invalid(0, format!("{name} exceeds u32::MAX")));
    }
    Ok(len as u32)
}

pub fn read_records(bytes: &[u8]) -> Result<Vec<EmfPlusRecord>> {
    let mut reader = Reader::new(std::io::Cursor::new(bytes));
    let mut records = Vec::new();
    while reader.position()? < bytes.len() as u64 {
        records.push(EmfPlusRecord::read_from(&mut reader, bytes.len() as u64)?);
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{SdkRead, SdkSize, SdkWrite};

    #[test]
    fn emf_plus_record_roundtrip_preserves_padding() {
        let bytes = [
            0x01, 0x40, // Type
            0x00, 0x00, // Flags
            0x10, 0x00, 0x00, 0x00, // Size
            0x02, 0x00, 0x00, 0x00, // DataSize
            0xAA, 0xBB, // Data
            0xCC, 0xDD, // Padding
        ];
        let records = read_records(&bytes).unwrap();
        assert_eq!(records[0].data, [0xAA, 0xBB]);
        assert_eq!(records[0].padding, [0xCC, 0xDD]);

        let mut writer = Writer::new(std::io::Cursor::new(Vec::new()));
        records[0].write_to(&mut writer).unwrap();
        assert_eq!(writer.into_inner().into_inner(), bytes);
        assert_eq!(records[0].record_kind(), Some(EmfPlusRecordType::Header));
    }

    #[test]
    fn derived_emf_plus_header_data_roundtrips() {
        let header = EmfPlusHeaderData {
            graphics_version: EmfPlusGraphicsVersion {
                value: (EMFPLUS_METAFILE_SIGNATURE << 12) | 0x0002,
            },
            emf_plus_flags: 1,
            logical_dpi_x: 96,
            logical_dpi_y: 96,
        };

        assert_eq!(
            header.graphics_version.metafile_signature(),
            EMFPLUS_METAFILE_SIGNATURE
        );
        assert_eq!(
            header.graphics_version.graphics_version(),
            Some(EmfPlusGraphicsVersionValue::Version1_1)
        );
        assert_eq!(header.sdk_size(), 16);

        let mut writer = Writer::new(std::io::Cursor::new(Vec::new()));
        header.write_to(&mut writer).unwrap();
        let bytes = writer.into_inner().into_inner();

        let mut reader = Reader::new(std::io::Cursor::new(bytes));
        let parsed = EmfPlusHeaderData::read_from(&mut reader).unwrap();
        assert_eq!(parsed, header);
    }

    #[test]
    fn emf_plus_fill_rects_parse_and_write_roundtrips() {
        let record = EmfPlusRecord {
            record_type: EmfPlusRecordType::FillRects.raw(),
            flags: (EmfPlusRecordFlags::SOLID_COLOR | EmfPlusRecordFlags::COMPRESSED).bits(),
            data: vec![
                0x33, 0x22, 0x11, 0xFF, // ARGB in little-endian byte order
                0x01, 0x00, 0x00, 0x00, // Count
                0x01, 0x00, // x
                0x02, 0x00, // y
                0x03, 0x00, // width
                0x04, 0x00, // height
            ],
            padding: Vec::new(),
        };

        let data = record.parse_data().unwrap();
        assert_eq!(
            data,
            EmfPlusRecordData::FillRects(EmfPlusFillRectsData {
                brush: EmfPlusBrushRef::Color(EmfPlusArgb {
                    blue: 0x33,
                    green: 0x22,
                    red: 0x11,
                    alpha: 0xFF,
                }),
                rects: vec![EmfPlusRect::Compressed(EmfPlusRectS {
                    x: 1,
                    y: 2,
                    width: 3,
                    height: 4,
                })],
            })
        );

        let written = EmfPlusRecord::from_data(&data, EmfPlusRecordFlags::empty()).unwrap();
        assert_eq!(written.record_type, record.record_type);
        assert_eq!(written.flags, record.flags);
        assert_eq!(written.data, record.data);
    }

    #[test]
    fn emf_plus_draw_rects_carries_pen_id_in_flags() {
        let data = EmfPlusRecordData::DrawRects(EmfPlusDrawRectsData {
            pen_id: 7,
            rects: vec![EmfPlusRect::Float(RectF {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
            })],
        });

        let record = EmfPlusRecord::from_data(&data, EmfPlusRecordFlags::empty()).unwrap();
        assert_eq!(record.record_type, EmfPlusRecordType::DrawRects.raw());
        assert_eq!(record.flags().object_id(), 7);
        assert!(!record.flags().contains(EmfPlusRecordFlags::COMPRESSED));
        assert_eq!(record.parse_data().unwrap(), data);
    }

    #[test]
    fn emf_plus_transform_data_roundtrips() {
        let data = EmfPlusRecordData::TranslateWorldTransform(EmfPlusTranslateWorldTransformData {
            dx: 12.5,
            dy: -3.25,
        });
        let flags = EmfPlusRecordFlags::POST_MULTIPLY;

        let record = EmfPlusRecord::from_data(&data, flags).unwrap();
        assert_eq!(
            record.record_type,
            EmfPlusRecordType::TranslateWorldTransform.raw()
        );
        assert!(record.flags().contains(EmfPlusRecordFlags::POST_MULTIPLY));
        assert_eq!(record.parse_data().unwrap(), data);
    }

    fn assert_emf_plus_data_roundtrip(data: EmfPlusRecordData<'_>, flags: EmfPlusRecordFlags) {
        let record = EmfPlusRecord::from_data(&data, flags).unwrap();
        assert_eq!(record.parse_data().unwrap(), data);
        assert_eq!(record.data.len() as u64, data.sdk_size());
    }

    #[test]
    fn emf_plus_control_comment_and_clear_records_roundtrip() {
        assert_emf_plus_data_roundtrip(EmfPlusRecordData::Eof, EmfPlusRecordFlags::empty());
        assert_emf_plus_data_roundtrip(EmfPlusRecordData::GetDc, EmfPlusRecordFlags::empty());
        assert_emf_plus_data_roundtrip(
            EmfPlusRecordData::Comment(vec![1, 2, 3, 4]),
            EmfPlusRecordFlags::empty(),
        );
        assert_emf_plus_data_roundtrip(
            EmfPlusRecordData::Clear(EmfPlusClearData {
                color: EmfPlusArgb {
                    blue: 1,
                    green: 2,
                    red: 3,
                    alpha: 4,
                },
            }),
            EmfPlusRecordFlags::empty(),
        );
    }

    #[test]
    fn emf_plus_clip_and_property_records_roundtrip() {
        assert_emf_plus_data_roundtrip(EmfPlusRecordData::ResetClip, EmfPlusRecordFlags::empty());
        assert_emf_plus_data_roundtrip(
            EmfPlusRecordData::SetClipRect(EmfPlusSetClipRectData {
                clip_rect: RectF {
                    x: 1.0,
                    y: 2.0,
                    width: 3.0,
                    height: 4.0,
                },
            }),
            EmfPlusRecordFlags::from_bits_retain(0x0002),
        );
        assert_emf_plus_data_roundtrip(
            EmfPlusRecordData::SetClipPath,
            EmfPlusRecordFlags::from_bits_retain(0x0100),
        );
        assert_emf_plus_data_roundtrip(
            EmfPlusRecordData::SetClipRegion,
            EmfPlusRecordFlags::from_bits_retain(0x0200),
        );
        assert_emf_plus_data_roundtrip(
            EmfPlusRecordData::OffsetClip(EmfPlusTranslateWorldTransformData { dx: 5.0, dy: 6.0 }),
            EmfPlusRecordFlags::empty(),
        );
        assert_emf_plus_data_roundtrip(
            EmfPlusRecordData::SetAntiAliasMode,
            EmfPlusRecordFlags::from_bits_retain(0x0108),
        );
        assert_emf_plus_data_roundtrip(
            EmfPlusRecordData::SetRenderingOrigin(PointL { x: 10, y: -20 }),
            EmfPlusRecordFlags::empty(),
        );
    }

    #[test]
    fn emf_plus_state_and_transform_records_roundtrip() {
        assert_emf_plus_data_roundtrip(
            EmfPlusRecordData::Save(EmfPlusStackIndexData { stack_index: 7 }),
            EmfPlusRecordFlags::empty(),
        );
        assert_emf_plus_data_roundtrip(
            EmfPlusRecordData::Restore(EmfPlusStackIndexData { stack_index: 7 }),
            EmfPlusRecordFlags::empty(),
        );
        assert_emf_plus_data_roundtrip(
            EmfPlusRecordData::BeginContainerNoParams(EmfPlusStackIndexData { stack_index: 9 }),
            EmfPlusRecordFlags::empty(),
        );
        assert_emf_plus_data_roundtrip(
            EmfPlusRecordData::BeginContainer(EmfPlusBeginContainerData {
                dest_rect: RectF {
                    x: 0.0,
                    y: 1.0,
                    width: 2.0,
                    height: 3.0,
                },
                src_rect: RectF {
                    x: 4.0,
                    y: 5.0,
                    width: 6.0,
                    height: 7.0,
                },
                stack_index: 11,
            }),
            EmfPlusRecordFlags::from_bits_retain(0x0002),
        );
        assert_emf_plus_data_roundtrip(
            EmfPlusRecordData::EndContainer(EmfPlusStackIndexData { stack_index: 11 }),
            EmfPlusRecordFlags::empty(),
        );
        assert_emf_plus_data_roundtrip(
            EmfPlusRecordData::RotateWorldTransform(EmfPlusRotateWorldTransformData {
                angle: 45.0,
            }),
            EmfPlusRecordFlags::POST_MULTIPLY,
        );
        assert_emf_plus_data_roundtrip(
            EmfPlusRecordData::SetPageTransform(EmfPlusSetPageTransformData { page_scale: 1.5 }),
            EmfPlusRecordFlags::from_bits_retain(0x0002),
        );
    }

    #[test]
    fn emf_plus_fixed_drawing_records_roundtrip() {
        assert_emf_plus_data_roundtrip(
            EmfPlusRecordData::FillEllipse(EmfPlusFillRectShapeData {
                brush: EmfPlusBrushRef::Color(EmfPlusArgb {
                    blue: 10,
                    green: 20,
                    red: 30,
                    alpha: 255,
                }),
                rect: EmfPlusRect::Compressed(EmfPlusRectS {
                    x: 1,
                    y: 2,
                    width: 3,
                    height: 4,
                }),
            }),
            EmfPlusRecordFlags::empty(),
        );
        assert_emf_plus_data_roundtrip(
            EmfPlusRecordData::DrawEllipse(EmfPlusDrawRectShapeData {
                pen_id: 6,
                rect: EmfPlusRect::Float(RectF {
                    x: 1.0,
                    y: 2.0,
                    width: 3.0,
                    height: 4.0,
                }),
            }),
            EmfPlusRecordFlags::empty(),
        );
        assert_emf_plus_data_roundtrip(
            EmfPlusRecordData::FillPie(EmfPlusFillPieData {
                brush: EmfPlusBrushRef::ObjectId(3),
                start_angle: 45.0,
                sweep_angle: 90.0,
                rect: EmfPlusRect::Compressed(EmfPlusRectS {
                    x: 5,
                    y: 6,
                    width: 7,
                    height: 8,
                }),
            }),
            EmfPlusRecordFlags::empty(),
        );
        assert_emf_plus_data_roundtrip(
            EmfPlusRecordData::DrawArc(EmfPlusDrawArcData {
                pen_id: 4,
                start_angle: 10.0,
                sweep_angle: -20.0,
                rect: EmfPlusRect::Compressed(EmfPlusRectS {
                    x: 9,
                    y: 10,
                    width: 11,
                    height: 12,
                }),
            }),
            EmfPlusRecordFlags::empty(),
        );
        assert_emf_plus_data_roundtrip(
            EmfPlusRecordData::FillPath(EmfPlusBrushObjectData {
                brush: EmfPlusBrushRef::ObjectId(2),
            }),
            EmfPlusRecordFlags::from_bits_retain(0x0300),
        );
        assert_emf_plus_data_roundtrip(
            EmfPlusRecordData::FillRegion(EmfPlusBrushObjectData {
                brush: EmfPlusBrushRef::Color(EmfPlusArgb {
                    blue: 1,
                    green: 2,
                    red: 3,
                    alpha: 4,
                }),
            }),
            EmfPlusRecordFlags::from_bits_retain(0x0500),
        );
        assert_emf_plus_data_roundtrip(
            EmfPlusRecordData::DrawPath,
            EmfPlusRecordFlags::from_bits_retain(0x0500),
        );
    }
}
