//! Re-export interno para llamadas entre submódulos de transporte.
pub(in crate::command::transport) use super::rail::*;
pub(in crate::command::transport) use super::road::*;
pub(in crate::command::transport) use super::shared::*;
