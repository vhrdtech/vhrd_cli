//! The tests of vc_picker that drive the example CLI `pickdemo`: its registration (`registration`), and the picker
//! on Tab and on Alt+D in bash, zsh and fish in a pty (`shells`).
mod registration;
mod shells;

use vc_test::{Scratch, example};

/// A scratch directory with `pickdemo` in its `bin/`.
fn scratch(name: &str) -> Scratch {
    Scratch::new(name).with_bin("pickdemo", &example("pickdemo"))
}
