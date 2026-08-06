#[derive(Clone, Debug, PartialEq)]
pub enum IntOp {
    LShift(usize),
    RShift(usize),
    LUT32([u32; 16]),
    And(u64),
}

pub struct IntInfo {
    pub name: String,
    pub ops: Vec<IntOp>,
}

pub enum LayoutData {
    NativeU32(Vec<IntInfo>),
    NativeI32(Vec<IntInfo>),
    NativeU64(Vec<IntInfo>),
    NativeI64(Vec<IntInfo>),
    CString(String),
    Unused(usize),
}

pub enum PaddingMode {
    Total(usize),
    Align(usize),
    Packed,
}

pub struct SerLayout {
    pub name: String,
    pub maps_to: String,
    pub padding: PaddingMode,
    pub members: Vec<LayoutData>,
}
