// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Discovery and loading: the jobs that answer a selection in the UI, and
//! record what they read as metrics on the way.

use super::{Event, Job, Worker};
use bacnet_probe::metrics::{self, DeviceLabels};
use bacnet_probe::scan::{self, ObjectReport};
use bacnet_probe::{Device, ObjectId, props};

impl Worker {
    pub(super) fn discover(&mut self) {
        self.emit(Event::Discovering);
        match self.client.discover(&self.plan) {
            Ok(devices) => {
                metrics::record_discovery(self.client.meter(), devices.len());
                self.emit(Event::Log(format!(
                    "Who-Is: {} datagram(s), {} device(s) answered",
                    self.plan.len(),
                    devices.len()
                )));
                self.emit(Event::Found(devices));
            }
            Err(e) => {
                self.emit(Event::Log(format!("Who-Is failed: {e}")));
                self.emit(Event::Found(Vec::new()));
            }
        }
    }

    pub(super) fn load_detail(&mut self, device: &Device, id: ObjectId) {
        if let Some(report) = self.read(Job::Detail, device, id, &props::for_kind(id.kind)) {
            self.record(device.instance, &report);
            self.emit(Event::Detail(device.instance, report));
        }
    }

    pub(super) fn load_device(&mut self, device: &Device) {
        let me = ObjectId::new(8, device.instance);
        let job = Job::Device(device.instance);
        let Some(info) = self.read(job, device, me, &scan::DEVICE_SUMMARY) else {
            return self.emit(Event::DeviceDone(device.instance, false));
        };
        self.names.insert(device.instance, info.name());
        let count = scan::object_count(&mut self.client, device);
        metrics::record_device(
            self.client.meter(),
            device,
            Some(&info),
            true,
            count.as_ref().ok().copied(),
        );
        self.emit(Event::DeviceInfo(device.instance, info));
        let ids = match count.and_then(|n| {
            self.emit(Event::ObjectCount(device.instance, n));
            scan::object_ids(&mut self.client, device, n)
        }) {
            Ok(ids) => ids,
            Err(e) => {
                self.emit(Event::Log(format!("device {}: {e}", device.instance)));
                return self.emit(Event::DeviceDone(device.instance, true));
            }
        };
        let ids: Vec<ObjectId> = ids.into_iter().filter(|id| *id != me).collect();
        for chunk in ids.chunks(scan::BATCH) {
            if self.superseded(job) {
                return self.emit(Event::DeviceDone(device.instance, false));
            }
            let wanted: Vec<(ObjectId, Vec<u32>)> = chunk
                .iter()
                .map(|id| (*id, scan::summary_props(id.kind)))
                .collect();
            for report in scan::read_many(&mut self.client, device, &wanted) {
                self.record(device.instance, &report);
                self.emit(Event::Object(device.instance, report));
            }
        }
        self.emit(Event::DeviceDone(device.instance, true));
    }

    /// One `ReadPropertyMultiple` where the device has it; otherwise one read
    /// per property, giving up (`None`) as soon as the job is superseded.
    fn read(
        &mut self,
        job: Job,
        device: &Device,
        id: ObjectId,
        wanted: &[u32],
    ) -> Option<ObjectReport> {
        if self.client.supports_rpm(device.instance) {
            let report = scan::read_many(&mut self.client, device, &[(id, wanted.to_vec())]).pop();
            if self.client.supports_rpm(device.instance) {
                return report;
            }
        }
        let mut report = ObjectReport {
            id,
            reads: Vec::new(),
        };
        for &prop in wanted {
            if self.superseded(job) {
                return None;
            }
            report
                .reads
                .extend(scan::read(&mut self.client, device, id, &[prop]).reads);
        }
        Some(report)
    }

    /// Records an object's readings as metrics (a no-op without `--otlp`).
    pub(super) fn record(&mut self, instance: u32, report: &ObjectReport) {
        let name = self.names.get(&instance).map_or("", String::as_str);
        metrics::record_object(self.client.meter(), DeviceLabels { instance, name }, report);
        self.labels
            .insert((instance, report.id), (report.name(), report.units()));
    }
}
