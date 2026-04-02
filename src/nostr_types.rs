use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NostrEvent {
    pub id: String,
    pub pubkey: String,
    pub created_at: i64,
    pub kind: u64,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
}

impl NostrEvent {
    pub fn verify_id(&self) -> bool {
        let canonical = serde_json::json!([
            0,
            self.pubkey,
            self.created_at,
            self.kind,
            self.tags,
            self.content,
        ]);
        let serialized = canonical.to_string();
        let mut hasher = Sha256::new();
        hasher.update(serialized.as_bytes());
        let hash = hasher.finalize();
        hex::encode(hash) == self.id
    }

    pub fn verify_sig(&self) -> bool {
        let Ok(pubkey_bytes) = hex::decode(&self.pubkey) else {
            return false;
        };
        let Ok(sig_bytes) = hex::decode(&self.sig) else {
            return false;
        };
        let Ok(id_bytes) = hex::decode(&self.id) else {
            return false;
        };
        let Ok(xonly) =
            secp256k1::XOnlyPublicKey::from_slice(&pubkey_bytes)
        else {
            return false;
        };
        let Ok(sig) = secp256k1::schnorr::Signature::from_slice(&sig_bytes)
        else {
            return false;
        };
        let Ok(msg) = secp256k1::Message::from_digest_slice(&id_bytes) else {
            return false;
        };
        secp256k1::global::SECP256K1
            .verify_schnorr(&sig, &msg, &xonly)
            .is_ok()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Filter {
    pub ids: Option<Vec<String>>,
    pub authors: Option<Vec<String>>,
    pub kinds: Option<Vec<u64>>,
    #[serde(rename = "#e")]
    pub e_tags: Option<Vec<String>>,
    #[serde(rename = "#p")]
    pub p_tags: Option<Vec<String>>,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub limit: Option<usize>,
}

impl Filter {
    pub fn matches(&self, event: &NostrEvent) -> bool {
        if let Some(ids) = &self.ids {
            if !ids.iter().any(|id| event.id.starts_with(id.as_str())) {
                return false;
            }
        }
        if let Some(authors) = &self.authors {
            if !authors
                .iter()
                .any(|a| event.pubkey.starts_with(a.as_str()))
            {
                return false;
            }
        }
        if let Some(kinds) = &self.kinds {
            if !kinds.contains(&event.kind) {
                return false;
            }
        }
        if let Some(since) = self.since {
            if event.created_at < since {
                return false;
            }
        }
        if let Some(until) = self.until {
            if event.created_at > until {
                return false;
            }
        }
        if let Some(e_tags) = &self.e_tags {
            let event_e_tags: Vec<&str> = event
                .tags
                .iter()
                .filter(|t| t.first().map(|s| s == "e").unwrap_or(false))
                .filter_map(|t| t.get(1).map(|s| s.as_str()))
                .collect();
            if !e_tags.iter().any(|e| event_e_tags.contains(&e.as_str())) {
                return false;
            }
        }
        if let Some(p_tags) = &self.p_tags {
            let event_p_tags: Vec<&str> = event
                .tags
                .iter()
                .filter(|t| t.first().map(|s| s == "p").unwrap_or(false))
                .filter_map(|t| t.get(1).map(|s| s.as_str()))
                .collect();
            if !p_tags.iter().any(|p| event_p_tags.contains(&p.as_str())) {
                return false;
            }
        }
        true
    }
}

pub fn npub_to_hex(npub: &str) -> Result<String> {
    let (hrp, data, _) = bech32::decode(npub)
        .map_err(|e| anyhow!("bech32 decode error: {e}"))?;
    if hrp != "npub" {
        return Err(anyhow!("Not an npub (got hrp: {hrp})"));
    }
    let bytes = bech32::convert_bits(&data, 5, 8, false)
        .map_err(|e| anyhow!("bit conversion error: {e}"))?;
    Ok(hex::encode(bytes))
}

pub fn hex_to_npub(hex_key: &str) -> Result<String> {
    let bytes = hex::decode(hex_key)
        .map_err(|e| anyhow!("hex decode error: {e}"))?;
    let data = bech32::convert_bits(&bytes, 8, 5, true)
        .map_err(|e| anyhow!("bit conversion error: {e}"))?;
    bech32::encode("npub", data, bech32::Variant::Bech32)
        .map_err(|e| anyhow!("bech32 encode error: {e}"))
}
