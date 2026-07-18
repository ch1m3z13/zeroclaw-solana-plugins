use crate::types::{AccountData, Blockhash, TokenAccount, TokenLargestAccount};
use serde::{Deserialize, Serialize};

/// Abstraction over the blocking HTTP transport.
///
/// The wasm build uses `waki` (see `WakiTransport`). Host builds and tests
/// inject a `MockTransport` so the core can be exercised with no live network
/// and no wasm toolchain (plan global constraint: "host-run tests, mock RPC").
pub trait Transport {
    fn post(&self, url: &str, api_key: Option<&str>, body: &[u8]) -> Result<Vec<u8>, String>;
}

/// JSON-RPC 2.0 client over a pluggable transport.
pub struct RpcClient {
    url: String,
    api_key: Option<String>,
    transport: Box<dyn Transport>,
}

#[derive(Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: u64,
    method: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct JsonRpcResponse {
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize, Debug)]
struct JsonRpcError {
    code: i64,
    message: String,
}

/// In-memory transport for host tests (mock RPC).
pub struct MockTransport {
    pub handler: Box<dyn Fn(&str, Option<&str>, &[u8]) -> Result<Vec<u8>, String>>,
}

impl Transport for MockTransport {
    fn post(&self, url: &str, api_key: Option<&str>, body: &[u8]) -> Result<Vec<u8>, String> {
        (self.handler)(url, api_key, body)
    }
}

/// Host-only default transport. Errors if a request is actually issued —
/// host code must inject `MockTransport` (or a real one) before calling RPC.
#[allow(dead_code)]
struct NullTransport;

impl Transport for NullTransport {
    fn post(&self, _url: &str, _api_key: Option<&str>, _body: &[u8]) -> Result<Vec<u8>, String> {
        Err("RpcClient::new on host has no transport; inject one via with_transport".to_string())
    }
}

#[cfg(target_family = "wasm")]
struct WakiTransport;

#[cfg(target_family = "wasm")]
impl Transport for WakiTransport {
    fn post(&self, url: &str, api_key: Option<&str>, body: &[u8]) -> Result<Vec<u8>, String> {
        let mut headers = vec![("Content-Type", "application/json")];
        let auth;
        if let Some(key) = api_key {
            auth = format!("Bearer {key}");
            headers.push(("Authorization", auth.as_str()));
        }
        let response = waki::Client::new()
            .post(url)
            .body(body.to_vec())
            .headers(headers.iter().map(|(k, v)| (*k, *v)))
            .send()
            .map_err(|e| format!("http: {e}"))?;
        let status = response.status_code();
        let bytes = response.body().map_err(|e| format!("read: {e}"))?;
        if status != 200 {
            return Err(format!("rpc {status}: {}", String::from_utf8_lossy(&bytes)));
        }
        Ok(bytes)
    }
}

impl RpcClient {
    /// Construct for the wasm runtime (uses `waki` over wasi:http).
    #[cfg(target_family = "wasm")]
    pub fn new(url: String, api_key: Option<String>) -> Self {
        Self {
            url,
            api_key,
            transport: Box::new(WakiTransport),
        }
    }

    /// Construct for the host. The host path must inject a real transport
    /// (e.g. `MockTransport`) before issuing requests; calling a method on a
    /// `new`-built client will error with a clear message.
    #[cfg(not(target_family = "wasm"))]
    pub fn new(_url: String, _api_key: Option<String>) -> Self {
        Self {
            url: _url,
            api_key: _api_key,
            transport: Box::new(NullTransport),
        }
    }

    /// Construct with an explicit transport — used by host tests with `MockTransport`.
    pub fn with_transport(
        url: String,
        api_key: Option<String>,
        transport: Box<dyn Transport>,
    ) -> Self {
        Self {
            url,
            api_key,
            transport,
        }
    }

