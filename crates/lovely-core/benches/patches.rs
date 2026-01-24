use std::path::Path;
use std::sync::OnceLock;
use std::env;
use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use crop::Rope;
use lovely_core::patch::{InsertPosition, PatternPatch, RegexPatch, Target};

const SHORT_SAMPLE_SIZE: usize = 10;

const SAMPLE_BUFFER: &str = include_str!("assets/sample_buffer.txt");
const SAMPLE_BUFFER_SHORT: &str = include_str!("assets/sample_buffer_short.txt");
const DUMMY_PATH: &str = "benches/sample_buffer.txt";

fn configure_criterion() -> Criterion {
    let short_mode = env::var("LOVELY_BENCH_MODE")
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "short" | "dev"))
        .unwrap_or(false);

    let mut criterion = Criterion::default();
    if short_mode {
        eprintln!("running benchmarks in short mode (sample size = {SHORT_SAMPLE_SIZE})");
        criterion = criterion.sample_size(SHORT_SAMPLE_SIZE);
    }

    criterion
}

fn pattern_patches_no_match() -> &'static [PatternPatch] {
    static PATCHES: OnceLock<Vec<PatternPatch>> = OnceLock::new();
    PATCHES.get_or_init(|| {
        vec![
            PatternPatch {
                target: Target::Single("sample_buffer.txt".to_string()),
                pattern: "ABC".to_string(),
                position: InsertPosition::At,
                payload: "REPLACED".to_string(),
                match_indent: false,
                times: None,
                overwrite: false,
                name: None, silent: false,
            },
            PatternPatch {
                target: Target::Single("sample_buffer.txt".to_string()),
                pattern: "XYZ\n123".to_string(),
                position: InsertPosition::At,
                payload: "REPLACED".to_string(),
                match_indent: false,
                times: None,
                overwrite: false,
                name: None, silent: false,
            },
            PatternPatch {
                target: Target::Single("sample_buffer.txt".to_string()),
                pattern: "function process_data(input)\n    local result = {}\n    for i, v in ipairs(input) do\n        result[i] = v * 2\n    end\n    return result\nend".to_string(),
                position: InsertPosition::At,
                payload: "REPLACED".to_string(),
                match_indent: false,
                times: None,
                overwrite: false,
                name: None, silent: false,
            },
            PatternPatch {
                target: Target::Single("sample_buffer.txt".to_string()),
                pattern: "if condition_one and condition_two then\n    perform_action()\n    update_state()\nelseif condition_three then\n    alternative_action()\nelse\n    default_behavior()\nend".to_string(),
                position: InsertPosition::At,
                payload: "REPLACED".to_string(),
                match_indent: false,
                times: None,
                overwrite: false,
                name: None, silent: false,
            },
        ]
    })
}

fn pattern_patches_with_match() -> &'static [PatternPatch] {
    static PATCHES: OnceLock<Vec<PatternPatch>> = OnceLock::new();
    PATCHES.get_or_init(|| {
        vec![
            PatternPatch {
                target: Target::Single("sample_buffer.txt".to_string()),
                pattern: "NXKOO".to_string(),
                position: InsertPosition::At,
                payload: "REPLACED".to_string(),
                match_indent: false,
                times: Some(1),
                overwrite: false,
                name: None, silent: false,
            },
            PatternPatch {
                target: Target::Single("sample_buffer.txt".to_string()),
                pattern: "NXKOONXKOO".to_string(),
                position: InsertPosition::At,
                payload: "REPLACED".to_string(),
                match_indent: false,
                times: Some(5),
                overwrite: false,
                name: None, silent: false,
            },
            PatternPatch {
                target: Target::Single("sample_buffer.txt".to_string()),
                pattern: "NXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOO\nNXKOONXKOONXKOONXKOONXKOO".to_string(),
                position: InsertPosition::At,
                payload: "-- REPLACED BLOCK --\n-- END REPLACED BLOCK --".to_string(),
                match_indent: false,
                times: Some(1),
                overwrite: false,
                name: None, silent: false,
            },
            PatternPatch {
                target: Target::Single("sample_buffer.txt".to_string()),
                pattern: "NXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOO\nNXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOO\nNXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOONXKOO".to_string(),
                position: InsertPosition::At,
                payload: "-- COMPLEX REPLACED BLOCK --\n-- MULTIPLE LINES --\n-- END COMPLEX REPLACED BLOCK --".to_string(),
                match_indent: false,
                times: Some(2),
                overwrite: false,
                name: None, silent: false,
            },
        ]
    })
}

