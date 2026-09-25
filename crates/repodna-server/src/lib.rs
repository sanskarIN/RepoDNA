//! The RepoDNA local server behind `repodna serve`.
//!
//! It serves the web interface and a JSON API for stored analyses, and it can start
//! analyses in the background. It listens on 127.0.0.1 only. Every API request must carry
//! the session token, either in a header or in a `SameSite=Strict` cookie set when the
//! user opens the link printed at startup; requests with any other `Host` header are
//! refused (which defeats DNS rebinding), and requests that change state are refused when
//! their `Origin` is another site. Responses carry a restrictive Content Security Policy.
//! Nothing is uploaded anywhere: the server only reads local storage and the paths the
//! user asks it to analyze.

pub mod assets;
pub mod http;

pub use assets::Assets;
