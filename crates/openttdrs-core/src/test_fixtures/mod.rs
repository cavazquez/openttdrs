//! Fixtures de estado mínimo para tests del crate (#151).
//!
//! Visible en unit e integration tests vía `openttdrs_core::test_fixtures`,
//! sin re-exportar en la API runtime de la raíz.

mod sandbox_map;

pub use sandbox_map::{SANDBOX_MONEY, SandboxMap};
