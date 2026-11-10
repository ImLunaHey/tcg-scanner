//! Store scan staging: typed imports, revision-checked updates and paired photos.
//! Approval and inventory publication are deliberately outside this API.
mod client;
pub mod contracts;
pub mod ownership;

pub use client::{Client, review_url};
use contracts::{CardInput, CardReply, Import, ImportReply};
use scanner_types::error::Result;

/// An injected staging destination. Implementations must not follow redirects.
#[async_trait::async_trait]
pub trait Staging: Send + Sync {
    fn origin(&self) -> &str;
    fn scanner_instance_id(&self) -> &str;
    async fn import(&self, value: &Import) -> Result<ImportReply>;
    async fn update(&self, card: &CardReply, input: &CardInput) -> Result<CardReply>;
    async fn upload(&self, card: &CardReply, front: Vec<u8>, back: Vec<u8>) -> Result<CardReply>;
}
