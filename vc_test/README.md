# vc_test

The test harness of the vhrd_cli crates, not published: finds the example binaries cargo builds with the tests, makes scratch directories, starts commands with a clean environment, and runs bash, zsh and fish interactively in a pty so a test can type into them and read the line back.

Only the dev-dependencies of the workspace's crates use it.
