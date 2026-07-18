use crate::types::{PayRequest, PayResult};

/// Build a Solana Pay `solana:` transfer-request URL.
///
/// Format: `solana:<recipient>?amount=<amount>&spl-token=<mint>[&memo=<memo>][&reference=<ref>]`
///
/// This is a T1 operation — the URL contains no unsigned transaction.
/// The payer's wallet builds the transaction at scan time with a fresh blockhash.
pub fn build_pay_url(req: &PayRequest) -> Result<PayResult, String> {
    // Validate recipient is a valid pubkey
    if !crate::accounts::is_valid_pubkey(&req.recipient) {
        return Err(format!("invalid recipient address: {}", req.recipient));
    }

    // Validate amount > 0
    if req.amount <= 0.0 {
        return Err("amount must be greater than zero".to_string());
    }

    // Validate mint is a valid pubkey
    if !crate::accounts::is_valid_pubkey(&req.mint) {
        return Err(format!("invalid mint address: {}", req.mint));
    }

    // Build URL
    let mut url = format!("solana:{}?amount={}", req.recipient, req.amount);
    url.push_str(&format!("&spl-token={}", req.mint));

    if let Some(memo) = &req.memo {
        if !memo.is_empty() {
            url.push_str(&format!("&memo={}", url_encode(memo)));
        }
    }

    if let Some(reference) = &req.reference {
        if !reference.is_empty() {
            if !crate::accounts::is_valid_pubkey(reference) {
                return Err(format!("invalid reference address: {}", reference));
            }
            url.push_str(&format!("&reference={}", reference));
        }
    }

    // Build human-readable summary
    let memo_display = req.memo.as_deref().unwrap_or("no memo");
    let summary = format!(
        "Charge {} to recipient {} for '{}'. Solana Pay URL ready.",
        req.amount,
        &req.recipient[..8.min(req.recipient.len())],
        memo_display
    );

    Ok(PayResult {
        url,
        summary,
        risk: "pending".to_string(), // caller fills in after risk check
        qr_ready: true,
    })
}

/// URL-encode a string (RFC 3986).
fn url_encode(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 3);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            _ => {
                result.push('%');
                result.push_str(&format!("{:02X}", byte));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_recipient() -> String {
        "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU".to_string()
    }

    fn valid_mint() -> String {
        "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string()
    }

    #[test]
    fn basic_pay_url() {
        let req = PayRequest {
            recipient: valid_recipient(),
            amount: 25.0,
            mint: valid_mint(),
            memo: Some("Table 4 payment".into()),
            reference: None,
        };
        let result = build_pay_url(&req).unwrap();
        assert!(result.url.starts_with("solana:"));
        assert!(result.url.contains("amount=25"));
        assert!(result.url.contains("spl-token="));
        assert!(result.url.contains("memo=Table%204%20payment"));
        assert!(result.qr_ready);
    }

    #[test]
    fn pay_url_with_reference() {
        let req = PayRequest {
            recipient: valid_recipient(),
            amount: 10.0,
            mint: valid_mint(),
            memo: None,
            reference: Some(valid_recipient()), // using same key for simplicity
        };
        let result = build_pay_url(&req).unwrap();
        assert!(result.url.contains("reference="));
    }

    #[test]
    fn invalid_recipient_fails() {
        let req = PayRequest {
            recipient: "not-a-valid-key".into(),
            amount: 10.0,
            mint: valid_mint(),
            memo: None,
            reference: None,
        };
        assert!(build_pay_url(&req).is_err());
    }

    #[test]
    fn zero_amount_fails() {
        let req = PayRequest {
            recipient: valid_recipient(),
            amount: 0.0,
            mint: valid_mint(),
            memo: None,
            reference: None,
        };
        assert!(build_pay_url(&req).is_err());
    }

    #[test]
    fn invalid_mint_fails() {
        let req = PayRequest {
            recipient: valid_recipient(),
            amount: 10.0,
            mint: "bad-mint".into(),
            memo: None,
            reference: None,
        };
        assert!(build_pay_url(&req).is_err());
    }

    #[test]
    fn url_encode_spaces() {
        assert_eq!(url_encode("hello world"), "hello%20world");
        assert_eq!(url_encode("a&b=c"), "a%26b%3Dc");
    }
}
