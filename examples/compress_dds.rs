use nvtt_rs::{
    CompressionOptions, CompressionOutput, Compressor, InputOptions, OutputLocation, OutputOptions,
};
use std::{env, error::Error, path::PathBuf};

fn example_data_path() -> Option<PathBuf> {
    env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .map(|p| p.join("example_data"))
}

#[cfg(feature = "nvtt_image_integration")]
fn main() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    use image::open;

    let compressor = Compressor::new()?;
    let mut input_options = InputOptions::new()?;
    let mut output_options = OutputOptions::new()?;
    let mut compression_options = CompressionOptions::new()?;

    let example_data_path = example_data_path().expect("Could not find example data");

    Ok(())
}

#[cfg(not(feature = "nvtt_image_integration"))]
fn main() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    Err("Must enable the `nvtt_image_integration` feature to run this example".into())
}
