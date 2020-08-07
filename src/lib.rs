#![warn(clippy::cast_lossless)]
#![warn(rust_2018_idioms)]

#[cfg(test)]
#[macro_use(quickcheck)]
extern crate quickcheck_macros;

pub mod converter;
pub mod ddr;
mod mini_parser;
pub mod osu;
pub mod utils;
pub mod xact3;
