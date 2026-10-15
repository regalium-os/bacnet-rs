// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Box-drawn tables: the device list, one row per object, or with `--detail`
//! a property/value table per object.

use bacnet_probe::scan::{DeviceReport, ObjectReport};
use bacnet_probe::{Device, explain, names, props};

/// Cells longer than this wrap onto further lines rather than being cut.
const WRAP: usize = 64;

pub(crate) fn devices(reports: &[DeviceReport]) -> String {
    let rows = reports
        .iter()
        .map(|r| {
            let d = &r.device;
            let text = |p| {
                r.this
                    .get(p)
                    .map(|v| explain::many(p, 8, v, None))
                    .unwrap_or_default()
            };
            vec![
                d.instance.to_string(),
                r.this.name(),
                d.addr.to_string(),
                route(d),
                text(121),
                text(70),
                text(44),
                d.max_apdu.to_string(),
                d.vendor.to_string(),
            ]
        })
        .collect();
    render(
        &[
            "DEVICE",
            "NAME",
            "ADDRESS",
            "ROUTE",
            "VENDOR",
            "MODEL",
            "FIRMWARE",
            "MAX-APDU",
            "VENDOR-ID",
        ],
        rows,
    )
}

/// One row per object: OBJECT, NAME, VALUE, STATUS.
pub(crate) fn objects(report: &DeviceReport) -> String {
    let rows = report
        .objects
        .iter()
        .map(|o| {
            let value =
                o.reads
                    .iter()
                    .find(|(p, _)| *p == 85)
                    .map_or_else(String::new, |(_, r)| match r {
                        Ok(_) => present_value(o),
                        Err(e) => format!("<{e}>"),
                    });
            let status = o
                .get(111)
                .map_or_else(String::new, |v| explain::many(111, o.id.kind, v, None));
            let status = if status == "all clear" {
                "ok".into()
            } else {
                status
            };
            vec![names::object(o.id), o.name(), value, status]
        })
        .collect();
    render(&["OBJECT", "NAME", "VALUE", "STATUS"], rows)
}

/// Every property read from one object, one row each.
pub(crate) fn properties(o: &ObjectReport) -> String {
    let rows = o
        .reads
        .iter()
        .map(|(prop, result)| {
            let value = match result {
                Ok(_) if *prop == 85 => present_value(o),
                Ok(values) => explain::many(*prop, o.id.kind, values, o.units()),
                Err(e) => format!("<{e}>"),
            };
            vec![props::name(*prop), value]
        })
        .collect();
    render(&["PROPERTY", "VALUE"], rows)
}

/// The present-value with units and the device's name for the state.
pub(crate) fn present_value(o: &ObjectReport) -> String {
    o.present_value().map_or_else(String::new, |v| {
        explain::present(o.id.kind, v, o.units(), &o.labels())
    })
}

fn route(d: &Device) -> String {
    d.route.as_ref().map_or_else(
        || "direct".into(),
        |r| {
            let mac: Vec<String> = r.mac.iter().map(|b| format!("{b:02x}")).collect();
            format!("net {} mac {}", r.net, mac.join(":"))
        },
    )
}

/// Renders rows under headers with box-drawing borders. Strings print
/// without quotes; a cell wider than `WRAP` wraps at spaces.
pub(crate) fn render(headers: &[&str], rows: Vec<Vec<String>>) -> String {
    let rows: Vec<Vec<Vec<String>>> = rows
        .into_iter()
        .map(|row| row.into_iter().map(|c| wrap(c.trim_matches('"'))).collect())
        .collect();
    let width = |i: usize| {
        let body = rows
            .iter()
            .filter_map(|r| r.get(i))
            .flatten()
            .map(|l| l.chars().count());
        body.chain([headers.get(i).map_or(0, |h| h.chars().count())])
            .max()
            .unwrap_or(0)
    };
    let widths: Vec<usize> = (0..headers.len()).map(width).collect();
    let rule = |l: &str, m: &str, r: &str| {
        let bars: Vec<String> = widths.iter().map(|w| "─".repeat(w + 2)).collect();
        format!("{l}{}{r}\n", bars.join(m))
    };
    let line = |cells: &[&str]| {
        let padded: Vec<String> = widths
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let c = cells.get(i).copied().unwrap_or("");
                format!(" {c}{} ", " ".repeat(w - c.chars().count()))
            })
            .collect();
        format!("│{}│\n", padded.join("│"))
    };
    let mut out = rule("┌", "┬", "┐");
    out += &line(headers);
    out += &rule("├", "┼", "┤");
    for row in &rows {
        let height = row.iter().map(Vec::len).max().unwrap_or(1);
        for n in 0..height {
            let cells: Vec<&str> = row
                .iter()
                .map(|c| c.get(n).map_or("", String::as_str))
                .collect();
            out += &line(&cells);
        }
    }
    out += &rule("└", "┴", "┘");
    out
}

fn wrap(cell: &str) -> Vec<String> {
    let mut lines = vec![String::new()];
    for word in cell.split(' ') {
        let last = lines.last_mut().map_or(0, |l| l.chars().count());
        if last > 0 && last + 1 + word.chars().count() > WRAP {
            lines.push(String::new());
        }
        if let Some(l) = lines.last_mut() {
            if !l.is_empty() {
                l.push(' ');
            }
            l.push_str(word);
        }
    }
    lines
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn columns_size_to_their_widest_cell_and_long_cells_wrap() {
        let long = "word ".repeat(20);
        let out = render(&["A", "B"], vec![vec!["xyz".into(), long.trim().into()]]);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines[0].chars().count(),
            lines[3].chars().count(),
            "\n{out}"
        );
        assert!(lines.len() > 5, "the long cell should wrap:\n{out}");
        assert!(lines[3].starts_with("│ xyz │"), "\n{out}");
    }
}
