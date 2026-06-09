#[derive(Debug)]
pub struct DeserializeError;

impl std::error::Error for DeserializeError {}

impl std::fmt::Display for DeserializeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Invalid input to deserialize method")
    }
}

pub fn get_i32(dst: &mut i32, input: &mut &[u8]) -> Result<(), DeserializeError> {
    if input.len() < 4 {
        return Err(DeserializeError);
    }
    let (int_bytes, rest) = input.split_at(std::mem::size_of::<i32>());
    *input = rest;
    *dst = i32::from_be_bytes(int_bytes.try_into().unwrap());
    Ok(())
}

pub fn get_u32(dst: &mut u32, input: &mut &[u8]) -> Result<(), DeserializeError> {
    if input.len() < 4 {
        return Err(DeserializeError);
    }
    let (int_bytes, rest) = input.split_at(std::mem::size_of::<u32>());
    *input = rest;
    *dst = u32::from_be_bytes(int_bytes.try_into().unwrap());
    Ok(())
}

pub fn get_i64(dst: &mut i64, input: &mut &[u8]) -> Result<(), DeserializeError> {
    if input.len() < 8 {
        return Err(DeserializeError);
    }
    let (int_bytes, rest) = input.split_at(std::mem::size_of::<i64>());
    *input = rest;
    *dst = i64::from_be_bytes(int_bytes.try_into().unwrap());
    Ok(())
}

pub fn get_u64(dst: &mut u64, input: &mut &[u8]) -> Result<(), DeserializeError> {
    if input.len() < 8 {
        return Err(DeserializeError);
    }
    let (int_bytes, rest) = input.split_at(std::mem::size_of::<u64>());
    *input = rest;
    *dst = u64::from_be_bytes(int_bytes.try_into().unwrap());
    Ok(())
}

pub fn get_bool(dst: &mut bool, input: &mut &[u8]) -> Result<(), DeserializeError> {
    if input.len() < 4 {
        return Err(DeserializeError);
    }
    let (bool_bytes, rest) = input.split_at(std::mem::size_of::<u32>());
    *input = rest;
    *dst = !matches!(u32::from_be_bytes(bool_bytes.try_into().unwrap()), 0);
    Ok(())
}

pub fn serialize_bool(src: &bool) -> [u8; 4] {
    match src {
        true => 1_u32.to_be_bytes(),
        false => 0_u32.to_be_bytes(),
    }
}

pub fn encode_padding(offset: usize, buf: &mut [u8]) -> usize {
    let padded_offset: usize = (offset + 3) & !(0b11usize);
    buf[offset..padded_offset].fill(0u8);
    padded_offset
}
