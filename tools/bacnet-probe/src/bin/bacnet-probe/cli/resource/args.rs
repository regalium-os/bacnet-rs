// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The resource verbs' arguments.

use crate::args::Format;
use clap::Args;

#[derive(Args)]
pub(crate) struct NameArgs {
    /// `devices/599`, `devices/599/objects/analog-input-1`, or a property:
    /// `devices/599/objects/analog-input-1/properties/present-value`.
    pub(super) name: String,
    #[arg(long, value_enum, default_value_t)]
    pub(super) format: Format,
}

#[derive(Args)]
pub(crate) struct WriteArgs {
    /// A property, or an object (meaning its present-value).
    pub(super) name: String,
    /// The value, read as the type the property holds: `75`, `open`,
    /// `Independent`, `true`, or `null` to relinquish at --priority.
    pub(super) value: String,
    /// 1 (highest) to 16, for a commandable present-value.
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=16))]
    pub(super) priority: Option<u8>,
    #[arg(long, value_enum, default_value_t)]
    pub(super) format: Format,
}

#[derive(Args)]
pub(crate) struct CreateArgs {
    /// The device to create it in: `devices/599`.
    pub(super) parent: String,
    /// The object type: `analog-value`, `binary-value`, `multi-state-value`.
    #[arg(long = "type")]
    pub(super) object_type: String,
    /// An exact identifier, `analog-value-7`; else the device chooses.
    #[arg(long)]
    pub(super) id: Option<String>,
    /// Its object-name.
    #[arg(long)]
    pub(super) display_name: Option<String>,
    /// Its initial present-value, as `write` reads one.
    #[arg(long)]
    pub(super) value: Option<String>,
    #[arg(long, value_enum, default_value_t)]
    pub(super) format: Format,
}
