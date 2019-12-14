extern crate lz4;
extern crate brotli;

use {
    lz4::{EncoderBuilder},
    brotli::{CompressorWriter},
    std::io::{Write, Read},
};

// compress incomming video using LZ4 and delete source file
pub fn compress_file_lz4(source: &str, destination: &str) -> std::io::Result<()> {
    let mut input_file = std::fs::File::open(source)?;
    let output_file = std::fs::File::create(destination)?;
    let mut encoder = EncoderBuilder::new()
        .level(4)
        .build(output_file).unwrap();

    std::io::copy(&mut input_file, &mut encoder).unwrap();
    let (_output, result) = encoder.finish();
    std::fs::remove_file(source)?;
    result
}

// compress incomming video using Brotli and delete source file
pub fn compress_file_brotli(source: &str, destination: &str) -> std::io::Result<()> {
    let mut input_file = std::fs::File::open(source)?;
    let output_file = std::fs::File::create(destination)?;
    const LEN: usize = 16 * 1024; // 16 Kb

    let mut writer = CompressorWriter::new(output_file, LEN, 11, 22);
    let mut buf = vec![0u8; LEN];

    loop {
        // Read a buffer from the file.
        let n = input_file.read(&mut buf)?;

         // If this is the end of file, clean up and return.
         if n == 0 {
            writer.flush()?;
            std::fs::remove_file(source)?;
            return Ok(());
        }

        // Write the buffer into compressor.
        writer.write_all(&buf[..n])?;
    }

    
}