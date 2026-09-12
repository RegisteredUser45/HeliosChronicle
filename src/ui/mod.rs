//! Phase U — operator shell (instrument panel over the live ledger).
//!
//! Map root + pan/zoom + time controls + Chronicle/History strip + System /
//! Body / Fleet (ship) / Research / Design inspectors. Issue 11: EventLog
//! moments only — no tick-perfect recording or replay viewer. STATEMENT §11:
//! several windows may be open at once; UI is one client of the live World.
//! Viewpoint is a knowledge filter (Operator = fog off; Empire = contact fog).

mod app;
mod fog;
mod map;

pub use app::{run, HeliosApp};
