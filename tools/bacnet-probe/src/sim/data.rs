// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The simulated gateway's objects and their properties.

use super::{CAR_LOAD, DEVICE, DOOR, FAN, MACHINE_ROOM, MODE};
use crate::tag::{ObjectId, Value};
use std::collections::HashMap;

/// Every property of every object, keyed by (object, property).
pub(super) type Table = HashMap<(ObjectId, u32), Vec<Value>>;

/// A gateway with one car: load, door, mode, a fan, and a hot machine room.
/// Advertises reading, writing, creating and deleting, `SubscribeCOV`,
/// Who-Is and Who-Has, as a modern controller does.
pub(super) fn lift_gateway() -> Table {
    let text = |s: &str| vec![Value::Text(s.into())];
    let one = |v: Value| vec![v];
    let ok = || one(Value::Bits(vec![false; 4]));
    let fan_slots: Vec<Value> = (1..=16)
        .map(|slot| {
            if slot == 8 {
                Value::Real(60.0)
            } else {
                Value::Null
            }
        })
        .collect();
    let services: Vec<bool> = (0..41)
        .map(|i| matches!(i, 5 | 10 | 11 | 12 | 14 | 15 | 16 | 26 | 27 | 33 | 34))
        .collect();
    let types: Vec<bool> = (0..60)
        .map(|i| matches!(i, 0 | 1 | 2 | 3 | 5 | 8 | 19))
        .collect();
    let objects = [DEVICE, CAR_LOAD, MACHINE_ROOM, FAN, DOOR, MODE]
        .map(Value::Object)
        .to_vec();
    let entries: Vec<(ObjectId, u32, Vec<Value>)> = vec![
        (DEVICE, 77, text("Lift gateway (simulated)")),
        (
            DEVICE,
            28,
            text("bacnet-probe simulator: one car behind a BACnet/IP gateway"),
        ),
        (DEVICE, 121, text("RegaliumOS simulator")),
        (DEVICE, 120, one(Value::Unsigned(999))),
        (DEVICE, 70, text("probe-sim")),
        (DEVICE, 44, text("1.0.0")),
        (DEVICE, 12, text("sim 2026.10")),
        (DEVICE, 58, text("Machine room, level 12")),
        (DEVICE, 112, one(Value::Enumerated(0))),
        (DEVICE, 98, one(Value::Unsigned(1))),
        (DEVICE, 139, one(Value::Unsigned(22))),
        (DEVICE, 155, one(Value::Unsigned(7))),
        (DEVICE, 62, one(Value::Unsigned(1476))),
        (DEVICE, 107, one(Value::Enumerated(3))),
        (DEVICE, 11, one(Value::Unsigned(3000))),
        (DEVICE, 73, one(Value::Unsigned(3))),
        (DEVICE, 56, one(Value::Date([126, 10, 15, 4]))),
        (DEVICE, 57, one(Value::Time([9, 30, 0, 0]))),
        (DEVICE, 97, one(Value::Bits(services))),
        (DEVICE, 96, one(Value::Bits(types))),
        (DEVICE, 76, objects),
        (CAR_LOAD, 77, text("Car 1 load")),
        (CAR_LOAD, 28, text("Load weighing cell under car 1")),
        (CAR_LOAD, 85, one(Value::Real(420.0))),
        (CAR_LOAD, 117, one(Value::Enumerated(39))),
        (CAR_LOAD, 22, one(Value::Real(5.0))),
        (CAR_LOAD, 69, one(Value::Real(0.0))),
        (CAR_LOAD, 65, one(Value::Real(1000.0))),
        (MACHINE_ROOM, 77, text("Machine room temperature")),
        (MACHINE_ROOM, 85, one(Value::Real(41.5))),
        (MACHINE_ROOM, 117, one(Value::Enumerated(62))),
        (
            MACHINE_ROOM,
            111,
            one(Value::Bits(vec![true, false, false, false])),
        ),
        (MACHINE_ROOM, 36, one(Value::Enumerated(3))),
        (FAN, 77, text("Machine room fan speed")),
        (FAN, 85, one(Value::Real(60.0))),
        (FAN, 117, one(Value::Enumerated(98))),
        (FAN, 87, fan_slots),
        (FAN, 104, one(Value::Real(0.0))),
        (DOOR, 77, text("Car 1 door")),
        (DOOR, 85, one(Value::Enumerated(0))),
        (DOOR, 4, text("open")),
        (DOOR, 46, text("closed")),
        (DOOR, 84, one(Value::Enumerated(0))),
        (MODE, 77, text("Car 1 operating mode")),
        (MODE, 85, one(Value::Unsigned(2))),
        (MODE, 74, one(Value::Unsigned(4))),
        (
            MODE,
            110,
            ["Out of service", "Normal", "Independent", "Fire service"]
                .map(|s| Value::Text(s.into()))
                .to_vec(),
        ),
    ];
    let mut props: HashMap<_, _> = entries.into_iter().map(|(o, p, v)| ((o, p), v)).collect();
    for point in [CAR_LOAD, MACHINE_ROOM, FAN, DOOR, MODE] {
        props.entry((point, 111)).or_insert_with(ok);
        props
            .entry((point, 36))
            .or_insert_with(|| one(Value::Enumerated(0)));
        props
            .entry((point, 103))
            .or_insert_with(|| one(Value::Enumerated(0)));
        props
            .entry((point, 81))
            .or_insert_with(|| one(Value::Bool(false)));
        props.insert((point, 75), one(Value::Object(point)));
        props.insert((point, 79), one(Value::Enumerated(point.kind.into())));
    }
    props
}
