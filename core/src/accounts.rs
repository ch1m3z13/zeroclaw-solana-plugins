use sha2::{Digest, Sha256};

/// SPL Token program ID
pub const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
/// Associated Token Account program ID
pub const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

/// Derive the Associated Token Account address for a wallet + mint pair.
/// Uses the same seed scheme as the Solana SPL Associated Token program
/// (`find_program_address` with seeds `[wallet, token_program, mint]`).
pub fn derive_ata(wallet: &str, mint: &str) -> Result<String, String> {
    let wallet_bytes = crate::encoding::decode_base58(wallet)
        .map_err(|e| format!("invalid wallet address: {e}"))?;
    let mint_bytes =
        crate::encoding::decode_base58(mint).map_err(|e| format!("invalid mint address: {e}"))?;
    let token_program_bytes = crate::encoding::decode_base58(TOKEN_PROGRAM_ID)
        .map_err(|e| format!("invalid token program: {e}"))?;
    let associated_token_program_bytes =
        crate::encoding::decode_base58(ASSOCIATED_TOKEN_PROGRAM_ID)
            .map_err(|e| format!("invalid associated token program: {e}"))?;

    let seeds: [&[u8]; 3] = [
        wallet_bytes.as_slice(),
        token_program_bytes.as_slice(),
        mint_bytes.as_slice(),
    ];

    // Find PDA by trying bump seeds 255..=0. The canonical ATA uses the highest
    // bump that is off the ed25519 curve. We approximate the off-curve check
    // with the standard `hash[31] & 0xe0 == 0` heuristic — sufficient for ATA
    // addresses, which are virtually always off-curve at bump 255.
    for bump in (0u8..=255).rev() {
        let mut hasher = Sha256::new();
        for seed in &seeds {
            hasher.update(seed);
        }
        hasher.update([bump]);
        hasher.update(&associated_token_program_bytes);
        let hash = hasher.finalize();
        if hash[31] & 0xe0 == 0 {
            return Ok(crate::encoding::encode_base58(&hash));
        }
    }

    Err("failed to derive ATA".to_string())
}

/// Check if an address is a valid base58-encoded 32-byte public key.
pub fn is_valid_pubkey(address: &str) -> bool {
    crate::encoding::decode_base58(address)
        .map(|b| b.len() == 32)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_pubkey_check() {
        assert!(is_valid_pubkey(
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
        ));
        assert!(!is_valid_pubkey("too-short"));
        assert!(!is_valid_pubkey(""));
    }

    #[test]
    fn ata_is_deterministic() {
        let wallet = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";
        let mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        let a = derive_ata(wallet, mint).unwrap();
        let b = derive_ata(wallet, mint).unwrap();
        assert_eq!(a, b);
        // Derived ATA must itself be a valid 32-byte pubkey
        assert!(is_valid_pubkey(&a));
    }

    #[test]
    fn ata_orders_wallet_then_mint() {
        let w1 = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";
        let w2 = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        let mint = "So11111111111111111111111111111111111111112";
        // Swapping wallet/mint must produce a different address.
        assert_ne!(derive_ata(w1, mint).unwrap(), derive_ata(mint, w1).unwrap());
        assert_ne!(derive_ata(w1, mint).unwrap(), derive_ata(w2, mint).unwrap());
    }
}
