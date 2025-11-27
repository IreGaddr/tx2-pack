use crate::error::{PackError, Result};
use crate::format::CompressionType;

#[derive(Debug, Clone, Copy)]
pub enum CompressionCodec {
    None,
    Zstd(i32),
    Lz4,
}

impl CompressionCodec {
    pub fn none() -> Self {
        CompressionCodec::None
    }

    pub fn zstd_default() -> Self {
        CompressionCodec::Zstd(3)
    }

    pub fn zstd_fast() -> Self {
        CompressionCodec::Zstd(1)
    }

    pub fn zstd_best() -> Self {
        CompressionCodec::Zstd(19)
    }

    pub fn lz4_default() -> Self {
        CompressionCodec::Lz4
    }
}

impl From<CompressionCodec> for CompressionType {
    fn from(codec: CompressionCodec) -> Self {
        match codec {
            CompressionCodec::None => CompressionType::None,
            CompressionCodec::Zstd(_) => CompressionType::Zstd,
            CompressionCodec::Lz4 => CompressionType::Lz4,
        }
    }
}

pub fn compress(data: &[u8], codec: CompressionCodec) -> Result<Vec<u8>> {
    match codec {
        CompressionCodec::None => Ok(data.to_vec()),

        CompressionCodec::Zstd(level) => {
            zstd::bulk::compress(data, level)
                .map_err(|e| PackError::Compression(e.to_string()))
        }

        CompressionCodec::Lz4 => {
            let mut encoder = lz4::EncoderBuilder::new()
                .level(4)
                .build(Vec::new())
                .map_err(|e| PackError::Compression(e.to_string()))?;

            std::io::copy(&mut &data[..], &mut encoder)
                .map_err(|e| PackError::Compression(e.to_string()))?;

            let (compressed, result) = encoder.finish();
            result.map_err(|e| PackError::Compression(e.to_string()))?;

            Ok(compressed)
        }
    }
}

pub fn decompress(data: &[u8], compression_type: CompressionType) -> Result<Vec<u8>> {
    match compression_type {
        CompressionType::None => Ok(data.to_vec()),

        CompressionType::Zstd => {
            zstd::bulk::decompress(data, 100 * 1024 * 1024)
                .map_err(|e| PackError::Decompression(e.to_string()))
        }

        CompressionType::Lz4 => {
            let mut decoder = lz4::Decoder::new(data)
                .map_err(|e| PackError::Decompression(e.to_string()))?;

            let mut decompressed = Vec::new();
            std::io::copy(&mut decoder, &mut decompressed)
                .map_err(|e| PackError::Decompression(e.to_string()))?;

            Ok(decompressed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zstd_compression() {
        let data = b"Hello, World! This is a test of ZSTD compression.".repeat(100);

        let compressed = compress(&data, CompressionCodec::zstd_default()).unwrap();
        assert!(compressed.len() < data.len());

        let decompressed = decompress(&compressed, CompressionType::Zstd).unwrap();
        assert_eq!(data, decompressed);
    }

    #[test]
    fn test_lz4_compression() {
        let data = b"Hello, World! This is a test of LZ4 compression.".repeat(100);

        let compressed = compress(&data, CompressionCodec::Lz4).unwrap();
        assert!(compressed.len() < data.len());

        let decompressed = decompress(&compressed, CompressionType::Lz4).unwrap();
        assert_eq!(data, decompressed);
    }

    #[test]
    fn test_no_compression() {
        let data = b"Hello, World!";

        let compressed = compress(data, CompressionCodec::None).unwrap();
        assert_eq!(data, compressed.as_slice());

        let decompressed = decompress(&compressed, CompressionType::None).unwrap();
        assert_eq!(data, decompressed.as_slice());
    }
}
