// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A pure-Rust `BACnet` driver for `RegaliumOS`.
//!
//! Implements ASHRAE 135 / ISO 16484-5. The protocol layers live in sibling
//! crates; what a consumer can reach is exactly what this crate re-exports,
//! which leaves the internals free to change without breaking anyone.
//!
//! # Re-export, never wrap
//!
//! Everything here is a `pub use`. A re-export is the same type, so a value
//! obtained through this crate satisfies the traits declared inside the layer
//! that defined it, and vice versa — which is what keeps the extension points
//! open. A third party can implement a datalink for a proprietary bridge, or
//! add an object type, without this repository being involved. A newtype
//! wrapper would compile and then silently close every one of those doors.
//!
//! # Layer contract
//!
//! Owns: the public surface, and nothing else. No logic lives here.
//!
//! Allowed imports: every crate in the workspace.
