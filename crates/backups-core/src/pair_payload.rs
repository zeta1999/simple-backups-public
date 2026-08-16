//! Mobile / desktop out-of-band pairing payload.
//!
//! Wire form: `simple-backups:v1:pair:<host:port>:<onetimecode>`

use anyhow::{bail, Result};

pub const PAIR_PREFIX: &str = "simple-backups:v1:pair:";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairPayload {
    pub addr: String,
    pub code: String,
}

pub fn format_pair_payload(addr: &str, code: &str) -> String {
    format!("{PAIR_PREFIX}{addr}:{code}")
}

pub fn parse_pair_payload(raw: &str) -> Result<PairPayload> {
    let s = raw.trim();
    if let Some(rest) = s.strip_prefix(PAIR_PREFIX) {
        let idx = rest
            .rfind(':')
            .ok_or_else(|| anyhow::anyhow!("invalid pair payload: missing code"))?;
        let addr = rest[..idx].trim();
        let code = rest[idx + 1..].trim();
        if addr.is_empty() || code.is_empty() {
            bail!("invalid pair payload: empty addr or code");
        }
        return Ok(PairPayload {
            addr: addr.to_string(),
            code: code.to_string(),
        });
    }
    // Bare host:port — code filled later.
    if s.is_empty() {
        bail!("empty pair payload");
    }
    Ok(PairPayload {
        addr: s.to_string(),
        code: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let s = format_pair_payload("192.168.1.10:9876", "deadbeef");
        let p = parse_pair_payload(&s).unwrap();
        assert_eq!(p.addr, "192.168.1.10:9876");
        assert_eq!(p.code, "deadbeef");
    }

    #[test]
    fn bare_addr() {
        let p = parse_pair_payload("127.0.0.1:9").unwrap();
        assert_eq!(p.addr, "127.0.0.1:9");
        assert!(p.code.is_empty());
    }
}
