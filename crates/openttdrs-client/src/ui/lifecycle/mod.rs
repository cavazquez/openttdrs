//! Teardown declarativo de sesión al salir de `InGame` (#145).

mod entity_cleanup;
mod plugin;
mod resource_reset;

#[cfg(test)]
mod registry_tests;

pub(crate) use plugin::InGameLifecyclePlugin;
