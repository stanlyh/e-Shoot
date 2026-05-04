use super::Provider;
use crate::error::Result;
use crate::messaging::client::HttpMessagingClient;
use crate::messaging::template::RenderedTemplate;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct WhatsAppProvider {
    pub phone_number_id: String,
}

#[async_trait]
impl Provider for WhatsAppProvider {
    async fn send(
        &self,
        client: &HttpMessagingClient,
        recipient: &str,
        template: RenderedTemplate,
    ) -> Result<()> {
        let payload = build_payload(recipient, &template);
        let path = format!("/{}/messages", self.phone_number_id);
        client.post_json(&path, &payload).await?;
        Ok(())
    }
}

fn build_payload(recipient: &str, template: &RenderedTemplate) -> Value {
    let components: Vec<Value> = template
        .components
        .iter()
        .filter(|c| !c.parameters.is_empty())
        .map(|c| {
            let params: Vec<Value> = c
                .parameters
                .iter()
                .map(|p| {
                    if p.kind == "text" {
                        json!({ "type": "text", "text": p.value })
                    } else {
                        json!({ "type": p.kind, "value": p.value })
                    }
                })
                .collect();
            json!({ "type": c.kind, "parameters": params })
        })
        .collect();

    json!({
        "messaging_product": "whatsapp",
        "to": recipient,
        "type": "template",
        "template": {
            "name": template.name,
            "language": { "code": template.language },
            "components": components
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messaging::template::{RenderedComponent, RenderedParameter, RenderedTemplate};

    #[test]
    fn builds_correct_whatsapp_payload() {
        let template = RenderedTemplate {
            name: "promo".to_string(),
            language: "es".to_string(),
            components: vec![RenderedComponent {
                kind: "body".to_string(),
                parameters: vec![RenderedParameter {
                    kind: "text".to_string(),
                    value: "Eduardo".to_string(),
                }],
            }],
        };
        let payload = build_payload("+573001234567", &template);
        assert_eq!(payload["messaging_product"], "whatsapp");
        assert_eq!(payload["to"], "+573001234567");
        assert_eq!(payload["template"]["name"], "promo");
        assert_eq!(
            payload["template"]["components"][0]["parameters"][0]["text"],
            "Eduardo"
        );
    }
}