fn regex_patches_no_match() -> &'static [RegexPatch] {
    static PATCHES: OnceLock<Vec<RegexPatch>> = OnceLock::new();
    PATCHES.get_or_init(|| {
        vec![
            RegexPatch {
                target: Target::Single("sample_buffer.txt".to_string()),
                pattern: r"ABC".to_string(),
                position: InsertPosition::At,
                root_capture: None,
                payload: "REPLACED".to_string(),
                line_prepend: String::new(),
                times: None,
                verbose: false,
                name: None, silent: false,
            },
            RegexPatch {
                target: Target::Single("sample_buffer.txt".to_string()),
                pattern: r"\d+\s*[a-z]+".to_string(),
                position: InsertPosition::At,
                root_capture: None,
                payload: "REPLACED".to_string(),
                line_prepend: String::new(),
                times: None,
                verbose: false,
                name: None, silent: false,
            },
            RegexPatch {
                target: Target::Single("sample_buffer.txt".to_string()),
                pattern: r"function\s+\w+\s*\([^)]*\)\s*\n\s*local\s+\w+\s*=\s*\{[^}]*\}\s*\n\s*for\s+\w+,\s*\w+\s+in\s+ipairs\([^)]+\)\s+do".to_string(),
                position: InsertPosition::At,
                root_capture: None,
                payload: "REPLACED".to_string(),
                line_prepend: String::new(),
                times: None,
                verbose: false,
                name: None, silent: false,
            },
            RegexPatch {
                target: Target::Single("sample_buffer.txt".to_string()),
                pattern: r"if\s+\w+\s+and\s+\w+\s+then\s*\n(?:\s+\w+\([^)]*\)\s*\n)+\s*elseif\s+\w+\s+then\s*\n(?:\s+\w+\([^)]*\)\s*\n)+\s*else".to_string(),
                position: InsertPosition::At,
                root_capture: None,
                payload: "REPLACED".to_string(),
                line_prepend: String::new(),
                times: None,
                verbose: false,
                name: None, silent: false,
            },
        ]
    })
}

fn regex_patches_with_match() -> &'static [RegexPatch] {
    static PATCHES: OnceLock<Vec<RegexPatch>> = OnceLock::new();
    PATCHES.get_or_init(|| {
        vec![
            RegexPatch {
                target: Target::Single("sample_buffer.txt".to_string()),
                pattern: r"NXKOO".to_string(),
                position: InsertPosition::At,
                root_capture: None,
                payload: "REPLACED".to_string(),
                line_prepend: String::new(),
                times: Some(1),
                verbose: false,
                name: None, silent: false,
            },
            RegexPatch {
                target: Target::Single("sample_buffer.txt".to_string()),
                pattern: r"NX[A-Z]{3}".to_string(),
                position: InsertPosition::At,
                root_capture: None,
                payload: "REPLACED".to_string(),
                line_prepend: String::new(),
                times: Some(5),
                verbose: false,
                name: None, silent: false,
            },
            RegexPatch {
                target: Target::Single("sample_buffer.txt".to_string()),
                pattern: r"(?:NXKOO){10}\n(?:NXKOO){5}".to_string(),
                position: InsertPosition::At,
                root_capture: None,
                payload: "-- REPLACED BLOCK --\n-- END REPLACED BLOCK --".to_string(),
                line_prepend: String::new(),
                times: Some(1),
                verbose: false,
                name: None, silent: false,
            },
            RegexPatch {
                target: Target::Single("sample_buffer.txt".to_string()),
                pattern: r"(NXKOO){20}\n(NXKOO){20}\n(NXKOO){20}".to_string(),
                position: InsertPosition::At,
                root_capture: None,
                payload: "-- COMPLEX REPLACED BLOCK --\n-- MULTIPLE LINES --\n-- END COMPLEX REPLACED BLOCK --".to_string(),
                line_prepend: String::new(),
                times: Some(2),
                verbose: false,
                name: None, silent: false,
            },
        ]
    })
}

fn pattern_patches_position() -> &'static [PatternPatch] {
    static PATCHES: OnceLock<Vec<PatternPatch>> = OnceLock::new();
    PATCHES.get_or_init(|| {
        vec![
            PatternPatch {
                target: Target::Single("sample_buffer.txt".to_string()),
                pattern: "BEGINNING*".to_string(),
                position: InsertPosition::At,
                payload: "REPLACED_BEGINNING".to_string(),
                match_indent: false,
                times: Some(1),
                overwrite: false,
                name: None, silent: false,
            },
            PatternPatch {
                target: Target::Single("sample_buffer.txt".to_string()),
                pattern: "MIDDLE*".to_string(),
                position: InsertPosition::At,
                payload: "REPLACED_MIDDLE".to_string(),
                match_indent: false,
                times: Some(1),
                overwrite: false,
                name: None, silent: false,
            },
            PatternPatch {
                target: Target::Single("sample_buffer.txt".to_string()),
                pattern: "END*".to_string(),
                position: InsertPosition::At,
                payload: "REPLACED_END".to_string(),
                match_indent: false,
                times: Some(1),
                overwrite: false,
                name: None, silent: false,
            },
        ]
    })
}

