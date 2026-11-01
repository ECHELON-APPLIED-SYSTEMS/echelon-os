// Compile the production module itself, not a copy or alternative implementation.
#[path = "../../../src-tauri/src/services/portable.rs"]
pub mod portable;

#[path = "../../../src-tauri/src/services/navcom.rs"]
pub mod navcom;

#[path = "../../../src-tauri/src/services/eve/mod.rs"]
pub mod eve;
