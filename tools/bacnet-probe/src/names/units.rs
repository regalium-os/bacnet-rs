// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `BACnetEngineeringUnits` (§21), common ones by symbol and standard name.

/// `BACnetEngineeringUnits` (§21): code, symbol, standard name.
const UNITS: &[(u32, &str, &str)] = &[
    (2, "mA", "milliamperes"),
    (3, "A", "amperes"),
    (4, "Ω", "ohms"),
    (5, "V", "volts"),
    (6, "kV", "kilovolts"),
    (8, "VA", "volt-amperes"),
    (9, "kVA", "kilovolt-amperes"),
    (15, "PF", "power-factor"),
    (16, "J", "joules"),
    (17, "kJ", "kilojoules"),
    (18, "Wh", "watt-hours"),
    (19, "kWh", "kilowatt-hours"),
    (27, "Hz", "hertz"),
    (29, "%RH", "percent-relative-humidity"),
    (30, "mm", "millimeters"),
    (31, "m", "meters"),
    (39, "kg", "kilograms"),
    (47, "W", "watts"),
    (48, "kW", "kilowatts"),
    (53, "Pa", "pascals"),
    (54, "kPa", "kilopascals"),
    (55, "bar", "bars"),
    (62, "°C", "degrees-celsius"),
    (63, "K", "degrees-kelvin"),
    (64, "°F", "degrees-fahrenheit"),
    (70, "d", "days"),
    (71, "h", "hours"),
    (72, "min", "minutes"),
    (73, "s", "seconds"),
    (74, "m/s", "meters-per-second"),
    (75, "km/h", "kilometers-per-hour"),
    (80, "m³", "cubic-meters"),
    (82, "L", "liters"),
    (87, "L/s", "liters-per-second"),
    (88, "L/min", "liters-per-minute"),
    (95, "", "no-units"),
    (96, "ppm", "parts-per-million"),
    (98, "%", "percent"),
    (104, "rpm", "revolutions-per-minute"),
];

/// Suffix for a value: ` °C`, empty for no-units, ` units#N` if unknown.
#[must_use]
pub fn units(code: u32) -> String {
    match UNITS.iter().find(|u| u.0 == code) {
        Some((_, "", _)) => String::new(),
        Some((_, symbol, _)) => format!(" {symbol}"),
        None => format!(" units#{code}"),
    }
}

/// The standard name alone: `degrees-celsius`.
#[must_use]
pub fn unit_label(code: u32) -> Option<String> {
    UNITS.iter().find(|u| u.0 == code).map(|u| u.2.to_owned())
}

/// The full name: `degrees-celsius (62)`.
#[must_use]
pub fn unit_name(code: u32) -> String {
    UNITS
        .iter()
        .find(|u| u.0 == code)
        .map_or_else(|| format!("units #{code}"), |u| format!("{} ({code})", u.2))
}
