use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Structured command payload produced by AI request assistant.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CliCommand {
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: HashMap<String, Value>,
    #[serde(default)]
    pub query: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
}

#[derive(Debug, Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Debug, Deserialize)]
struct Message {
    content: String,
}

/// Calls an OpenAI-compatible endpoint and returns a normalized CLI command object.
pub async fn ai_assist(prompt: &str, api_key: &str) -> Result<CliCommand> {
    let endpoint = std::env::var("ZAPREQ_AI_ENDPOINT")
        .unwrap_or_else(|_| "https://api.openai.com/v1/chat/completions".to_string());

    let system_prompt = "You are an HTTP request builder. Given a natural language description, output ONLY a JSON object: { method: string, url: string, headers: {key: value}, body: {key: value}, query: {key: value} }. No explanation. JSON only.";

    let payload = ChatRequest {
        model: "gpt-4o-mini",
        messages: vec![
            ChatMessage {
                role: "system",
                content: system_prompt,
            },
            ChatMessage {
                role: "user",
                content: prompt,
            },
        ],
    };

    let client = reqwest::Client::new();
    let response = client
        .post(endpoint)
        .bearer_auth(api_key)
        .json(&payload)
        .send()
        .await
        .context("failed to call AI endpoint")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("AI endpoint returned {status}: {body}"));
    }

    let data: ChatResponse = response
        .json()
        .await
        .context("failed to parse AI response envelope")?;
    let content = data
        .choices
        .first()
        .map(|c| c.message.content.trim())
        .ok_or_else(|| anyhow!("AI response did not include choices"))?;

    let command: CliCommand =
        serde_json::from_str(content).context("failed to parse AI JSON command payload")?;
    Ok(command)
}
