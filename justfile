export RUST_BACKTRACE := "1"

ui_test:
   cargo test
   
loom_test:
   RUSTFLAGS="--cfg loom" cargo test -- --test-threads 1

test: ui_test loom_test
