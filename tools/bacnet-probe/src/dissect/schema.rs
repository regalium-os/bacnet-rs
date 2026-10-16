// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! What each service's parameters are called, by nesting path and tag
//! number (Clauses 13–16), so the walker can label any of them without a
//! decoder per service.

use crate::service::{confirmed, unconfirmed};

/// A service body the dissector knows the grammar of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Service {
    ReadProperty,
    RpmRequest,
    RpmAck,
    SubscribeCov,
    CovNotification,
    WhoIs,
    WhoHas,
    IAm,
    IHave,
    WriteProperty,
    WpmRequest,
    CreateRequest,
    CreateAck,
    DeleteRequest,
    Error,
    /// `CreateObject-Error`: the error, then which initial value failed.
    CreateError,
    /// `WritePropertyMultiple-Error`: the error, then which write failed.
    WpmError,
    Other,
}

/// How a context-tagged parameter's data should be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Shape {
    Object,
    Property,
    Unsigned,
    Bool,
    Text,
    /// An object type number, named.
    Kind,
    Constructed,
}

impl Service {
    pub(super) fn confirmed(choice: u8, ack: bool) -> Self {
        match (choice, ack) {
            (confirmed::READ_PROPERTY, _) => Self::ReadProperty,
            (confirmed::READ_PROPERTY_MULTIPLE, false) => Self::RpmRequest,
            (confirmed::READ_PROPERTY_MULTIPLE, true) => Self::RpmAck,
            (confirmed::SUBSCRIBE_COV, false) => Self::SubscribeCov,
            (confirmed::COV_NOTIFICATION, false) => Self::CovNotification,
            (confirmed::WRITE_PROPERTY, false) => Self::WriteProperty,
            (confirmed::WRITE_PROPERTY_MULTIPLE, false) => Self::WpmRequest,
            (confirmed::CREATE_OBJECT, false) => Self::CreateRequest,
            (confirmed::CREATE_OBJECT, true) => Self::CreateAck,
            (confirmed::DELETE_OBJECT, false) => Self::DeleteRequest,
            _ => Self::Other,
        }
    }

    /// An Error-PDU's body, whose shape depends on the service that failed.
    pub(super) fn error(choice: u8) -> Self {
        match choice {
            confirmed::CREATE_OBJECT => Self::CreateError,
            confirmed::WRITE_PROPERTY_MULTIPLE => Self::WpmError,
            _ => Self::Error,
        }
    }

    pub(super) fn unconfirmed(choice: u8) -> Self {
        match choice {
            unconfirmed::I_AM => Self::IAm,
            unconfirmed::I_HAVE => Self::IHave,
            unconfirmed::COV_NOTIFICATION => Self::CovNotification,
            unconfirmed::WHO_HAS => Self::WhoHas,
            unconfirmed::WHO_IS => Self::WhoIs,
            _ => Self::Other,
        }
    }

    /// The name and shape of context tag `n` inside the constructed tags
    /// `path` (outermost first).
    pub(super) fn context(self, path: &[u8], n: u8) -> (&'static str, Shape) {
        use Shape::{Bool, Constructed, Kind, Object, Property, Text, Unsigned};
        // One arm per parameter name, every place it occurs listed together,
        // so a field means the same thing wherever the grammar puts it.
        match (self, path, n) {
            (
                Self::ReadProperty
                | Self::RpmRequest
                | Self::RpmAck
                | Self::WriteProperty
                | Self::WpmRequest,
                [],
                0,
            )
            | (Self::CreateRequest, [0], 1)
            | (Self::WpmError, [1], 0)
            | (Self::WhoHas, [], 2) => ("object-identifier", Object),
            (Self::ReadProperty | Self::WriteProperty, [], 1)
            | (Self::RpmRequest | Self::WpmRequest | Self::CreateRequest, [1], 0)
            | (Self::RpmAck, [1], 2)
            | (Self::CovNotification, [4], 0)
            | (Self::WpmError, [1], 1) => ("property-identifier", Property),
            (Self::ReadProperty | Self::WriteProperty, [], 2)
            | (Self::RpmRequest | Self::WpmRequest | Self::CreateRequest, [1], 1)
            | (Self::RpmAck, [1], 3)
            | (Self::CovNotification, [4], 1) => ("property-array-index", Unsigned),
            (Self::ReadProperty | Self::WriteProperty, [], 3)
            | (Self::WpmRequest | Self::CreateRequest, [1], 2)
            | (Self::RpmAck, [1], 4)
            | (Self::CovNotification, [4], 2) => ("property-value", Constructed),
            (Self::WriteProperty, [], 4)
            | (Self::WpmRequest | Self::CreateRequest, [1], 3)
            | (Self::CovNotification, [4], 3) => ("priority", Unsigned),
            (Self::RpmRequest, [], 1) => ("list-of-property-references", Constructed),
            (Self::RpmAck, [], 1) => ("list-of-results", Constructed),
            (Self::RpmAck, [1], 5) => ("property-access-error", Constructed),
            (Self::WpmRequest, [], 1) => ("list-of-properties", Constructed),
            (Self::CreateRequest, [], 0) => ("object-specifier", Constructed),
            (Self::CreateRequest, [0], 0) => ("object-type", Kind),
            (Self::CreateRequest, [], 1) => ("list-of-initial-values", Constructed),
            (Self::CreateError | Self::WpmError, [], 0) => ("error-type", Constructed),
            (Self::CreateError, [], 1) => ("first-failed-element-number", Unsigned),
            (Self::WpmError, [], 1) => ("first-failed-write-attempt", Constructed),
            (Self::SubscribeCov | Self::CovNotification, [], 0) => {
                ("subscriber-process-identifier", Unsigned)
            }
            (Self::SubscribeCov, [], 1) | (Self::CovNotification, [], 2) => {
                ("monitored-object-identifier", Object)
            }
            (Self::SubscribeCov, [], 2) => ("issue-confirmed-notifications", Bool),
            (Self::SubscribeCov, [], 3) => ("lifetime (s)", Unsigned),
            (Self::CovNotification, [], 1) => ("initiating-device-identifier", Object),
            (Self::CovNotification, [], 3) => ("time-remaining (s)", Unsigned),
            (Self::CovNotification, [], 4) => ("list-of-values", Constructed),
            (Self::WhoIs | Self::WhoHas, [], 0) => ("device-instance-range-low-limit", Unsigned),
            (Self::WhoIs | Self::WhoHas, [], 1) => ("device-instance-range-high-limit", Unsigned),
            (Self::WhoHas, [], 3) => ("object-name", Text),
            _ => ("parameter", Unsigned),
        }
    }

    /// The name of the `position`th application-tagged value directly
    /// inside `path`, where the grammar is positional.
    pub(super) fn positional(self, path: &[u8], position: usize) -> Option<&'static str> {
        const I_AM: [&str; 4] = [
            "i-am-device-identifier",
            "max-apdu-length-accepted",
            "segmentation-supported",
            "vendor-id",
        ];
        const I_HAVE: [&str; 3] = ["device-identifier", "object-identifier", "object-name"];
        const ERROR: [&str; 2] = ["error-class", "error-code"];
        match (self, path) {
            (Self::IAm, []) => I_AM.get(position).copied(),
            (Self::IHave, []) => I_HAVE.get(position).copied(),
            (Self::Error, [])
            | (Self::RpmAck, [1, 5])
            | (Self::CreateError | Self::WpmError, [0]) => ERROR.get(position).copied(),
            (Self::CreateAck | Self::DeleteRequest, []) if position == 0 => {
                Some("object-identifier")
            }
            _ => None,
        }
    }
}
