use dipstick::*;
use std::time::Duration;
use slugify::slugify;

pub struct Metrics {
    pub statsd: Statsd
}

impl Metrics {
    pub fn update(&mut self, d: &Duration, host: &str) {
        let slug = slugify!(host);
        println!("{} {:?} {}", slug, d, d.as_micros() as u64);
        self.statsd.metrics().counter(&*slug).count(1);
        self.statsd.metrics().timer(&*slug).interval_us(d.as_micros() as u64); 
    }
}

pub fn metrics(name: &str) -> Metrics {
    let statsd = dipstick::Statsd::send_to("localhost:8125")
        .expect("Connected")
        .named(name);

    let metrics = Metrics {
        statsd
    };
    metrics
}

