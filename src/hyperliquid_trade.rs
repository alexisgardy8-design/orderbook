// 🚀 Hyperliquid Trading Module with Proper ECDSA Signing
// Implements Ethereum-style ECDSA signing for Hyperliquid order placement

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;

#[cfg(feature = "websocket")]
use secp256k1::{SecretKey, PublicKey, Message, Secp256k1};
#[cfg(feature = "websocket")]
use sha3::{Digest, Keccak256};
#[cfg(feature = "websocket")]
use rmp_serde::Serializer;

// --- Wire Formats for Hyperliquid API ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderWire {
    pub a: u32,      // asset index
    pub b: bool,     // is_buy
    pub p: String,   // price
    pub s: String,   // size
    pub r: bool,     // reduce_only
    pub t: OrderTypeWire,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderTypeWire {
    pub limit: LimitOrderType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitOrderType {
    pub tif: String, // "Gtc"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Action {
    Order {
        orders: Vec<OrderWire>,
        grouping: String, // "na"
    },
    Cancel {
        cancels: Vec<CancelRequest>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelRequest {
    pub a: u32, // asset index
    pub o: u64, // order id
}

// Internal structs for signing to ensure field order
#[derive(Serialize)]
struct ActionWire {
    #[serde(rename = "type")]
    type_: String,
    orders: Vec<OrderWire>,
    grouping: String,
}

#[derive(Serialize)]
struct CancelActionWire {
    #[serde(rename = "type")]
    type_: String,
    cancels: Vec<CancelRequest>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Signature {
    pub r: String,
    pub s: String,
    pub v: u8,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExchangeRequest {
    pub action: Action,
    pub nonce: u64,
    pub signature: Signature,
    #[serde(rename = "vaultAddress", skip_serializing_if = "Option::is_none")]
    pub vault_address: Option<String>,
}

/// Response from Hyperliquid Exchange API
#[derive(Debug, Deserialize)]
pub struct ExchangeResponse {
    pub status: String,
    pub response: Option<ExchangeResponseData>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum ExchangeResponseData {
    Order {
        statuses: Vec<serde_json::Value>,
    },
    Cancel {
        statuses: Vec<serde_json::Value>,
    }
}

/// Hyperliquid Trading Client with ECDSA signing
pub struct HyperliquidTrader {
    private_key_hex: String,
    #[cfg(feature = "websocket")]
    secret_key: SecretKey,
    #[cfg(feature = "websocket")]
    pub wallet_address: String,
}

impl HyperliquidTrader {
    /// Initialize trader with private key from .env
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        dotenv::dotenv().ok();

        let private_key_hex = env::var("private_key")
            .or_else(|_| env::var("HYPERLIQUID_PRIVATE_KEY"))
            .map_err(|_| "❌ Private key not found in .env or HYPERLIQUID_PRIVATE_KEY env var")?;

        #[cfg(feature = "websocket")]
        {
            // Parse the private key (remove 0x prefix if present)
            let key_bytes = if private_key_hex.starts_with("0x") {
                hex::decode(&private_key_hex[2..])
                    .map_err(|e| format!("❌ Failed to decode hex: {}", e))?
            } else {
                hex::decode(&private_key_hex)
                    .map_err(|e| format!("❌ Failed to decode hex: {}", e))?
            };

            // Create secret key from bytes
            let secret_key = SecretKey::from_slice(&key_bytes)
                .map_err(|e| format!("❌ Failed to create secret key: {:?}", e))?;

            // Derive address
            let secp = Secp256k1::new();
            let public_key = PublicKey::from_secret_key(&secp, &secret_key);
            let serialized = public_key.serialize_uncompressed();
            let hash = Keccak256::digest(&serialized[1..]);
            let mut address_bytes = [0u8; 20];
            address_bytes.copy_from_slice(&hash[12..]);
            let wallet_address = format!("0x{}", hex::encode(address_bytes));

            println!("✅ Loaded private key for address: {}", wallet_address);

            Ok(Self {
                private_key_hex: private_key_hex.clone(),
                secret_key,
                wallet_address,
            })
        }

        #[cfg(not(feature = "websocket"))]
        {
            Err("❌ WebSocket feature required for signing".into())
        }
    }

    #[cfg(feature = "websocket")]
    pub fn get_wallet_address(&self) -> &str {
        &self.wallet_address
    }

    /// Fetch asset index from Hyperliquid API
    pub async fn fetch_asset_index(&self, coin: &str) -> Result<u32, Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        let response = client
            .post("https://api.hyperliquid.xyz/info")
            .json(&json!({"type": "meta"}))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        let universe = response["universe"].as_array()
            .ok_or("Failed to parse universe")?;

        for (i, asset) in universe.iter().enumerate() {
            if asset["name"].as_str() == Some(coin) {
                return Ok(i as u32);
            }
        }
        Err(format!("Asset {} not found", coin).into())
    }

    /// Sign an action using MsgPack + Keccak256 + ECDSA
    #[cfg(feature = "websocket")]
    fn sign_action(&self, action: Action, nonce: u64) -> Result<Signature, Box<dyn std::error::Error>> {
        // Serialize to MsgPack with strict order
        let mut action_buf = Vec::new();
        let mut serializer = Serializer::new(&mut action_buf).with_struct_map();

        match action {
            Action::Order { orders, grouping } => {
                let wire = ActionWire {
                    type_: "order".to_string(),
                    orders,
                    grouping,
                };
                wire.serialize(&mut serializer)?;
            },
            Action::Cancel { cancels } => {
                let wire = CancelActionWire {
                    type_: "cancel".to_string(),
                    cancels,
                };
                wire.serialize(&mut serializer)?;
            }
        }

        // Construct the payload for hashing:
        // msgpack(action) + nonce (8 bytes BE) + vault_address (1 byte 0x00 if None)
        let mut buf = action_buf.clone();
        buf.extend_from_slice(&nonce.to_be_bytes());
        buf.push(0x00); // vault_address is None

        // Hash with Keccak256 (Action Hash)
        // println!("DEBUG: MsgPack Hex: {}", hex::encode(&action_buf));
        let mut hasher = Keccak256::new();
        hasher.update(&buf);
        let action_hash = hasher.finalize();
        
        // Construct Phantom Agent (EIP-712)
        // Domain Separator
        // EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)
        let domain_type_hash = Keccak256::digest(b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)");
        let name_hash = Keccak256::digest(b"Exchange");
        let version_hash = Keccak256::digest(b"1");
        let chain_id = 1337u64; // Always 1337 for Hyperliquid Exchange domain
        let verifying_contract = [0u8; 20]; // 0x00...00

        let mut domain_hasher = Keccak256::new();
        domain_hasher.update(&domain_type_hash);
        domain_hasher.update(&name_hash);
        domain_hasher.update(&version_hash);
        domain_hasher.update(&[0u8; 24]); // Padding for u256
        domain_hasher.update(&chain_id.to_be_bytes());
        domain_hasher.update(&[0u8; 12]); // Padding for address
        domain_hasher.update(&verifying_contract);
        let domain_separator = domain_hasher.finalize();

        // Hash Struct (Agent)
        // Agent(string source,bytes32 connectionId)
        let agent_type_hash = Keccak256::digest(b"Agent(string source,bytes32 connectionId)");
        let source = "a"; // "a" for Mainnet, "b" for Testnet
        // TODO: Make source configurable based on environment
        let source_hash = Keccak256::digest(source.as_bytes());
        
        let mut struct_hasher = Keccak256::new();
        struct_hasher.update(&agent_type_hash);
        struct_hasher.update(&source_hash);
        struct_hasher.update(&action_hash);
        let struct_hash = struct_hasher.finalize();

        // Final EIP-712 Hash
        let mut final_hasher = Keccak256::new();
        final_hasher.update(b"\x19\x01");
        final_hasher.update(&domain_separator);
        final_hasher.update(&struct_hash);
        let message_hash = final_hasher.finalize();

        // Sign with secp256k1
        let secp = Secp256k1::new();
        let msg = Message::from_digest_slice(&message_hash)?;
        
        let recoverable_sig = secp.sign_ecdsa_recoverable(&msg, &self.secret_key);
        let (rec_id, sig_bytes) = recoverable_sig.serialize_compact();
        
        // Construct signature
        let v = rec_id.to_i32() as u8 + 27;
        
        Ok(Signature {
            r: format!("0x{}", hex::encode(&sig_bytes[0..32])),
            s: format!("0x{}", hex::encode(&sig_bytes[32..64])),
            v,
        })
    }

    fn get_nonce() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    fn float_to_wire(x: f64) -> String {
        let s = format!("{:.8}", x);
        let s = s.trim_end_matches('0');
        let s = s.trim_end_matches('.');
        if s.is_empty() { "0".to_string() } else { s.to_string() }
    }

    /// Place a limit order on Hyperliquid Mainnet
    pub async fn place_limit_order(
        &self,
        coin: &str,
        is_buy: bool,
        px: f64,
        sz: f64,
    ) -> Result<u64, Box<dyn std::error::Error>> {
        println!("\n📝 Placing limit order on Hyperliquid...");
        
        // 1. Get asset index
        let asset_index = self.fetch_asset_index(coin).await?;
        println!("   Asset Index for {}: {}", coin, asset_index);

        #[cfg(feature = "websocket")]
        {
            // 2. Create the order wire format
            let order = OrderWire {
                a: asset_index,
                b: is_buy,
                p: Self::float_to_wire(px),
                s: Self::float_to_wire(sz),
                r: false,
                t: OrderTypeWire {
                    limit: LimitOrderType { tif: "Gtc".to_string() }
                }
            };

            let action = Action::Order {
                orders: vec![order],
                grouping: "na".to_string(),
            };

            let nonce = Self::get_nonce();
            
            // 3. Sign the action
            println!("🔐 Signing order...");
            let signature = self.sign_action(action.clone(), nonce)?;

            // 4. Create request
            let request = ExchangeRequest {
                action,
                nonce,
                signature,
                vault_address: None,
            };

            // 5. Send to API
            println!("📤 Sending to https://api.hyperliquid.xyz/exchange");
            let client = reqwest::Client::new();
            let response = client
                .post("https://api.hyperliquid.xyz/exchange")
                .json(&request)
                .send()
                .await?;

            let status = response.status();
            let body = response.text().await?;
            
            println!("📥 Response Status: {}", status);
            // println!("📥 Response Body: {}", body);

            if status.is_success() {
                let resp: ExchangeResponse = serde_json::from_str(&body)?;
                if let Some(ExchangeResponseData::Order { statuses }) = resp.response {
                    // Statuses is a list of objects like {"filled": ...} or {"resting": {"oid": 123}}
                    // We want to extract the OID if resting
                    if let Some(first_status) = statuses.first() {
                        if let Some(resting) = first_status.get("resting") {
                            let oid = resting["oid"].as_u64().ok_or("Missing OID in resting status")?;
                            println!("✅ Order placed! OID: {}", oid);
                            return Ok(oid);
                        }
                    }
                    println!("✅ Order processed but not resting (filled/canceled?): {:?}", statuses);
                    return Ok(0); // Or handle differently
                }
            }
            
            Err(format!("API Error: {}", body).into())
        }

        #[cfg(not(feature = "websocket"))]
        {
            Err("WebSocket feature required".into())
        }
    }

    /// Cancel an order
    pub async fn cancel_order(
        &self,
        coin: &str,
        oid: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n❌ Cancelling order {}...", oid);
        
        let asset_index = self.fetch_asset_index(coin).await?;

        #[cfg(feature = "websocket")]
        {
            let cancel = CancelRequest {
                a: asset_index,
                o: oid,
            };

            let action = Action::Cancel {
                cancels: vec![cancel],
            };

            let nonce = Self::get_nonce();
            let signature = self.sign_action(action.clone(), nonce)?;

            let request = ExchangeRequest {
                action,
                nonce,
                signature,
                vault_address: None,
            };

            // let request_json = serde_json::to_string(&request)?;
            // println!("📤 Cancel Request: {}", request_json);

            let client = reqwest::Client::new();
            let response = client
                .post("https://api.hyperliquid.xyz/exchange")
                .header("Content-Type", "application/json")
                .body(request_json)
                .send()
                .await?;

            let status = response.status();
            let body = response.text().await?;

            // println!("📥 Cancel Response: {}", body);

            if status.is_success() {
                Ok(())
            } else {
                Err(format!("Cancel failed: {}", body).into())
            }
        }
        #[cfg(not(feature = "websocket"))]
        { Err("WebSocket feature required".into()) }
    }
}
