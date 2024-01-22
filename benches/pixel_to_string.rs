use criterion::{black_box, criterion_group, criterion_main, Criterion};
use bad_apple_flut::{Pixel, Color};
use rand::Rng;

pub fn criterion_benchmark(c: &mut Criterion) {
    // random pixels
    let mut rng = rand::thread_rng();
    let px = black_box(
        Pixel {
            x: rng.gen_range(0..1000),
            y: rng.gen_range(0..1000),
            color: Color {
                r: rng.gen_range(0..255),
                g: rng.gen_range(0..255),
                b: rng.gen_range(0..255),
            }
        }
    );

    c.bench_function("pixel_to_string", |b| b.iter(|| {
        px.to_pixelflut_string(400, 0);
    }));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);