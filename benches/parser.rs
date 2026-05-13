use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use kree::parser;

fn reminder_file(lines: usize) -> String {
    const SCHEDULES: &[&str] = &[
        "0 9 * * 1-5",
        "25 9 * * 1-5",
        "30 9-16 * * 1-5",
        "0 16 * * 1-5",
    ];
    const MESSAGES: &[&str] = &[
        "🌅 Morning - check planner, make day plan, start timer",
        "🎙️ Standup in 5 - shipped, blocked, next",
        "💪 Move, drink, push-ups",
        "📝 Log time, update follow-ups",
    ];

    let mut input = String::with_capacity(lines * 96);
    input.push_str("# generated benchmark reminders\n\n");
    for i in 0..lines {
        let idx = i % SCHEDULES.len();
        input.push_str(SCHEDULES[idx]);
        input.push_str(" | ");
        input.push_str(MESSAGES[idx]);
        input.push('\n');
    }
    input
}

fn parse_reminders(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_reminders");
    for lines in [4usize, 100, 1_000] {
        let input = reminder_file(lines);
        group.bench_with_input(format!("{lines}_lines"), &input, |b, input| {
            b.iter(|| {
                let report = parser::parse(input);
                assert!(report.errors.is_empty());
                assert_eq!(report.reminders.len(), lines);
                report
            });
        });
    }
    group.finish();
}

fn mixed_reminder_file(lines: usize) -> String {
    let mut input = String::with_capacity(lines * 72);
    for i in 0..lines {
        match i % 10 {
            0 => input.push_str("# comment\n"),
            1 => input.push('\n'),
            2 => input.push_str("not a cron | invalid\n"),
            _ => input.push_str("30 9-16 * * 1-5 | 💪 Move, drink, push-ups\n"),
        }
    }
    input
}

fn parse_mixed_reminders(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_mixed_reminders");
    for lines in [1_000usize, 10_000] {
        let input = mixed_reminder_file(lines);
        group.bench_with_input(format!("{lines}_lines"), &input, |b, input| {
            b.iter(|| {
                let report = parser::parse(input);
                assert!(!report.reminders.is_empty());
                assert!(!report.errors.is_empty());
                report
            });
        });
    }
    group.finish();
}

fn parse_line_cases(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_line");
    for (name, line) in [
        ("plain", "0 9 * * 1-5 | Plain reminder"),
        ("emoji", "0 9 * * 1-5 | 💧 Drink water"),
        (
            "zwj_emoji",
            "0 9 * * 1-5 | 👨\u{200D}👩\u{200D}👧 Family reminder",
        ),
        ("pipes", "0 9 * * 1-5 | Review | follow-up | close loop"),
        ("invalid_cron", "nope | Invalid"),
        ("missing_separator", "0 9 * * 1-5 Missing separator"),
        ("empty_message", "0 9 * * 1-5 |   "),
    ] {
        group.bench_with_input(name, line, |b, line| b.iter(|| parser::parse_line(line)));
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(3));
    targets = parse_reminders, parse_mixed_reminders, parse_line_cases
}
criterion_main!(benches);
