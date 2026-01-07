use thiserror::Error;

pub const ZCASH_PCZT_UR_TYPE: &str = "zcash-pczt";

#[derive(Debug, Error)]
pub enum ZcashPcztUrCborError {
    #[error("unexpected end of CBOR")]
    UnexpectedEnd,
    #[error("unsupported CBOR additional info")]
    UnsupportedAdditionalInfo,
    #[error("invalid zcash-pczt CBOR: {0}")]
    Invalid(&'static str),
}

struct CborHead {
    major: u8,
    value: u64,
    offset: usize,
    indefinite: bool,
}

fn encode_cbor_major_len(major: u8, len: usize) -> Vec<u8> {
    if len < 24 {
        return vec![(major << 5) | (len as u8)];
    }
    if len < 0x100 {
        return vec![(major << 5) | 24, len as u8];
    }
    if len < 0x10000 {
        return vec![(major << 5) | 25, (len >> 8) as u8, (len & 0xff) as u8];
    }
    if len < 0x1_0000_0000 {
        let n = len as u32;
        return vec![
            (major << 5) | 26,
            (n >> 24) as u8,
            (n >> 16) as u8,
            (n >> 8) as u8,
            (n & 0xff) as u8,
        ];
    }

    let n = len as u64;
    vec![
        (major << 5) | 27,
        (n >> 56) as u8,
        (n >> 48) as u8,
        (n >> 40) as u8,
        (n >> 32) as u8,
        (n >> 24) as u8,
        (n >> 16) as u8,
        (n >> 8) as u8,
        (n & 0xff) as u8,
    ]
}

fn read_head(bytes: &[u8], offset: usize) -> Result<CborHead, ZcashPcztUrCborError> {
    let initial = *bytes
        .get(offset)
        .ok_or(ZcashPcztUrCborError::UnexpectedEnd)?;
    let major = initial >> 5;
    let ai = initial & 0x1f;
    let mut next = offset + 1;

    if ai < 24 {
        return Ok(CborHead {
            major,
            value: ai as u64,
            offset: next,
            indefinite: false,
        });
    }

    match ai {
        24 => {
            let value = *bytes.get(next).ok_or(ZcashPcztUrCborError::UnexpectedEnd)? as u64;
            next += 1;
            Ok(CborHead {
                major,
                value,
                offset: next,
                indefinite: false,
            })
        }
        25 => {
            let b0 = *bytes.get(next).ok_or(ZcashPcztUrCborError::UnexpectedEnd)? as u64;
            let b1 = *bytes
                .get(next + 1)
                .ok_or(ZcashPcztUrCborError::UnexpectedEnd)? as u64;
            next += 2;
            Ok(CborHead {
                major,
                value: (b0 << 8) | b1,
                offset: next,
                indefinite: false,
            })
        }
        26 => {
            let b0 = *bytes.get(next).ok_or(ZcashPcztUrCborError::UnexpectedEnd)? as u64;
            let b1 = *bytes
                .get(next + 1)
                .ok_or(ZcashPcztUrCborError::UnexpectedEnd)? as u64;
            let b2 = *bytes
                .get(next + 2)
                .ok_or(ZcashPcztUrCborError::UnexpectedEnd)? as u64;
            let b3 = *bytes
                .get(next + 3)
                .ok_or(ZcashPcztUrCborError::UnexpectedEnd)? as u64;
            next += 4;
            Ok(CborHead {
                major,
                value: (b0 << 24) | (b1 << 16) | (b2 << 8) | b3,
                offset: next,
                indefinite: false,
            })
        }
        27 => {
            let mut n: u64 = 0;
            for i in 0..8 {
                let b = *bytes
                    .get(next + i)
                    .ok_or(ZcashPcztUrCborError::UnexpectedEnd)? as u64;
                n = (n << 8) | b;
            }
            next += 8;
            Ok(CborHead {
                major,
                value: n,
                offset: next,
                indefinite: false,
            })
        }
        31 => Ok(CborHead {
            major,
            value: 0,
            offset: next,
            indefinite: true,
        }),
        _ => Err(ZcashPcztUrCborError::UnsupportedAdditionalInfo),
    }
}

pub fn encode_zcash_pczt_ur_cbor(pczt_bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + 1 + 9 + pczt_bytes.len());
    out.extend_from_slice(&encode_cbor_major_len(5, 1)); // map(1)
    out.extend_from_slice(&encode_cbor_major_len(0, 1)); // key 1
    out.extend_from_slice(&encode_cbor_major_len(2, pczt_bytes.len())); // bytes
    out.extend_from_slice(pczt_bytes);
    out
}

pub fn decode_zcash_pczt_ur_cbor(cbor: &[u8]) -> Result<Vec<u8>, ZcashPcztUrCborError> {
    let mut offset = 0;

    let map = read_head(cbor, offset)?;
    offset = map.offset;
    if map.major != 5 || map.indefinite || map.value != 1 {
        return Err(ZcashPcztUrCborError::Invalid("expected map(1)"));
    }

    let key = read_head(cbor, offset)?;
    offset = key.offset;
    if key.major != 0 || key.indefinite || key.value != 1 {
        return Err(ZcashPcztUrCborError::Invalid("expected key 1"));
    }

    let value = read_head(cbor, offset)?;
    offset = value.offset;
    if value.major != 2 {
        return Err(ZcashPcztUrCborError::Invalid("expected byte string"));
    }

    if !value.indefinite {
        let len: usize = value
            .value
            .try_into()
            .map_err(|_| ZcashPcztUrCborError::Invalid("byte string too large"))?;
        if offset + len > cbor.len() {
            return Err(ZcashPcztUrCborError::Invalid("truncated data"));
        }
        let bytes = cbor[offset..offset + len].to_vec();
        offset += len;
        if offset != cbor.len() {
            return Err(ZcashPcztUrCborError::Invalid("trailing bytes"));
        }
        return Ok(bytes);
    }

    let mut chunks: Vec<Vec<u8>> = Vec::new();
    while offset < cbor.len() {
        if cbor[offset] == 0xff {
            offset += 1;
            break;
        }

        let chunk = read_head(cbor, offset)?;
        offset = chunk.offset;
        if chunk.major != 2 || chunk.indefinite {
            return Err(ZcashPcztUrCborError::Invalid("invalid chunk"));
        }

        let len: usize = chunk
            .value
            .try_into()
            .map_err(|_| ZcashPcztUrCborError::Invalid("chunk too large"))?;
        if offset + len > cbor.len() {
            return Err(ZcashPcztUrCborError::Invalid("truncated chunk"));
        }

        chunks.push(cbor[offset..offset + len].to_vec());
        offset += len;
    }

    if offset != cbor.len() {
        return Err(ZcashPcztUrCborError::Invalid("trailing bytes"));
    }

    let total: usize = chunks.iter().map(|c| c.len()).sum();
    let mut out = Vec::with_capacity(total);
    for c in chunks {
        out.extend_from_slice(&c);
    }
    Ok(out)
}
