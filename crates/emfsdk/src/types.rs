use emfsdk_derive::SdkObject;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "shared")]
pub struct PointL {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "shared")]
pub struct PointS {
    pub x: i16,
    pub y: i16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "shared")]
pub struct SizeL {
    pub cx: i32,
    pub cy: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "shared")]
pub struct SizeS {
    pub cx: i16,
    pub cy: i16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "shared")]
pub struct RectL {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "shared")]
pub struct RectS {
    pub left: i16,
    pub top: i16,
    pub right: i16,
    pub bottom: i16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "shared")]
pub struct ColorRef {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub reserved: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "emfplus")]
pub struct EmfPlusArgb {
    pub blue: u8,
    pub green: u8,
    pub red: u8,
    pub alpha: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, SdkObject)]
#[sdk(format = "emf")]
pub struct XForm {
    pub m11: f32,
    pub m12: f32,
    pub m21: f32,
    pub m22: f32,
    pub dx: f32,
    pub dy: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, SdkObject)]
#[sdk(format = "emfplus")]
pub struct PointF {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, SdkObject)]
#[sdk(format = "emfplus")]
pub struct SizeF {
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, SdkObject)]
#[sdk(format = "emfplus")]
pub struct RectF {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, SdkObject)]
#[sdk(format = "emf")]
pub struct TriVertex {
    pub x: i32,
    pub y: i32,
    pub red: u16,
    pub green: u16,
    pub blue: u16,
    pub alpha: u16,
}
