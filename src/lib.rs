#![warn(clippy::cast_lossless)]

#[cfg(test)]
#[macro_use(quickcheck)]
extern crate quickcheck_macros;

pub mod converter;
pub mod ddr;
mod mini_parser;
pub mod osu;
pub mod utils;
pub mod xact3;
