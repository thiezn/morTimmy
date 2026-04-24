use std::time::Duration;

use crate::config::NexoConfig;
use anyhow::{Result, anyhow, bail};
use nexo_ws_client::{NexoConnection, default_user_connect_params, perform_handshake};
use nexo_ws_schema::{
    AUTH_TOKEN, AgentEventPayload, AgentParams, AgentResponse, AgentStatus, EventKind, Frame,
    Method, SessionCreateParams, SessionCreateResponse,
};
use tokio::sync::{mpsc, oneshot};
use tokio::time::sleep;

const REQUEST_CHANNEL_CAPACITY: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatReply {
    pub session_id: String,
    pub run_id: String,
    pub content: String,
}

#[derive(Clone, Debug)]
pub struct NexoGateway {
    request_tx: mpsc::Sender<GatewayRequest>,
}

#[derive(Debug)]
enum GatewayRequest {
    Chat {
        prompt: String,
        reply_tx: oneshot::Sender<Result<ChatReply>>,
    },
}

struct GatewayWorker {
    config: NexoConfig,
    reconnect_interval: Duration,
    session_id: Option<String>,
    connection: Option<NexoConnection>,
}

impl NexoGateway {
    pub fn spawn(url: impl Into<String>, reconnect_interval: Duration) -> Self {
        let mut config = NexoConfig::default();
        config.gateway_url = url.into();
        Self::spawn_with_config(config, reconnect_interval)
    }

    pub fn spawn_with_config(config: NexoConfig, reconnect_interval: Duration) -> Self {
        let (request_tx, request_rx) = mpsc::channel(REQUEST_CHANNEL_CAPACITY);
        let worker = GatewayWorker {
            config,
            reconnect_interval,
            session_id: None,
            connection: None,
        };

        tokio::spawn(async move {
            worker.run(request_rx).await;
        });

        Self { request_tx }
    }

    pub async fn chat(&self, prompt: impl Into<String>) -> Result<ChatReply> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.request_tx
            .send(GatewayRequest::Chat {
                prompt: prompt.into(),
                reply_tx,
            })
            .await
            .map_err(|_| anyhow!("nexo worker is not running"))?;

        reply_rx
            .await
            .map_err(|_| anyhow!("nexo worker stopped before returning a reply"))?
    }
}

impl GatewayWorker {
    async fn run(mut self, mut request_rx: mpsc::Receiver<GatewayRequest>) {
        while let Some(request) = request_rx.recv().await {
            match request {
                GatewayRequest::Chat { prompt, reply_tx } => {
                    let result = self.execute_chat_with_retry(prompt).await;
                    let _ = reply_tx.send(result);
                }
            }
        }
    }

    async fn execute_chat_with_retry(&mut self, prompt: String) -> Result<ChatReply> {
        loop {
            if self.connection.is_none() {
                match self.connect().await {
                    Ok(()) => {}
                    Err(error) => {
                        tracing::warn!(
                            gateway = %self.config.gateway_url,
                            "nexo gateway connect failed; retrying in {} ms: {error:#}",
                            self.reconnect_interval.as_millis()
                        );
                        sleep(self.reconnect_interval).await;
                        continue;
                    }
                }
            }

            let result = match self.connection.as_mut() {
                Some(connection) => {
                    Self::send_chat(connection, &mut self.session_id, &prompt).await
                }
                None => continue,
            };

            match result {
                Ok(reply) => return Ok(reply),
                Err(error) => {
                    tracing::warn!(
                        gateway = %self.config.gateway_url,
                        "nexo chat request failed; resetting connection and retrying in {} ms: {error:#}",
                        self.reconnect_interval.as_millis()
                    );
                    self.connection = None;
                    self.session_id = None;
                    sleep(self.reconnect_interval).await;
                }
            }
        }
    }

    async fn connect(&mut self) -> Result<()> {
        let mut connection =
            NexoConnection::connect(&self.config.gateway_url, &auth_token_from_env()).await?;
        let params = default_user_connect_params(
            &self.config.client_id,
            &self.config.client_version,
            self.config.platform,
            &self.config.device_id,
        );
        let hello = perform_handshake(&mut connection, params).await?;
        tracing::info!(
            gateway = %self.config.gateway_url,
            protocol = hello.protocol,
            tick_ms = hello.policy.tick_interval_ms,
            "nexo gateway connected"
        );
        self.connection = Some(connection);
        Ok(())
    }

