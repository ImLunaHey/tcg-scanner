//! Transport-independent scanner RPC. Authenticate and bound concurrency in the
//! transport before dispatching; this crate opens no listener or child process.
mod library;
mod model;
mod params;
mod profiles;
mod router;
mod scanner;
pub use model::{Failure, Request, Response};
pub use router::Router;
