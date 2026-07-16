//! Fixtures de estado mínimo para tests del crate (#151, #152).
//!
//! Visible en unit e integration tests vía `openttdrs_core::test_fixtures`,
//! sin re-exportar en la API runtime de la raíz.

mod sandbox_map;
mod sim_harness;

pub use sandbox_map::{SANDBOX_MONEY, SandboxMap};
pub use sim_harness::SimHarness;
