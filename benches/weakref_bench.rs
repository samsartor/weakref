use criterion::{Criterion, black_box, criterion_group, criterion_main};
use std::rc::Rc;
use std::sync::Arc;
use weakref::{Own, pin, refer};

fn benchmark_own_box_creation(c: &mut Criterion) {
    c.bench_function("own_new_box", |b| {
        b.iter_with_large_drop(|| Own::new_box(black_box(42)));
    });
}

fn benchmark_own_empty_creation(c: &mut Criterion) {
    c.bench_function("own_new_empty", |b| {
        b.iter_with_large_drop(|| Own::new(()));
    });
}

fn benchmark_own_destruction(c: &mut Criterion) {
    c.bench_function("own_drop_box", |b| {
        b.iter_batched(
            || Own::new_box(42),
            |data| {
                drop(black_box(data));
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn benchmark_ref_access(c: &mut Criterion) {
    c.bench_function("ref_get", |b| {
        let data = Own::new_box(42);
        let weak_ref = refer!(data);
        b.iter(|| {
            let guard = pin();
            let result = weak_ref.get(&guard);
            black_box(result);
        })
    });
}

fn benchmark_ref_access_dead(c: &mut Criterion) {
    c.bench_function("ref_get_dead", |b| {
        let data = Own::new_box(42);
        let weak_ref = refer!(data);
        drop(data);
        b.iter(|| {
            let guard = pin();
            let result = weak_ref.get(&guard);
            black_box(result);
        })
    });
}

fn benchmark_ref_map(c: &mut Criterion) {
    c.bench_function("ref_map", |b| {
        let data = Own::new_box(vec![1, 2, 3, 4, 5]);
        let weak_ref = refer!(data);
        b.iter(|| {
            let mapped = weak_ref.map(|v| &v[2]);
            black_box(mapped);
        })
    });
}

fn benchmark_comparison_arc_weak_creation(c: &mut Criterion) {
    c.bench_function("new_arc_weak", |b| {
        b.iter_with_large_drop(|| {
            let data = Arc::new(42);
            let weak = Arc::downgrade(&data);
            black_box((data, weak))
        })
    });
}

fn benchmark_comparison_arc_weak_empty_creation(c: &mut Criterion) {
    c.bench_function("new_arc_weak_empty", |b| {
        b.iter_with_large_drop(|| {
            let data = Arc::new(());
            let weak = Arc::downgrade(&data);
            black_box((data, weak))
        })
    });
}

fn benchmark_comparison_arc_weak_destruction_ab(c: &mut Criterion) {
    c.bench_function("arc_weak_drop_ab", |b| {
        b.iter_batched(
            || {
                let data = Arc::new(());
                let weak = Arc::downgrade(&data);
                (data, weak)
            },
            |(data, weak)| {
                drop(black_box(data));
                drop(black_box(weak));
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn benchmark_comparison_arc_weak_destruction_ba(c: &mut Criterion) {
    c.bench_function("arc_weak_drop_ba", |b| {
        b.iter_batched(
            || {
                let data = Arc::new(());
                let weak = Arc::downgrade(&data);
                (data, weak)
            },
            |(data, weak)| {
                drop(black_box(weak));
                drop(black_box(data));
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn benchmark_comparison_arc_weak_get(c: &mut Criterion) {
    c.bench_function("arc_weak_get", |b| {
        let data = Arc::new(42);
        let weak = Arc::downgrade(&data);
        b.iter(|| {
            let result = weak.upgrade();
            black_box(result);
        })
    });
}

fn benchmark_comparison_arc_weak_clone(c: &mut Criterion) {
    c.bench_function("arc_weak_clone", |b| {
        let data = Arc::new(42);
        let weak = Arc::downgrade(&data);
        b.iter_with_large_drop(|| black_box(weak.clone()))
    });
}

fn benchmark_heavy_workload(c: &mut Criterion) {
    c.bench_function("heavy_workload_weakref", |b| {
        b.iter(|| {
            let data = Own::new_box(vec![1; 1000]);
            let weak_ref = refer!(data);

            // Simulate multiple accesses
            for _ in 0..100 {
                let guard = pin();
                if let Some(vec) = weak_ref.get(&guard) {
                    black_box(vec.len());
                }
            }

            drop(data);

            // Access after drop
            for _ in 0..100 {
                let guard = pin();
                let result = weak_ref.get(&guard);
                black_box(result);
            }
        })
    });
}

fn benchmark_heavy_workload_arc(c: &mut Criterion) {
    c.bench_function("heavy_workload_arc", |b| {
        b.iter(|| {
            let data = Arc::new(vec![1; 1000]);
            let weak = Arc::downgrade(&data);

            // Simulate multiple accesses
            for _ in 0..100 {
                if let Some(vec) = weak.upgrade() {
                    black_box(vec.len());
                }
            }

            drop(data);

            // Access after drop
            for _ in 0..100 {
                let result = weak.upgrade();
                black_box(result);
            }
        })
    });
}

criterion_group!(
    benches,
    benchmark_own_box_creation,
    benchmark_own_empty_creation,
    benchmark_own_destruction,
    benchmark_ref_access,
    benchmark_ref_access_dead,
    benchmark_ref_map,
    benchmark_comparison_arc_weak_creation,
    benchmark_comparison_arc_weak_empty_creation,
    benchmark_comparison_arc_weak_destruction_ab,
    benchmark_comparison_arc_weak_destruction_ba,
    benchmark_comparison_arc_weak_get,
    benchmark_comparison_arc_weak_clone,
    benchmark_heavy_workload,
    benchmark_heavy_workload_arc,
);

criterion_main!(benches);
