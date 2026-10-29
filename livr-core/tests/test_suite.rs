//! The LIVR test suite.

mod common;

fn main() {
    let args = libtest_mimic::Arguments::from_args();
    libtest_mimic::run(&args, common::trials(Clone::clone)).exit();
}
