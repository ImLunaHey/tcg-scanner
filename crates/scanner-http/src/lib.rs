//! In-process HTTP adapter. The host owns the Engine and listener lifetime.
//! Bind only to the validated configuration address and call Engine::shutdown
//! after draining requests. Credentials are accepted exclusively as Bearer headers.
mod access;
mod configuration;
mod error;
mod events;
mod gateway;
mod images;
mod lease;
mod path;
mod routes;
mod server;
pub use access::generate_token;
pub use configuration::Configuration;
pub use gateway::Gateway;
