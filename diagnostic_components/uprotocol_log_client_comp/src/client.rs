// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0
/* Portions of this file were generated with AI assistance. */

//! Thin uProtocol RPC client for the ECU `getLogs` method.
//!
//! Wraps `up_rust`'s Communication Layer (`InMemoryRpcClient`) so the rest of
//! the component only deals in [`LogQuery`]/[`LogResponse`], never in raw
//! `UMessage`s. The underlying `UTransport` is injected, so the same client
//! works over POSIX/Zenoh/SOME/IP in a deployment and over the in-process
//! transport in tests.

use std::sync::Arc;

use up_rust::communication::{CallOptions, InMemoryRpcClient, RpcClient, UPayload};
use up_rust::{LocalUriProvider, StaticUriProvider, UPayloadFormat, UPriority, UTransport, UUri};

use crate::wire::{LogQuery, LogResponse, WireLogConfig};

/// Addressing and timing for the ECU log service.
///
/// Defaults match `DummyLogProviderService.uproto.layer.yaml`
/// (service_id `0x8010`, version 1, method `getLogs` = `0x0001`, authority
/// `vehicle`, 5000 ms timeout).
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    /// uProtocol authority hosting the ECU service.
    pub authority: String,
    /// uEntity id of the ECU log service.
    pub service_id: u32,
    /// Major version of the service.
    pub version_major: u8,
    /// Resource/method id of `getLogs`.
    pub get_logs_method_id: u16,
    /// Resource/method id of `getConfig`.
    pub get_config_method_id: u16,
    /// Resource/method id of `configure` (replace capture config).
    pub configure_method_id: u16,
    /// Resource/method id of `resetConfig` (restore default capture config).
    pub reset_config_method_id: u16,
    /// RPC time-to-live in milliseconds.
    pub timeout_ms: u32,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            authority: "vehicle".to_string(),
            service_id: 0x8010,
            version_major: 1,
            get_logs_method_id: 0x0001,
            get_config_method_id: 0x0002,
            configure_method_id: 0x0003,
            reset_config_method_id: 0x0004,
            timeout_ms: 5000,
        }
    }
}

/// Errors raised while performing the `getLogs` RPC.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("failed to build RPC client: {0}")]
    Setup(String),
    #[error("failed to encode request: {0}")]
    Encode(#[from] serde_json::Error),
    #[error("RPC invocation failed: {0}")]
    Invoke(String),
    #[error("service returned an empty response")]
    EmptyResponse,
}

/// uProtocol client for the ECU log service.
pub struct LogServiceClient {
    rpc: InMemoryRpcClient,
    get_logs_method: UUri,
    get_config_method: UUri,
    configure_method: UUri,
    reset_config_method: UUri,
    config: ServiceConfig,
}

impl LogServiceClient {
    /// Builds a client over the given transport.
    ///
    /// `local` identifies this SOVD-side uEntity (used as the RPC response
    /// sink); `config` addresses the remote ECU service.
    ///
    /// # Errors
    /// Returns [`ClientError::Setup`] if the response listener cannot be
    /// registered with the transport.
    pub async fn new(
        transport: Arc<dyn UTransport>,
        local: &StaticUriProvider,
        config: ServiceConfig,
    ) -> Result<Self, ClientError> {
        let uri_provider: Arc<dyn LocalUriProvider> = Arc::new(StaticUriProvider::new(
            local.get_authority(),
            // get_source_uri() carries this uEntity's id/version.
            local.get_source_uri().ue_id,
            u8::try_from(local.get_source_uri().ue_version_major).unwrap_or(1),
        ));

        let rpc = InMemoryRpcClient::new(transport, uri_provider)
            .await
            .map_err(|e| ClientError::Setup(e.to_string()))?;

        let method_uri = |resource_id: u16| UUri {
            authority_name: config.authority.clone(),
            ue_id: config.service_id,
            ue_version_major: u32::from(config.version_major),
            resource_id: u32::from(resource_id),
            ..Default::default()
        };

        let get_logs_method = method_uri(config.get_logs_method_id);
        let get_config_method = method_uri(config.get_config_method_id);
        let configure_method = method_uri(config.configure_method_id);
        let reset_config_method = method_uri(config.reset_config_method_id);

        Ok(Self {
            rpc,
            get_logs_method,
            get_config_method,
            configure_method,
            reset_config_method,
            config,
        })
    }

