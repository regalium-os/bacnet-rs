// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A minimal JSON writer (std only), and the mapping from `BACnet` values to
//! JSON: raw numbers stay numbers, enumerations gain their standard name.

use std::fmt::Write;

/// A JSON value. Objects keep insertion order, so output reads in the order
/// it was built.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    /// `null`.
    Null,
    /// `true` / `false`.
    Bool(bool),
    /// A signed integer, written exactly.
    Int(i64),
    /// An unsigned integer, written exactly.
    UInt(u64),
    /// A float; NaN and infinities are written as `null`.
    Num(f64),
    /// A string, escaped on output.
    Str(String),
    /// An array.
    Arr(Vec<Json>),
    /// An object, in insertion order.
    Obj(Vec<(String, Json)>),
}

impl Json {
    /// An object from `(key, value)` pairs.
    pub fn obj<'a>(pairs: impl IntoIterator<Item = (&'a str, Json)>) -> Self {
        Self::Obj(pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
    }

    /// A string value.
    pub fn str(s: impl Into<String>) -> Self {
        Self::Str(s.into())
    }

    /// Single-line JSON, for one-event-per-line streams.
    #[must_use]
    pub fn compact(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, None, 0);
        out
    }

    /// Indented JSON, for a document a person will read.
    #[must_use]
    pub fn pretty(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, Some(2), 0);
        out
    }

    fn write(&self, out: &mut String, indent: Option<usize>, depth: usize) {
        let newline = |out: &mut String, depth: usize| {
            if let Some(n) = indent {
                out.push('\n');
                out.extend(std::iter::repeat_n(' ', n * depth));
            }
        };
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Self::Int(n) => out.push_str(&n.to_string()),
            Self::UInt(n) => out.push_str(&n.to_string()),
            // JSON has no NaN or infinity; a sensor fault can still send one.
            Self::Num(n) if !n.is_finite() => out.push_str("null"),
            Self::Num(n) => out.push_str(&n.to_string()),
            Self::Str(s) => escape(out, s),
            Self::Arr(items) if items.is_empty() => out.push_str("[]"),
            Self::Obj(pairs) if pairs.is_empty() => out.push_str("{}"),
            Self::Arr(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    newline(out, depth + 1);
                    item.write(out, indent, depth + 1);
                }
                newline(out, depth);
                out.push(']');
            }
            Self::Obj(pairs) => {
                out.push('{');
                for (i, (key, value)) in pairs.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    newline(out, depth + 1);
                    escape(out, key);
                    out.push_str(if indent.is_some() { ": " } else { ":" });
                    value.write(out, indent, depth + 1);
                }
                newline(out, depth);
                out.push('}');
            }
        }
    }
}

fn escape(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

mod value;

pub use value::{value, values};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tag::Value;

    #[test]
    fn strings_are_escaped_and_non_finite_numbers_become_null() {
        let doc = Json::obj([("name", Json::str("a \"b\"\n")), ("v", Json::Num(f64::NAN))]);
        assert_eq!(doc.compact(), r#"{"name":"a \"b\"\n","v":null}"#);
    }

    #[test]
    fn an_enumeration_carries_its_standard_name() {
        let j = value(Some(36), Some(0), &Value::Enumerated(3));
        assert_eq!(j.compact(), r#"{"value":3,"name":"high-limit"}"#);
    }

    #[test]
    fn status_flags_become_named_booleans() {
        let j = value(
            Some(111),
            None,
            &Value::Bits(vec![true, false, false, false]),
        );
        assert_eq!(
            j.compact(),
            r#"{"in-alarm":true,"fault":false,"overridden":false,"out-of-service":false}"#
        );
    }

    #[test]
    fn an_array_property_with_one_element_is_still_an_array() {
        assert_eq!(
            values(110, 19, &[Value::Text("Normal".into())]).compact(),
            r#"["Normal"]"#
        );
        assert_eq!(values(85, 0, &[Value::Real(21.5)]).compact(), "21.5");
    }
}
