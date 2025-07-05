export RUST_BACKTRACE := "1"

ui_test:
   cargo test --release
   
miri_test:
   # miriflags are mostly for crossbeam, but it still only works on master
   MIRIFLAGS="-Zmiri-strict-provenance -Zmiri-symbolic-alignment-check -Zmiri-disable-isolation -Zmiri-disable-stacked-borrows" cargo miri test 
   
loom_test:
   RUSTFLAGS="--cfg loom" cargo test -- --test-threads 1 loom_tests

test: ui_test loom_test
