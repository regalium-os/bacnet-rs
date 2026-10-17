// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Reading a primitive from an element's contents (§20.2.2–§20.2.14).

use super::{BitString, CharacterString, Date, ObjectIdentifier, Primitive, Time, number, tag};
use crate::tag::{Class, Element, Error, ErrorKind, Lvt};

impl<'a> Primitive<'a> {
    /// The value of an application-tagged element.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::WrongTag`] for a context or constructed element;
    /// [`ErrorKind::WrongLength`] or [`ErrorKind::InvalidValue`] for contents
    /// the type does not allow. The error's bytes are the contents.
    pub fn decode(element: &Element<'a>) -> Result<Self, Error<'a>> {
        match (element.header.class, element.header.lvt) {
            (Class::Application, Lvt::Boolean(v)) => match v {
                0 | 1 => Ok(Self::Boolean(v == 1)),
                _ => Err(Error::new(
                    ErrorKind::InvalidValue { tag: tag::BOOLEAN },
                    element.content,
                    0,
                )),
            },
            (Class::Application, Lvt::Length(_)) => {
                Self::decode_as(element.header.number, element.content)
            }
            _ => Err(Error::new(ErrorKind::WrongTag, element.content, 0)),
        }
    }

    /// `content` read as application type `tag`, for a context-tagged
    /// element whose type the grammar gives. A context BOOLEAN is one octet
    /// of contents (§20.2.3), unlike an application one.
    ///
    /// # Errors
    ///
    /// As [`Primitive::decode`]; an unknown `tag` is [`ErrorKind::WrongTag`].
    pub fn decode_as(tag: u8, content: &'a [u8]) -> Result<Self, Error<'a>> {
        let len = |ok: bool| {
            if ok {
                Ok(())
            } else {
                Err(Error::new(
                    ErrorKind::WrongLength {
                        tag,
                        len: content.len(),
                    },
                    content,
                    0,
                ))
            }
        };
        let invalid = || Error::new(ErrorKind::InvalidValue { tag }, content, 0);
        let four = || -> Result<[u8; 4], Error<'a>> {
            content.try_into().map_err(|_| {
                Error::new(
                    ErrorKind::WrongLength {
                        tag,
                        len: content.len(),
                    },
                    content,
                    0,
                )
            })
        };
        Ok(match tag {
            tag::NULL => len(content.is_empty()).map(|()| Self::Null)?,
            tag::BOOLEAN => match content {
                [0] => Self::Boolean(false),
                [1] => Self::Boolean(true),
                [_] => return Err(invalid()),
                _ => return len(false).map(|()| Self::Null),
            },
            tag::UNSIGNED => len((1..=8).contains(&content.len()))
                .map(|()| Self::Unsigned(number::unsigned(content)))?,
            tag::SIGNED => len((1..=8).contains(&content.len()))
                .map(|()| Self::Signed(number::signed(content)))?,
            tag::REAL => Self::Real(f32::from_be_bytes(four()?)),
            tag::DOUBLE => {
                let octets: [u8; 8] = content.try_into().map_err(|_| {
                    Error::new(
                        ErrorKind::WrongLength {
                            tag,
                            len: content.len(),
                        },
                        content,
                        0,
                    )
                })?;
                Self::Double(f64::from_be_bytes(octets))
            }
            tag::OCTET_STRING => Self::OctetString(content),
            tag::CHARACTER_STRING => match content.split_first() {
                Some((&charset, bytes)) => {
                    Self::CharacterString(CharacterString { charset, bytes })
                }
                None => return len(false).map(|()| Self::Null),
            },
            tag::BIT_STRING => match content.split_first() {
                Some((&unused, bytes)) if unused <= 7 && (unused == 0 || !bytes.is_empty()) => {
                    Self::BitString(BitString { unused, bytes })
                }
                Some(_) => return Err(invalid()),
                None => return len(false).map(|()| Self::Null),
            },
            tag::ENUMERATED => {
                len((1..=4).contains(&content.len()))?;
                Self::Enumerated(u32::try_from(number::unsigned(content)).map_err(|_| invalid())?)
            }
            tag::DATE => {
                let [year, month, day, weekday] = four()?;
                Self::Date(Date {
                    year,
                    month,
                    day,
                    weekday,
                })
            }
            tag::TIME => {
                let [hour, minute, second, hundredths] = four()?;
                Self::Time(Time {
                    hour,
                    minute,
                    second,
                    hundredths,
                })
            }
            tag::OBJECT_IDENTIFIER => {
                Self::ObjectIdentifier(ObjectIdentifier::from_raw(u32::from_be_bytes(four()?)))
            }
            _ => return Err(Error::new(ErrorKind::WrongTag, content, 0)),
        })
    }
}
