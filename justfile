export RUST_BACKTRACE := "1"

test: check ui_tests recycler_tests loom_tests

check:
   cargo check --all

ui_tests:
   cargo test --release -- ui_tests src/
   
miri_tests:
   # miriflags are mostly for crossbeam, but it still only works on master
   MIRIFLAGS="-Zmiri-strict-provenance -Zmiri-symbolic-alignment-check -Zmiri-disable-isolation -Zmiri-disable-stacked-borrows" cargo miri test 
   
loom_tests:
   RUSTFLAGS="--cfg loom" cargo test -- --test-threads 1 loom_tests

recycler_tests:
   cargo test --release -- --test-threads 1 recycler_tests
