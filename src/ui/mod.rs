//! Phase U — operator shell (instrument panel over the live ledger).
//!
//! Map root + pan/zoom + time controls + Chronicle/History strip + system inspector.
//! Issue 11: EventLog moments only — no tick-perfect recording or replay viewer.

mod app;
mod map;

pub use app::{run, HeliosApp};
