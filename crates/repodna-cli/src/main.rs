//! The `repodna` binary.

/// Static Linux builds link musl, whose allocator makes multi-threaded analysis several
/// times slower; they use mimalloc instead.
#[cfg(target_env = "musl")]
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> std::process::ExitCode {
    repodna_cli::run()
}