    /// Calls the ECU `getLogs` method and returns the parsed response.
    ///
    /// # Errors
    /// Returns a [`ClientError`] if encoding, invocation, or (empty) response
    /// handling fails.
    pub async fn get_logs(&self, query: &LogQuery) -> Result<LogResponse, ClientError> {
        let body = serde_json::to_vec(query)?;
        let payload = UPayload::new(body, UPayloadFormat::UPAYLOAD_FORMAT_JSON);

        let options = CallOptions::for_rpc_request(
            self.config.timeout_ms,
            None,
            None,
            Some(UPriority::UPRIORITY_CS4),
        );

        let response = self
            .rpc
            .invoke_method(self.get_logs_method.clone(), options, Some(payload))
            .await
            .map_err(|e| ClientError::Invoke(e.to_string()))?
            .ok_or(ClientError::EmptyResponse)?;

        let bytes = response.payload();
        LogResponse::parse(&bytes).map_err(ClientError::Encode)
    }

    /// Calls the ECU `getConfig` method and returns the capture configuration.
    ///
    /// The request carries no payload — `getConfig` takes no arguments. The
    /// response is a JSON array of [`WireLogConfig`] rules.
    ///
    /// # Errors
    /// Returns a [`ClientError`] if invocation, the (empty) response, or
    /// decoding fails.
    pub async fn get_config(&self) -> Result<Vec<WireLogConfig>, ClientError> {
        let options = CallOptions::for_rpc_request(
            self.config.timeout_ms,
            None,
            None,
            Some(UPriority::UPRIORITY_CS4),
        );

        let response = self
            .rpc
            .invoke_method(self.get_config_method.clone(), options, None)
            .await
            .map_err(|e| ClientError::Invoke(e.to_string()))?
            .ok_or(ClientError::EmptyResponse)?;

        let bytes = response.payload();
        serde_json::from_slice::<Vec<WireLogConfig>>(&bytes).map_err(ClientError::Encode)
    }

    /// Calls the ECU `configure` method, replacing the capture configuration
    /// with `config`. The request body is a JSON array of [`WireLogConfig`].
    ///
    /// # Errors
    /// Returns a [`ClientError`] if encoding or invocation fails.
    pub async fn set_config(&self, config: &[WireLogConfig]) -> Result<(), ClientError> {
        let body = serde_json::to_vec(config)?;
        let payload = UPayload::new(body, UPayloadFormat::UPAYLOAD_FORMAT_JSON);

        let options = CallOptions::for_rpc_request(
            self.config.timeout_ms,
            None,
            None,
            Some(UPriority::UPRIORITY_CS4),
        );

        self.rpc
            .invoke_method(self.configure_method.clone(), options, Some(payload))
            .await
            .map_err(|e| ClientError::Invoke(e.to_string()))?;
        Ok(())
    }

    /// Calls the ECU `resetConfig` method, restoring the default capture
    /// configuration. The request carries no payload.
    ///
    /// # Errors
    /// Returns a [`ClientError`] if invocation fails.
    pub async fn reset_config(&self) -> Result<(), ClientError> {
        let options = CallOptions::for_rpc_request(
            self.config.timeout_ms,
            None,
            None,
            Some(UPriority::UPRIORITY_CS4),
        );

        self.rpc
            .invoke_method(self.reset_config_method.clone(), options, None)
            .await
            .map_err(|e| ClientError::Invoke(e.to_string()))?;
        Ok(())
    }
}
