//! Run on the CPU worker: cargo run --release --offline --example benchmark.
use prim::{Config, prepare};
use std::time::Instant;

const CASES: &[(&str, &str, &str)] = &[
    (
        "integer_loop",
        include_str!("benchmarks/integer_loop.pr"),
        "199990000",
    ),
    ("recursive", include_str!("benchmarks/recursive.pr"), "2584"),
    ("captures", include_str!("benchmarks/captures.pr"), "5000"),
    ("array", include_str!("benchmarks/array.pr"), "20000"),
];

fn main() {
    let repetitions: usize = std::env::args()
        .nth(1)
        .map(|v| v.parse().expect("positive sample count"))
        .unwrap_or(9);
    assert!((1..=1000).contains(&repetitions));
    println!(
        "{{\"metadata\":true,\"llvm\":\"{}\",\"arch\":\"{}\",\"samples\":{repetitions},\"unit\":\"ms\"}}",
        env!("PRIM_LLVM_VERSION"),
        std::env::consts::ARCH
    );
    for (name, source, expected) in CASES {
        for (mode, enabled, opt_level) in [
            ("interpreter", false, 0),
            ("jit-O0", true, 0),
            ("jit-O1", true, 1),
            ("jit-O2", true, 2),
        ] {
            let prepare_started = Instant::now();
            let mut rt = prepare(
                source,
                Config {
                    jit_enabled: enabled,
                    opt_level,
                    capture_output: false,
                    ..Config::default()
                },
            )
            .unwrap();
            let prepare_ms = prepare_started.elapsed().as_secs_f64() * 1000.0;
            let cold = Instant::now();
            assert_eq!(rt.run().unwrap().display(), *expected);
            let cold_ms = cold.elapsed().as_secs_f64() * 1000.0;
            let before = rt.stats.clone();
            let mut samples = Vec::new();
            for _ in 0..repetitions {
                let started = Instant::now();
                let value = rt.run().unwrap();
                samples.push(started.elapsed().as_secs_f64() * 1000.0);
                assert_eq!(value.display(), *expected);
            }
            assert_eq!(rt.stats.compiled_functions, before.compiled_functions);
            let raw_samples = samples
                .iter()
                .map(|n| format!("{n:.6}"))
                .collect::<Vec<_>>()
                .join(",");
            samples.sort_by(f64::total_cmp);
            let median = if repetitions % 2 == 1 {
                samples[repetitions / 2]
            } else {
                (samples[repetitions / 2 - 1] + samples[repetitions / 2]) / 2.0
            };
            let s = &rt.stats;
            let count = repetitions as u64;
            println!(
                "{{\"case\":\"{name}\",\"mode\":\"{mode}\",\"prepare_ms\":{prepare_ms:.6},\"cold_ms\":{cold_ms:.6},\"compile_ms\":{:.6},\"warm_median_ms\":{median:.6},\"warm_samples_ms\":[{raw_samples}],\"steps_per_run\":{},\"helpers_per_run\":{},\"slots_per_run\":{},\"closures_per_run\":{},\"scalar_calls_per_run\":{},\"gc_ms_per_run\":{:.6},\"native_code_bytes\":{}}}",
                before.compile_ms,
                (s.steps - before.steps) / count,
                (s.native_helper_calls - before.native_helper_calls) / count,
                (s.slot_allocations - before.slot_allocations) / count,
                (s.closure_allocations - before.closure_allocations) / count,
                (s.scalar_calls - before.scalar_calls) / count,
                (s.gc_ms - before.gc_ms) / repetitions as f64,
                s.native_code_bytes
            );
        }
    }
}
