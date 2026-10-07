// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0
/* Portions of this file were generated with AI assistance. */

//! uProtocol client/adapter for the ECU `getLogs` service.
//!
//! The SOVD server uses [`UProtocolLogProvider`] (an `opensovd_core::LogProvider`)
//! to pull logs from a vehicle ECU over the uProtocol Communication Layer,
//! instead of serving in-memory seed data. The toos-api codegen layer is not
//! used; the `getLogs` request/response travel as JSON (see [`wire`]).

pub mod client;
pub mod provider;
pub mod wire;

pub use client::{ClientError, LogServiceClient, ServiceConfig};
pub use provider::UProtocolLogProvider;
pub use wire::{LogQuery, LogResponse, WireLogEntry};