fn regex_patches_position() -> &'static [RegexPatch] {
    static PATCHES: OnceLock<Vec<RegexPatch>> = OnceLock::new();
    PATCHES.get_or_init(|| {
        vec![
            RegexPatch {
                target: Target::Single("sample_buffer.txt".to_string()),
                pattern: r"BEGINNING.*".to_string(),
                position: InsertPosition::At,
                root_capture: None,
                payload: "REPLACED_BEGINNING".to_string(),
                line_prepend: String::new(),
                times: Some(1),
                verbose: false,
                name: None, silent: false,
            },
            RegexPatch {
                target: Target::Single("sample_buffer.txt".to_string()),
                pattern: r"MIDDLE.*".to_string(),
                position: InsertPosition::At,
                root_capture: None,
                payload: "REPLACED_MIDDLE".to_string(),
                line_prepend: String::new(),
                times: Some(1),
                verbose: false,
                name: None, silent: false,
            },
            RegexPatch {
                target: Target::Single("sample_buffer.txt".to_string()),
                pattern: r"END.*".to_string(),
                position: InsertPosition::At,
                root_capture: None,
                payload: "REPLACED_END".to_string(),
                line_prepend: String::new(),
                times: Some(1),
                verbose: false,
                name: None, silent: false,
            },
        ]
    })
}

