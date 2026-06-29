#![allow(dead_code)]

pub mod apps;
pub mod boot;
pub mod contracts;
pub mod display;
pub mod imported;
pub mod input;

// Optional Lua app runtime probe. Disabled by default; no SD app loading.
pub mod io;
pub mod lua;
pub mod physical;
pub mod ring_remote;
pub mod runtime_adapter_contracts;

// Rustmix-owned progress state boundary.
pub mod state;

pub mod ui;

pub mod text;

pub mod sleep;

pub mod time;

pub mod network;

// Rustmix-owned runtime modules migrated out of target-xteink-x4/src/rustmix_x4.
pub mod x4_apps;
pub mod x4_kernel;