    fn post_json(
        &self,
        method: &'static str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method,
            params,
        };
        let body = serde_json::to_vec(&request).map_err(|e| format!("serialize: {e}"))?;
        let bytes = self
            .transport
            .post(&self.url, self.api_key.as_deref(), &body)?;
        let text = String::from_utf8(bytes).map_err(|e| format!("utf8: {e}"))?;
        let resp: JsonRpcResponse =
            serde_json::from_str(&text).map_err(|e| format!("parse: {e}"))?;
        if let Some(err) = resp.error {
            return Err(format!("rpc error {}: {}", err.code, err.message));
        }
        resp.result.ok_or_else(|| "rpc: null result".to_string())
    }

    pub fn get_account(&self, pubkey: &str) -> Result<Option<AccountData>, String> {
        let params = serde_json::json!([pubkey, { "encoding": "base64" }]);
        let result = self.post_json("getAccountInfo", Some(params))?;
        let value = result.get("value").and_then(|v| v.as_object());
        match value {
            Some(obj) if !obj.is_empty() => {
                let data_b64 = obj
                    .get("data")
                    .and_then(|d| d.as_array())
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let data = crate::encoding::decode_base64(data_b64)?;
                let owner = obj
                    .get("owner")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let lamports = obj.get("lamports").and_then(|v| v.as_u64()).unwrap_or(0);
                Ok(Some(AccountData {
                    data,
                    owner,
                    lamports,
                }))
            }
            _ => Ok(None),
        }
    }

    pub fn get_latest_blockhash(&self, commitment: &str) -> Result<Blockhash, String> {
        let params = serde_json::json!([{ "commitment": commitment }]);
        let result = self.post_json("getLatestBlockhash", Some(params))?;
        let hash = result
            .get("value")
            .and_then(|v| v.get("blockhash"))
            .and_then(|v| v.as_str())
            .ok_or("missing blockhash")?
            .to_string();
        Ok(Blockhash(hash))
    }

    pub fn get_token_accounts_by_owner(
        &self,
        owner: &str,
        mint: Option<&str>,
    ) -> Result<Vec<TokenAccount>, String> {
        let mint_filter = if let Some(m) = mint {
            serde_json::json!({ "mint": m })
        } else {
            serde_json::json!({ "programId": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA" })
        };
        let params = serde_json::json!([
            owner,
            mint_filter,
            { "encoding": "jsonParsed" }
        ]);
        let result = self.post_json("getTokenAccountsByOwner", Some(params))?;
        let accounts = result
            .get("value")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        let info = item
                            .get("account")?
                            .get("data")?
                            .get("parsed")?
                            .get("info")?;
                        Some(TokenAccount {
                            mint: info.get("mint")?.as_str()?.to_string(),
                            owner: info.get("owner")?.as_str()?.to_string(),
                            amount: info
                                .get("tokenAmount")?
                                .get("uiAmount")?
                                .as_f64()
                                .unwrap_or(0.0) as u64,
                            delegate: info
                                .get("delegate")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            state: info.get("state")?.as_str()?.to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(accounts)
    }

    pub fn get_token_largest_accounts(
        &self,
        mint: &str,
    ) -> Result<Vec<TokenLargestAccount>, String> {
        let params = serde_json::json!([mint]);
        let result = self.post_json("getTokenLargestAccounts", Some(params))?;
        let accounts = result
            .get("value")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        Some(TokenLargestAccount {
                            address: item.get("address")?.as_str()?.to_string(),
                            amount: item.get("amount")?.as_str()?.parse().ok()?,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(accounts)
    }

    pub fn send_transaction(&self, tx_bytes: &[u8]) -> Result<String, String> {
        let encoded = crate::encoding::encode_base64(tx_bytes);
        let params = serde_json::json!([encoded, { "encoding": "base64" }]);
        let result = self.post_json("sendTransaction", Some(params))?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "missing signature".to_string())
    }

    /// Get recent transaction signatures for an address (getSignaturesForAddress).
    pub fn get_recent_signatures(
        &self,
        address: &str,
        limit: usize,
    ) -> Result<Vec<serde_json::Value>, String> {
        let params = serde_json::json!([address, { "limit": limit }]);
        let result = self.post_json("getSignaturesForAddress", Some(params))?;
        result
            .as_array()
            .cloned()
            .ok_or_else(|| "expected array of signatures".to_string())
    }

    /// Get a confirmed transaction by signature (getTransaction, jsonParsed).
    pub fn get_transaction(&self, signature: &str) -> Result<Option<serde_json::Value>, String> {
        let params = serde_json::json!([signature, { "encoding": "jsonParsed" }]);
        let result = self.post_json("getTransaction", Some(params))?;
        if result.is_null() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::{decode_base64, encode_base64};

    fn mock_for(method: &str, value: serde_json::Value) -> RpcClient {
        let method = method.to_string();
        let resp = serde_json::json!({ "jsonrpc": "2.0", "id": 1, "result": value });
        let handler = move |_url: &str, _key: Option<&str>, req: &[u8]| {
            let parsed: serde_json::Value = serde_json::from_slice(req).unwrap();
            assert_eq!(parsed["method"], method);
            Ok(serde_json::to_vec(&resp).unwrap())
        };
        RpcClient::with_transport(
            "http://mock".into(),
            None,
            Box::new(MockTransport {
                handler: Box::new(handler),
            }),
        )
    }

    #[test]
    fn get_account_parses_base64_data() {
        let account_data = encode_base64(b"hello-solana");
        let value = serde_json::json!({
            "value": {
                "data": [account_data, "base64"],
                "owner": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
                "lamports": 2039280
            }
        });
        let client = mock_for("getAccountInfo", value);
        let acct = client.get_account("Mint1111").unwrap().unwrap();
        assert_eq!(acct.data, b"hello-solana");
        assert_eq!(acct.owner, "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
        assert_eq!(acct.lamports, 2039280);
    }

    #[test]
    fn get_token_largest_accounts_parses() {
        let value = serde_json::json!({
            "value": [
                { "address": "Holder1111", "amount": "500000", "decimals": 6, "uiAmount": 0.5 },
                { "address": "Holder2222", "amount": "300000", "decimals": 6, "uiAmount": 0.3 }
            ]
        });
        let client = mock_for("getTokenLargestAccounts", value);
        let largest = client.get_token_largest_accounts("Mint1111").unwrap();
        assert_eq!(largest.len(), 2);
        assert_eq!(largest[0].address, "Holder1111");
        assert_eq!(largest[0].amount, 500000);
    }

    #[test]
    fn base64_round_trips() {
        let data = b"any byte sequence \x00\x01\x02\xff";
        let enc = encode_base64(data);
        let dec = decode_base64(&enc).unwrap();
        assert_eq!(dec, data);
    }
}