    async fn send_chat(
        connection: &mut NexoConnection,
        session_id: &mut Option<String>,
        prompt: &str,
    ) -> Result<ChatReply> {
        let session_id = match session_id.clone() {
            Some(session_id) => session_id,
            None => {
                let created = Self::request_response::<SessionCreateResponse, _>(
                    connection,
                    Method::SessionCreate,
                    SessionCreateParams::default(),
                )
                .await?;
                *session_id = Some(created.session_id.clone());
                created.session_id
            }
        };

        let agent_response = Self::request_response::<AgentResponse, _>(
            connection,
            Method::Agent,
            AgentParams {
                prompt: prompt.to_string(),
                idempotency_key: Frame::new_id(),
                session_id: Some(session_id.clone()),
                context: None,
                model_id: None,
                thinking: None,
            },
        )
        .await?;

        Self::collect_chat_reply(connection, session_id, agent_response.run_id).await
    }

    async fn collect_chat_reply(
        connection: &mut NexoConnection,
        session_id: String,
        run_id: String,
    ) -> Result<ChatReply> {
        let mut latest_content = String::new();

        loop {
            let frame = connection.recv_frame().await?.ok_or_else(|| {
                anyhow!("nexo gateway closed the websocket while waiting for a chat reply")
            })?;

            match frame {
                Frame::Event {
                    event: EventKind::Agent,
                    payload,
                    ..
                } => {
                    let event: AgentEventPayload = serde_json::from_value(payload)?;
                    if event.run_id != run_id {
                        continue;
                    }

                    if let Some(content) = event.content {
                        latest_content = content;
                    }

                    match event.status {
                        AgentStatus::Accepted
                        | AgentStatus::Queued
                        | AgentStatus::Thinking
                        | AgentStatus::ToolCall
                        | AgentStatus::Streaming => {}
                        AgentStatus::Completed => {
                            return Ok(ChatReply {
                                session_id,
                                run_id,
                                content: latest_content,
                            });
                        }
                        AgentStatus::Failed | AgentStatus::Cancelled => {
                            let error = event.error.unwrap_or_else(|| {
                                format!("chat run ended with status {:?}", event.status)
                            });
                            bail!(error);
                        }
                    }
                }
                Frame::Event {
                    event: EventKind::Tick,
                    ..
                }
                | Frame::Event {
                    event: EventKind::Heartbeat,
                    ..
                }
                | Frame::Event {
                    event: EventKind::Presence,
                    ..
                } => {}
                Frame::Response {
                    ok: false,
                    error: Some(error),
                    ..
                } => {
                    bail!("{}: {}", error.code, error.message);
                }
                _ => {}
            }
        }
    }

    async fn request_response<T, P>(
        connection: &mut NexoConnection,
        method: Method,
        params: P,
    ) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
        P: serde::Serialize,
    {
        let frame = Frame::request(method, params)?;
        let request_id = match &frame {
            Frame::Request { id, .. } => id.clone(),
            _ => unreachable!("Frame::request always returns a request frame"),
        };

        connection.send_frame(&frame).await?;

        loop {
            let response = connection.recv_frame().await?.ok_or_else(|| {
                anyhow!("nexo gateway closed the websocket while waiting for a response")
            })?;

            match response {
                Frame::Response {
                    id,
                    ok: true,
                    payload: Some(payload),
                    ..
                } if id == request_id => return Ok(serde_json::from_value(payload)?),
                Frame::Response {
                    id,
                    ok: false,
                    error: Some(error),
                    ..
                } if id == request_id => {
                    bail!("{}: {}", error.code, error.message);
                }
                Frame::Event {
                    event: EventKind::Tick,
                    ..
                }
                | Frame::Event {
                    event: EventKind::Heartbeat,
                    ..
                }
                | Frame::Event {
                    event: EventKind::Presence,
                    ..
                } => {}
                _ => {}
            }
        }
    }
}

fn auth_token_from_env() -> String {
    std::env::var("MORTIMMY_NEXO_AUTH_TOKEN").unwrap_or_else(|_| AUTH_TOKEN.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    pub const DEFAULT_GATEWAY_URL: &str = "ws://127.0.0.1:6969";

    #[tokio::test]
    #[ignore = "requires a local nexo-gateway on ws://127.0.0.1:6969"]
    async fn live_chat_roundtrip_returns_content() {
        let gateway = NexoGateway::spawn(DEFAULT_GATEWAY_URL, Duration::from_millis(250));
        let reply = tokio::time::timeout(
            Duration::from_secs(90),
            gateway.chat("Reply with a short greeting for mortimmy."),
        )
        .await
        .expect("chat request timed out")
        .expect("chat request failed");

        assert!(!reply.session_id.is_empty());
        assert!(!reply.run_id.is_empty());
        assert!(!reply.content.trim().is_empty());
    }
}
