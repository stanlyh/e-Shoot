pub mod whatsapp;

use crate::error::Result;
use crate::messaging::template::RenderedTemplate;
use async_trait::async_trait;

#[async_trait]
pub trait Provider: Send + Sync {
    async fn send(
        &self,
        client: &super::client::HttpMessagingClient,
        recipient: &str,
        template: RenderedTemplate,
    ) -> Result<()>;
}
