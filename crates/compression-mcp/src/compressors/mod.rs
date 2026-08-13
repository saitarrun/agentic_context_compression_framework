pub mod cache_aligner;
pub mod smart_crusher;
pub mod code_compressor;
pub mod kompress_base;
pub mod repo_map;

use mcp_types::MpcError;

/// Trait for compression algorithms.
/// Each compressor handles a specific content type and preserves signal while removing noise.
pub trait Compressor: Send + Sync {
    /// Compress the given content.
    /// Returns the compressed output and the compression ratio (original_size / compressed_size).
    fn compress(&self, content: &str) -> Result<(String, f64), MpcError>;

    /// Get the name of this compressor.
    fn name(&self) -> &str;
}

// Re-export concrete implementations
pub use cache_aligner::CacheAligner;
pub use smart_crusher::SmartCrusher;
pub use code_compressor::CodeCompressor;
pub use kompress_base::KompressBase;
pub use repo_map::RepoMapCompressor;
