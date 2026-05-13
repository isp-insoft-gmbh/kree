use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use kree::config;

fn parse_config(c: &mut Criterion) {
    c.bench_function("config_parse_template", |b| {
        b.iter(|| {
            let parsed: config::Config = toml::from_str(config::TEMPLATE).expect("parses");
            assert!(parsed.chime);
            parsed
        });
    });
}

fn patch_config(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_patch");
    for patch in [
        config::ConfigPatch::Chime(false),
        config::ConfigPatch::Speak(false),
        config::ConfigPatch::PopupPosition(config::PopupPosition::BottomRight),
        config::ConfigPatch::Theme(config::ThemeChoice::Light),
    ] {
        group.bench_with_input(format!("{patch:?}"), &patch, |b, patch| {
            b.iter(|| {
                let updated =
                    config::apply_patch_to_string(config::TEMPLATE, *patch).expect("patches");
                assert!(!updated.is_empty());
                updated
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
    targets = parse_config, patch_config
}
criterion_main!(benches);
