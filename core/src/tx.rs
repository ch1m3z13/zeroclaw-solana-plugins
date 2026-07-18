use crate::types::Blockhash;

/// System program ID
pub const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
/// SPL Token program ID
pub const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

/// An instruction to be included in a transaction.
pub struct Instruction {
    pub program_id: String,
    pub accounts: Vec<AccountMeta>,
    pub data: Vec<u8>,
}

/// Account metadata for an instruction.
pub struct AccountMeta {
    pub pubkey: String,
    pub is_signer: bool,
    pub is_writable: bool,
}

/// Build a legacy transaction (non-versioned) from instructions.
/// Returns the serialized transaction bytes (unsigned).
///
/// This is a T1 operation — the transaction is NOT signed.
/// The human or host signs it before submission.
pub fn build_legacy_transaction(
    instructions: &[Instruction],
    payer: &str,
    recent_blockhash: &Blockhash,
) -> Result<Vec<u8>, String> {
    // Message layout:
    // header (3 bytes) + account_keys + recent_blockhash + instructions

    // Collect all unique accounts
    let mut account_keys: Vec<String> = Vec::new();
    // Payer is always first and writable+signer
    account_keys.push(payer.to_string());

    for ix in instructions {
        for acc in &ix.accounts {
            if !account_keys.contains(&acc.pubkey) {
                account_keys.push(acc.pubkey.clone());
            }
        }
        // Also add program_id if not already present
        if !account_keys.contains(&ix.program_id) {
            account_keys.push(ix.program_id.clone());
        }
    }

    // Build header
    let num_signers = account_keys
        .iter()
        .filter(|k| {
            instructions
                .iter()
                .flat_map(|ix| &ix.accounts)
                .any(|a| &a.pubkey == *k && a.is_signer)
                || *k == payer
        })
        .count() as u8;
    let num_readonly_signed = 0u8; // simplified: signers are always writable here
    let num_readonly_unsigned = account_keys
        .iter()
        .filter(|k| {
            let is_signer = *k == payer
                || instructions
                    .iter()
                    .flat_map(|ix| &ix.accounts)
                    .any(|a| &a.pubkey == *k && a.is_signer);
            let is_writable = *k == payer
                || instructions
                    .iter()
                    .flat_map(|ix| &ix.accounts)
                    .any(|a| &a.pubkey == *k && a.is_writable);
            !is_signer && !is_writable
        })
        .count() as u8;

    let mut message = Vec::new();
    // Header
    message.push(num_signers);
    message.push(num_readonly_signed);
    message.push(num_readonly_unsigned);

    // Account keys (base58 decoded, length-prefixed)
    for key in &account_keys {
        let key_bytes = crate::encoding::decode_base58(key)
            .map_err(|e| format!("invalid account key {}: {}", key, e))?;
        message.push(key_bytes.len() as u8);
        message.extend_from_slice(&key_bytes);
    }

    // Recent blockhash (32 bytes)
    let hash_bytes = crate::encoding::decode_base58(&recent_blockhash.0)
        .map_err(|e| format!("invalid blockhash: {}", e))?;
    message.extend_from_slice(&hash_bytes);

    // Instructions count
    message.push(instructions.len() as u8);

    // Each instruction
    for ix in instructions {
        // program_id index
        let program_idx = account_keys
            .iter()
            .position(|k| k == &ix.program_id)
            .ok_or(format!("program {} not in account keys", ix.program_id))?;
        message.push(program_idx as u8);

        // accounts count + indices
        message.push(ix.accounts.len() as u8);
        for acc in &ix.accounts {
            let idx = account_keys
                .iter()
                .position(|k| k == &acc.pubkey)
                .ok_or(format!("account {} not in account keys", acc.pubkey))?;
            message.push(idx as u8);
        }

        // instruction data
        message.push(ix.data.len() as u8);
        message.extend_from_slice(&ix.data);
    }

    // Wrap in a Transaction: [signatures_count(1) | signature(64 zeros) | message]
    let mut tx = Vec::new();
    tx.push(1u8); // one signer (the payer)
    tx.extend_from_slice(&[0u8; 64]); // placeholder signature
    tx.extend_from_slice(&message);

    Ok(tx)
}

/// Encode a transfer instruction (SPL Token).
/// Builds the instruction data for a `Transfer` instruction.
pub fn encode_token_transfer(
    source: &str,
    destination: &str,
    authority: &str,
    amount: u64,
) -> Instruction {
    // SPL Token Transfer instruction layout:
    // [3] = instruction index for Transfer
    // [amount: 8 bytes LE]
    let mut data = vec![3u8]; // Transfer instruction
    data.extend_from_slice(&amount.to_le_bytes());

    Instruction {
        program_id: TOKEN_PROGRAM_ID.to_string(),
        accounts: vec![
            AccountMeta { pubkey: source.to_string(), is_signer: false, is_writable: true },
            AccountMeta { pubkey: destination.to_string(), is_signer: false, is_writable: true },
            AccountMeta { pubkey: authority.to_string(), is_signer: true, is_writable: false },
        ],
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_simple_transfer() {
        let payer = "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU";
        let source = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        let dest = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";

        let ix = encode_token_transfer(source, dest, payer, 1_000_000);
        let blockhash = Blockhash("EkSnNWid2cvTkEVWBQbGxwdj2Udd38dzX7iPpdhn6JFj".into());

        let tx = build_legacy_transaction(&[ix], payer, &blockhash).unwrap();
        // tx = 1 byte sig_count + 64 bytes sig + message
        assert!(tx.len() > 65);
        assert_eq!(tx[0], 1); // one signature slot
    }
}