fn benchmark_pattern_no_match(c: &mut Criterion) {
    let patches = pattern_patches_no_match();
    let path = Path::new(DUMMY_PATH);

    let mut group = c.benchmark_group("patch::pattern_no_match");
    for (i, patch) in patches.iter().enumerate() {
        group.bench_function(format!("patch_{}", i), |b| {
            b.iter_batched(
                || Rope::from(SAMPLE_BUFFER),
                |mut rope| {
                    let _ = patch.apply("sample_buffer.txt", &mut rope, path);
                    black_box(rope);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn benchmark_pattern_with_match(c: &mut Criterion) {
    let patches = pattern_patches_with_match();
    let path = Path::new(DUMMY_PATH);

    let mut group = c.benchmark_group("patch::pattern_with_match");
    for (i, patch) in patches.iter().enumerate() {
        group.bench_function(format!("patch_{}", i), |b| {
            b.iter_batched(
                || Rope::from(SAMPLE_BUFFER),
                |mut rope| {
                    let _ = patch.apply("sample_buffer.txt", &mut rope, path);
                    black_box(rope);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn benchmark_regex_no_match(c: &mut Criterion) {
    let patches = regex_patches_no_match();
    let path = Path::new(DUMMY_PATH);

    let mut group = c.benchmark_group("patch::regex_no_match");
    for (i, patch) in patches.iter().enumerate() {
        group.bench_function(format!("patch_{}", i), |b| {
            b.iter_batched(
                || Rope::from(SAMPLE_BUFFER),
                |mut rope| {
                    let _ = patch.apply("sample_buffer.txt", &mut rope, path);
                    black_box(rope);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn benchmark_regex_with_match(c: &mut Criterion) {
    let patches = regex_patches_with_match();
    let path = Path::new(DUMMY_PATH);

    let mut group = c.benchmark_group("patch::regex_with_match");
    for (i, patch) in patches.iter().enumerate() {
        group.bench_function(format!("patch_{}", i), |b| {
            b.iter_batched(
                || Rope::from(SAMPLE_BUFFER),
                |mut rope| {
                    let _ = patch.apply("sample_buffer.txt", &mut rope, path);
                    black_box(rope);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn benchmark_pattern_no_match_short(c: &mut Criterion) {
    let patches = pattern_patches_no_match();
    let path = Path::new(DUMMY_PATH);

    let mut group = c.benchmark_group("patch::pattern_no_match_short");
    for (i, patch) in patches.iter().enumerate() {
        group.bench_function(format!("patch_{}", i), |b| {
            b.iter_batched(
                || Rope::from(SAMPLE_BUFFER_SHORT),
                |mut rope| {
                    let _ = patch.apply("sample_buffer.txt", &mut rope, path);
                    black_box(rope);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn benchmark_pattern_with_match_short(c: &mut Criterion) {
    let patches = pattern_patches_with_match();
    let path = Path::new(DUMMY_PATH);

    let mut group = c.benchmark_group("patch::pattern_with_match_short");
    for (i, patch) in patches.iter().enumerate() {
        group.bench_function(format!("patch_{}", i), |b| {
            b.iter_batched(
                || Rope::from(SAMPLE_BUFFER_SHORT),
                |mut rope| {
                    let _ = patch.apply("sample_buffer.txt", &mut rope, path);
                    black_box(rope);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn benchmark_regex_no_match_short(c: &mut Criterion) {
    let patches = regex_patches_no_match();
    let path = Path::new(DUMMY_PATH);

    let mut group = c.benchmark_group("patch::regex_no_match_short");
    for (i, patch) in patches.iter().enumerate() {
        group.bench_function(format!("patch_{}", i), |b| {
            b.iter_batched(
                || Rope::from(SAMPLE_BUFFER_SHORT),
                |mut rope| {
                    let _ = patch.apply("sample_buffer.txt", &mut rope, path);
                    black_box(rope);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn benchmark_regex_with_match_short(c: &mut Criterion) {
    let patches = regex_patches_with_match();
    let path = Path::new(DUMMY_PATH);

    let mut group = c.benchmark_group("patch::regex_with_match_short");
    for (i, patch) in patches.iter().enumerate() {
        group.bench_function(format!("patch_{}", i), |b| {
            b.iter_batched(
                || Rope::from(SAMPLE_BUFFER_SHORT),
                |mut rope| {
                    let _ = patch.apply("sample_buffer.txt", &mut rope, path);
                    black_box(rope);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn benchmark_pattern_position_long(c: &mut Criterion) {
    let patches = pattern_patches_position();
    let path = Path::new(DUMMY_PATH);

    let mut group = c.benchmark_group("patch::pattern_position_long");
    let positions = ["beginning", "middle", "end"];
    for (i, patch) in patches.iter().enumerate() {
        group.bench_function(positions[i], |b| {
            b.iter_batched(
                || Rope::from(SAMPLE_BUFFER),
                |mut rope| {
                    let _ = patch.apply("sample_buffer.txt", &mut rope, path);
                    black_box(rope);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn benchmark_pattern_position_short(c: &mut Criterion) {
    let patches = pattern_patches_position();
    let path = Path::new(DUMMY_PATH);

    let mut group = c.benchmark_group("patch::pattern_position_short");
    let positions = ["beginning", "middle", "end"];
    for (i, patch) in patches.iter().enumerate() {
        group.bench_function(positions[i], |b| {
            b.iter_batched(
                || Rope::from(SAMPLE_BUFFER_SHORT),
                |mut rope| {
                    let _ = patch.apply("sample_buffer.txt", &mut rope, path);
                    black_box(rope);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn benchmark_regex_position_long(c: &mut Criterion) {
    let patches = regex_patches_position();
    let path = Path::new(DUMMY_PATH);

    let mut group = c.benchmark_group("patch::regex_position_long");
    let positions = ["beginning", "middle", "end"];
    for (i, patch) in patches.iter().enumerate() {
        group.bench_function(positions[i], |b| {
            b.iter_batched(
                || Rope::from(SAMPLE_BUFFER),
                |mut rope| {
                    let _ = patch.apply("sample_buffer.txt", &mut rope, path);
                    black_box(rope);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn benchmark_regex_position_short(c: &mut Criterion) {
    let patches = regex_patches_position();
    let path = Path::new(DUMMY_PATH);

    let mut group = c.benchmark_group("patch::regex_position_short");
    let positions = ["beginning", "middle", "end"];
    for (i, patch) in patches.iter().enumerate() {
        group.bench_function(positions[i], |b| {
            b.iter_batched(
                || Rope::from(SAMPLE_BUFFER_SHORT),
                |mut rope| {
                    let _ = patch.apply("sample_buffer.txt", &mut rope, path);
                    black_box(rope);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group! {
    name = patch_benches;
    config = configure_criterion();
    targets =
        benchmark_pattern_no_match,
        benchmark_pattern_with_match,
        benchmark_regex_no_match,
        benchmark_regex_with_match,
        benchmark_pattern_no_match_short,
        benchmark_pattern_with_match_short,
        benchmark_regex_no_match_short,
        benchmark_regex_with_match_short,
        benchmark_pattern_position_long,
        benchmark_pattern_position_short,
        benchmark_regex_position_long,
        benchmark_regex_position_short
}
criterion_main!(patch_benches);
