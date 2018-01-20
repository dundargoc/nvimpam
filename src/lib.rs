//! The companion library to the nvimpam binary.
extern crate failure;
#[macro_use]
extern crate log;
extern crate neovim_lib;

pub mod handler;
pub mod event;
pub mod folds;
pub mod neovim_ext;
pub mod card;
pub mod codeyard;
pub mod lines;
pub mod carddata;
