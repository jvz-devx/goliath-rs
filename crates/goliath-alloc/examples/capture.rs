//! Exercises recursive allocation accounting with the diagnostic global allocator.
#[global_allocator]
static ALLOCATOR: goliath_alloc::Allocator = goliath_alloc::Allocator;

fn main() {
    goliath_alloc::start_capture(32, 1024).expect("capture starts once");
    let retained = vec![7u8; 1024 * 1024];
    for n in 0..1000 {
        let temporary = vec![n as u8; 128];
        std::hint::black_box(&temporary);
    }
    let capture = goliath_alloc::stop_capture().expect("capture output");
    assert!(!capture.sampler_failed);
    assert_eq!(capture.missed_tls_callbacks, 0);
    assert!(
        capture
            .samples
            .iter()
            .any(|sample| sample.bytes == retained.len())
    );
    assert!(capture.counts.live_bytes >= retained.len());
    std::hint::black_box(&retained);
    println!(
        "global capture passed; {} retained samples",
        capture.samples.len()
    );
}
