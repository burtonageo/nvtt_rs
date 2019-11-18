# nvtt_rs

nvtt is a library for converting textures into common compressed formats for use
with graphics APIs. See the [wiki] for more info.

## Example

```no_run
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# let (w, h) = (0, 0);
use nvtt_rs::{Compressor, CompressionOptions, Format, InputOptions, OutputOptions};

let input_options = InputOptions::new()?;

let mut output_options = OutputOptions::new()?;
output_options.set_output_location("output.dds");

let mut compression_opts = CompressionOptions::new()?;
compression_opts.set_format(Format::Dxt1);

let mut compressor = Compressor::new()?;
compressor.compress(&compression_opts, &input_options, &output_options)?;

# Ok(())
# }
```

## Features

### `nvtt_image_integration`

This feature provides the convenience method [`InputOptions::set_image`], which
can be used to configure the `InputOptions` directly from types provided by the
[`image`] crate.

Only a limited number of image formats are supported, although this library can
provide automatic conversions from a [`DynamicImage`]. See the [`ValidImage`]
type for more information.

## Dependencies

### Linux/macOS

This crate requires a valid cmake installation and a C++ compiler to build.

### Windows

This crate requires a valid installation of Visual Studio.

## Notes

This crate does not currently work on Microsoft Windows due to incomplete work
on the build system.


[wiki]: https://github.com/castano/nvidia-texture-tools/wiki/ApiDocumentation
[`InputOptions::set_image`]: struct.InputOptions.html#method.set_image
[`image`]: https://docs.rs/image/latest/image
[`DynamicImage`]: https://docs.rs/image/latest/image/enum.DynamicImage.html
[`ValidImage`]: enum.ValidImage.html
