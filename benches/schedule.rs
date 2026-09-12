use std::time::Duration;

use chrono::Local;
use criterion::{Criterion, criterion_group, criterion_main};
use kree::parser;

fn parse_schedule(schedule: &str) -> parser::Reminder {
    parser::parse_line(&format!("{schedule} | bench")).expect("schedule parses")
}

fn next_occurrence_patterns(c: &mut Criterion) {
    let now = Local::now();
    let mut group = c.benchmark_group("next_occurrence");
    for schedule in ["*/1 * * * *", "30 9-16 * * 1-5", "0 9 1 * *", "0 9 29 2 *"] {
        let reminder = parse_schedule(schedule);
        group.bench_with_input(schedule, &reminder, |b, reminder| {
            b.iter(|| reminder.cron.next_after(now))
        });
    }
    group.finish();
}

fn next_occurrence_batches(c: &mut Criterion) {
    let now = Local::now();
    let mut group = c.benchmark_group("next_occurrence_batch");
    for count in [10usize, 100, 1_000] {
        let reminders: Vec<_> = (0..count)
            .map(|i| match i % 4 {
                0 => parse_schedule("*/1 * * * *"),
                1 => parse_schedule("30 9-16 * * 1-5"),
                2 => parse_schedule("0 9 1 * *"),
                _ => parse_schedule("0 16 * * 1-5"),
            })
            .collect();
        group.bench_with_input(format!("{count}_reminders"), &reminders, |b, reminders| {
            b.iter(|| {
                reminders
                    .iter()
                    .map(|r| r.cron.next_after(now))
                    .collect::<Option<Vec<_>>>()
            });
        });
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(20)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(1));
    targets = next_occurrence_patterns, next_occurrence_batches
}
criterion_main!(benches);
