export RUST_BACKTRACE := "1"

ui_test:
   cargo test
   
loom_test:
   RUSTFLAGS="--cfg loom" cargo test 

test: ui_test loom_test
