//! The tests of vc_complete, driving the example CLI `vcdemo`: what the binary answers each shell (`protocol`), and
//! TAB in the real shells (`shells`).
mod protocol;
mod shells;

use vc_test::{Scratch, example};

/// A scratch directory with `vcdemo` in its `bin/`.
fn scratch(name: &str) -> Scratch {
    Scratch::new(name).with_bin("vcdemo", &example("vcdemo"))
}
