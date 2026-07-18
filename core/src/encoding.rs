/// Encode bytes to base58 (Solana addresses, signatures).
pub fn encode_base58(data: &[u8]) -> String {
    bs58::encode(data).into_string()
}

/// Decode base58 string to bytes.
pub fn decode_base58(s: &str) -> Result<Vec<u8>, String> {
    bs58::decode(s).into_vec().map_err(|e| format!("base58: {e}"))
}

fn b64_table() -> [i16; 256] {
    let mut t = [-1i16; 256];
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for (i, &b) in alphabet.iter().enumerate() {
        t[b as usize] = i as i16;
    }
    t
}

/// Encode bytes to standard base64.
pub fn encode_base64(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(data.len() * 4 / 3 + 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        result.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(TABLE[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

/// Decode standard base64 string to bytes.
pub fn decode_base64(s: &str) -> Result<Vec<u8>, String> {
    let table = b64_table();
    let s = s.trim_end_matches('=');
    let bytes = s.as_bytes();
    let mut result = Vec::with_capacity(s.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let n = chunk.len();
        let mut buf = [0u8; 4];
        for i in 0..n {
            let v = table[chunk[i] as usize];
            if v < 0 {
                return Err("invalid base64".to_string());
            }
            buf[i] = v as u8;
        }
        if n >= 1 {
            result.push((buf[0] << 2) | (buf[1] >> 4));
        }
        if n >= 3 {
            result.push((buf[1] << 4) | (buf[2] >> 2));
        }
        if n >= 4 {
            result.push((buf[2] << 6) | buf[3]);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base58_roundtrip() {
        let data = b"hello solana";
        let encoded = encode_base58(data);
        let decoded = decode_base58(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn base64_roundtrip() {
        let data = b"hello solana";
        let encoded = encode_base64(data);
        let decoded = decode_base64(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn base58_known_address() {
        // USDC token mint
        let addr = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        let bytes = decode_base58(addr).unwrap();
        assert_eq!(bytes.len(), 32);
        let re_encoded = encode_base58(&bytes);
        assert_eq!(re_encoded, addr);
    }

    #[test]
    fn base64_with_padding() {
        assert_eq!(decode_base64(&encode_base64(b"a")).unwrap(), b"a");
        assert_eq!(decode_base64(&encode_base64(b"ab")).unwrap(), b"ab");
        assert_eq!(decode_base64(&encode_base64(b"abc")).unwrap(), b"abc");
    }
}
