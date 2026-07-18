use crate::types::{Extension, MintInfo};

/// SPL Token mint account layout (82 bytes):
/// - mint_authority (36 bytes: 4-byte option tag + 32-byte pubkey)
/// - supply (8 bytes, little-endian)
/// - decimals (1 byte)
/// - is_initialized (1 byte, bool)
/// - freeze_authority (36 bytes: 4-byte option tag + 32-byte pubkey)
const MINT_SIZE: usize = 82;

/// Decode a standard SPL Token mint account from raw bytes.
pub fn decode_mint(data: &[u8], address: &str) -> Result<MintInfo, String> {
    if data.len() < MINT_SIZE {
        return Err(format!(
            "mint data too short: {} bytes, need {}",
            data.len(),
            MINT_SIZE
        ));
    }

    let mint_authority = decode_pubkey_option(&data[0..36]);
    let supply = u64::from_le_bytes(
        data[36..44]
            .try_into()
            .map_err(|_| "supply parse")?,
    );
    let decimals = data[44];
    let is_initialized = data[45] != 0;
    let freeze_authority = decode_pubkey_option(&data[46..82]);

    Ok(MintInfo {
        address: address.to_string(),
        mint_authority,
        freeze_authority,
        supply,
        decimals,
        is_initialized,
    })
}

/// Decode Token-2022 extensions from the account data.
/// Token-2022 mints have the standard 65-byte mint data, followed by
/// extension data in TLV (type-length-value) format starting at offset 65.
///
/// TLV format:
/// - type: u16 (little-endian)
/// - length: u16 (little-endian)
/// - value: [u8; length]
pub fn decode_extensions(data: &[u8]) -> Result<Vec<Extension>, String> {
    if data.len() <= MINT_SIZE {
        return Ok(vec![]);
    }

    let mut extensions = Vec::new();
    let mut offset = MINT_SIZE;

    while offset + 4 <= data.len() {
        let type_id =
            u16::from_le_bytes(data[offset..offset + 2].try_into().map_err(|_| "type parse")?);
        let length = u16::from_le_bytes(
            data[offset + 2..offset + 4]
                .try_into()
                .map_err(|_| "length parse")?,
        );
        let end = offset + 4 + length as usize;

        if end > data.len() {
            break; // incomplete extension, stop
        }

        let value = &data[offset + 4..end];

        let ext = match type_id {
            // TransferFee (type 0)
            0 => {
                if value.len() >= 10 {
                    let fee_basis_points = u16::from_le_bytes(value[0..2].try_into().unwrap());
                    let max_fee = u64::from_le_bytes(value[2..10].try_into().unwrap());
                    Extension::TransferFee {
                        fee_basis_points,
                        max_fee,
                    }
                } else {
                    Extension::Unknown { type_id }
                }
            }
            // TransferHook (type 3)
            3 => {
                if value.len() >= 32 {
                    let program_id = crate::encoding::encode_base58(&value[..32]);
                    Extension::TransferHook { program_id }
                } else {
                    Extension::Unknown { type_id }
                }
            }
            // NonTransferable (type 8)
            8 => Extension::NonTransferable,
            // InterestBearingConfig (type 9)
            9 => {
                if value.len() >= 34 {
                    let rate_authority = crate::encoding::encode_base58(&value[..32]);
                    let rate = i16::from_le_bytes(value[32..34].try_into().unwrap());
                    Extension::InterestBearing {
                        rate_authority,
                        rate,
                    }
                } else {
                    Extension::Unknown { type_id }
                }
            }
            // PermanentDelegate (type 14)
            14 => {
                if value.len() >= 32 {
                    let delegate = crate::encoding::encode_base58(&value[..32]);
                    Extension::PermanentDelegate { delegate }
                } else {
                    Extension::Unknown { type_id }
                }
            }
            // CloseAuthority (type 6)
            6 => {
                if value.len() >= 32 {
                    let authority = crate::encoding::encode_base58(&value[..32]);
                    Extension::CloseAuthority { authority }
                } else {
                    Extension::Unknown { type_id }
                }
            }
            // DefaultAccountState (type 2)
            2 => {
                if !value.is_empty() {
                    let state = match value[0] {
                        0 => "initialized",
                        1 => "frozen",
                        _ => "unknown",
                    };
                    Extension::DefaultAccountState {
                        state: state.to_string(),
                    }
                } else {
                    Extension::Unknown { type_id }
                }
            }
            _ => Extension::Unknown { type_id },
        };

        extensions.push(ext);
        offset = end;
    }

    Ok(extensions)
}

/// Decode a pubkey option (4-byte tag + 32-byte key).
/// Tag 0 = None, Tag 1 = Some(key).
fn decode_pubkey_option(data: &[u8]) -> Option<String> {
    if data.len() < 36 {
        return None;
    }
    let tag = u32::from_le_bytes(data[0..4].try_into().unwrap());
    if tag == 0 {
        None
    } else {
        Some(crate::encoding::encode_base58(&data[4..36]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mint_data(
        mint_auth: Option<[u8; 32]>,
        supply: u64,
        decimals: u8,
        freeze_auth: Option<[u8; 32]>,
    ) -> Vec<u8> {
        let mut data = vec![0u8; MINT_SIZE];
        if let Some(key) = mint_auth {
            data[0..4].copy_from_slice(&[1, 0, 0, 0]); // Some tag
            data[4..36].copy_from_slice(&key);
        }
        data[36..44].copy_from_slice(&supply.to_le_bytes());
        data[44] = decimals;
        data[45] = 1; // is_initialized
        if let Some(key) = freeze_auth {
            data[46..50].copy_from_slice(&[1, 0, 0, 0]); // Some tag
            data[50..82].copy_from_slice(&key);
        }
        data
    }

    #[test]
    fn decode_mint_no_authority() {
        let data = make_mint_data(None, 1_000_000, 6, None);
        let mint = decode_mint(&data, "testMint111111111111111111111111111111111").unwrap();
        assert_eq!(mint.mint_authority, None);
        assert_eq!(mint.freeze_authority, None);
        assert_eq!(mint.supply, 1_000_000);
        assert_eq!(mint.decimals, 6);
        assert!(mint.is_initialized);
    }

    #[test]
    fn decode_mint_with_authority() {
        let key = [1u8; 32];
        let data = make_mint_data(Some(key), 500, 9, Some(key));
        let mint = decode_mint(&data, "testMint111111111111111111111111111111111").unwrap();
        assert!(mint.mint_authority.is_some());
        assert!(mint.freeze_authority.is_some());
    }

    #[test]
    fn decode_mint_too_short() {
        let data = vec![0u8; 10];
        assert!(decode_mint(&data, "test").is_err());
    }

    #[test]
    fn decode_extensions_tlv() {
        // Build a Token-2022 mint: 65-byte base + one NonTransferable (type 8, len 0)
        // followed by a PermanentDelegate (type 14, len 32).
        let mut data = vec![0u8; MINT_SIZE];
        // NonTransferable: type=8 (LE), length=0
        data.extend_from_slice(&[8, 0, 0, 0]);
        // PermanentDelegate: type=14 (LE), length=32, then 32-byte delegate
        data.extend_from_slice(&[14, 0, 32, 0]);
        data.extend_from_slice(&[7u8; 32]);

        let exts = decode_extensions(&data).unwrap();
        assert_eq!(exts.len(), 2);
        assert!(matches!(exts[0], Extension::NonTransferable));
        assert!(matches!(exts[1], Extension::PermanentDelegate { .. }));
    }
}
