use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;

#[derive(Clone)]
pub(crate) enum Principal {
    Root,
    Session([u8; 32]),
    PublicImage,
}
pub(crate) struct Access {
    root: [u8; 32],
    sessions: HashMap<[u8; 32], Instant>,
    window: Instant,
    requests: u32,
    attempts: u32,
}
impl Access {
    pub fn new(token: &str) -> scanner_types::error::Result<Self> {
        if !valid_token(token) {
            return Err(scanner_types::error::ScannerError::new(
                "configuration",
                "Invalid gateway token.",
            ));
        }
        Ok(Self {
            root: digest(token),
            sessions: HashMap::new(),
            window: Instant::now(),
            requests: 0,
            attempts: 0,
        })
    }
    pub fn authenticate(&mut self, header: Option<&str>) -> Result<Principal> {
        let now = Instant::now();
        if now.duration_since(self.window) >= Duration::from_secs(60) {
            self.window = now;
            self.requests = 0;
            self.attempts = 0;
        }
        self.sessions.retain(|_, expiry| *expiry > now);
        let credential = header
            .and_then(|h| h.strip_prefix("Bearer "))
            .filter(|s| valid_token(s));
        let principal = credential.and_then(|token| {
            let hash = digest(token);
            if bool::from(hash.ct_eq(&self.root)) {
                Some(Principal::Root)
            } else if self.sessions.contains_key(&hash) {
                Some(Principal::Session(hash))
            } else {
                None
            }
        });
        if let Some(principal) = principal {
            if self.requests >= 600 {
                return Err(Error::new(
                    429,
                    "rateLimit",
                    "Request limit reached; retry later.",
                ));
            }
            self.requests += 1;
            Ok(principal)
        } else {
            if self.attempts >= 30 {
                return Err(Error::new(
                    429,
                    "rateLimit",
                    "Authentication attempt limit reached; retry later.",
                ));
            }
            self.attempts += 1;
            Err(Error::new(
                401,
                "unauthorized",
                "Invalid or missing Bearer credential.",
            ))
        }
    }
    pub fn valid(&self, principal: &Principal) -> bool {
        match principal {
            Principal::Root | Principal::PublicImage => true,
            Principal::Session(hash) => {
                self.sessions.get(hash).is_some_and(|e| *e > Instant::now())
            }
        }
    }
    pub fn issue(&mut self, principal: &Principal) -> Result<(String, DateTime<Utc>)> {
        if !matches!(principal, Principal::Root) {
            return Err(Error::new(
                403,
                "accessDenied",
                "Only the root credential can create sessions.",
            ));
        }
        self.sessions.retain(|_, expiry| *expiry > Instant::now());
        if self.sessions.len() >= 64 {
            return Err(Error::busy());
        }
        let token = generate_token().map_err(Error::from)?;
        self.sessions
            .insert(digest(&token), Instant::now() + Duration::from_secs(3600));
        Ok((token, Utc::now() + chrono::Duration::hours(1)))
    }
    pub fn revoke(&mut self, principal: &Principal) -> Result<()> {
        let Principal::Session(hash) = principal else {
            return Err(Error::params(
                "Root rotation requires replacing its credential and restarting the service.",
            ));
        };
        self.sessions.remove(hash);
        Ok(())
    }
}
pub fn generate_token() -> scanner_types::error::Result<String> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes).map_err(|_| {
        scanner_types::error::ScannerError::new("configuration", "Secure token generation failed.")
    })?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn valid_token(token: &str) -> bool {
    token.len() == 64
        && token
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn digest(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_authority_revocation_expiry_and_authentication_budget() {
        let token = generate_token().unwrap();
        let mut access = Access::new(&token).unwrap();
        let root = access
            .authenticate(Some(&format!("Bearer {token}")))
            .unwrap();
        let (session, _) = access.issue(&root).unwrap();
        let principal = access
            .authenticate(Some(&format!("Bearer {session}")))
            .unwrap();
        assert!(access.issue(&principal).is_err());
        access.revoke(&principal).unwrap();
        assert!(!access.valid(&principal));
        let (session, _) = access.issue(&root).unwrap();
        access
            .sessions
            .insert(digest(&session), Instant::now() - Duration::from_secs(1));
        assert!(
            access
                .authenticate(Some(&format!("Bearer {session}")))
                .is_err()
        );
        for _ in 1..30 {
            assert_eq!(access.authenticate(None).err().unwrap().status, 401);
        }
        assert_eq!(access.authenticate(None).err().unwrap().status, 429);
        assert!(
            access
                .authenticate(Some(&format!("Bearer {token}")))
                .is_ok()
        );
    }
}
