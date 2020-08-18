// Copyright © 2019 George Burton
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! # Nvtt
//!
//! nvtt is a library for converting textures into common compressed formats for use
//! with graphics APIs. See the [wiki] for more info.
//!
//! # Example
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let (w, h) = (0, 0);
//! use nvtt_rs::{Compressor, CompressionOptions, Format, InputOptions, OutputOptions};
//!
//! let input_options = InputOptions::new()?;
//!
//! let mut output_options = OutputOptions::new()?;
//! output_options.set_output_location("output.dds");
//!
//! let mut compression_opts = CompressionOptions::new()?;
//! compression_opts.set_format(Format::Dxt1);
//!
//! let mut compressor = Compressor::new()?;
//! compressor.compress(&compression_opts, &input_options, &output_options)?;
//!
//! # Ok(())
//! # }
//! ```
//!
//! # Features
//!
//! ## `nvtt_image_integration`
//!
//! This feature provides the convenience method [`InputOptions::set_image`], which
//! can be used to configure the `InputOptions` directly from types provided by the
//! [`image`] crate.
//!
//! Only a limited number of image formats are supported, although this library can
//! provide automatic conversions from a [`DynamicImage`]. See the [`ValidImage`]
//! type for more information.
//!
//! # Dependencies
//!
//! ## Linux/macOS
//!
//! This crate requires a valid cmake installation and a C++ compiler to build.
//!
//! ## Windows
//!
//! This crate requires a valid installation of Visual Studio.
//!
//! # Notes
//!
//! This crate does not currently work on Microsoft Windows due to incomplete work
//! on the build system.
//!
//! [wiki]: https://github.com/castano/nvidia-texture-tools/wiki/ApiDocumentation
//! [`InputOptions::set_image`]: struct.InputOptions.html#method.set_image
//! [`image`]: https://docs.rs/image/latest/image
//! [`DynamicImage`]: https://docs.rs/image/latest/image/enum.DynamicImage.html
//! [`ValidImage`]: enum.ValidImage.html

#![allow(nonstandard_style)]

use cfg_if::cfg_if;
#[cfg(feature = "nvtt_image_integration")]
use image::{Bgra, DynamicImage, ImageBuffer, Luma, Rgba};
use log::{error, trace};
#[cfg(feature = "nvtt_image_integration")]
use maybe_owned::MaybeOwned;
use nvtt_sys::*;
#[cfg(feature = "nvtt_image_integration")]
use safe_transmute::transmute_to_bytes;
#[cfg(feature = "nvtt_image_integration")]
use std::ops::Deref;
use std::{
    cell::{Cell, RefCell},
    cmp::PartialEq,
    convert::TryFrom,
    error::Error as ErrorTrait,
    ffi::{CStr, CString, NulError, OsStr},
    fmt, mem,
    os::raw::{c_int, c_uint, c_void},
    path::Path,
    ptr::NonNull,
    slice, thread_local,
};

/// Get the version of the linked `nvtt` library.
#[inline(always)]
pub const fn version() -> u32 {
    NVTT_VERSION
}

macro_rules! decl_enum {
    (
        $(#[$($attr:meta)*])*
        $v:vis enum $enum_name:ident: $raw:ident {
            $(
                $(#[$($brnch_attr:meta)*])*
                $rust_nm:ident = $sys_nm:ident
            ),*
            $(,)?
        }
    ) => {
        $(#[$($attr)*])*
        $v enum $enum_name {
            $(
                $(#[$($brnch_attr)*])*
                $rust_nm
            ),*
        }

        impl From<&'_ $enum_name> for $raw {
            #[inline]
            fn from(val: &'_ $enum_name) -> Self {
                From::from(*val)
            }
        }

        impl From<$enum_name> for $raw {
            #[inline]
            fn from(val: $enum_name) -> Self {
                 match val {
                    $(
                        $enum_name :: $rust_nm => $sys_nm
                    ),*
                }
            }
        }

        impl TryFrom<$raw> for $enum_name {
            type Error = ();

            // NvttFormat contains overlapping enum instances, so only the first
            // value declared will be returned.
            #[allow(unreachable_patterns)]
            #[inline]
            fn try_from(raw: $raw) -> Result<Self, Self::Error> {
                match raw {
                    $(
                        $sys_nm => { Ok($enum_name::$rust_nm) }
                    )*
                    _ => Err(())
                }
            }
        }

        impl PartialEq<$raw> for $enum_name {
            #[inline]
            fn eq(&self, rhs: &$raw) -> bool {
                let lhs: $raw = (*self).into();
                lhs == *rhs
            }
        }

        impl PartialEq<$enum_name> for $raw {
            #[inline]
            fn eq(&self, rhs: &$enum_name) -> bool {
                let rhs: $raw = (*rhs).into();
                *self == rhs
            }
        }
    };
}

decl_enum! {
    /// The container format used to store the texture data.
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    pub enum Container: NvttContainer {
        /// Dds container. This is used to contain data compressed
        /// in the `dxt` format.
        Dds = NvttContainer_NVTT_Container_DDS,
        /// Dds10 container. This is used to contain data compressed
        /// in the `dxt` format.
        Dds10 = NvttContainer_NVTT_Container_DDS10,
        /// Ktx container. This is used to contain data compressed
        /// in the `etc` format.
        Ktx = NvttContainer_NVTT_Container_KTX,
    }
}

impl Container {
    /// Gets the file extension of files used for the container.
    #[inline]
    pub fn file_extension(&self) -> &OsStr {
        match *self {
            Self::Dds | Self::Dds10 => OsStr::new("dds"),
            Self::Ktx => OsStr::new("ktx"),
        }
    }
}

decl_enum! {
    /// Specifies how the alpha should be interpreted on the image.
    ///
    /// You can view [`wikipedia`] for more information about alpha blending.
    ///
    /// [`wikipedia`]: https://en.wikipedia.org/wiki/Alpha_compositing
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    pub enum AlphaMode: NvttAlphaMode {
        /// The image does not contain any alpha information.
        None = NvttAlphaMode_NVTT_AlphaMode_None,
        /// The image uses premultiplied alpha.
        Premultiplied = NvttAlphaMode_NVTT_AlphaMode_Premultiplied,
        /// The image uses straight alpha, where the color channels represent
        /// the straight color of the channel without transparency.
        Transparency = NvttAlphaMode_NVTT_AlphaMode_Transparency,
    }
}

/// Parameters used to customise the kaiser filter used
/// for mipmapping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KaiserParameters {
    pub width: f32,
    pub alpha: f32,
    pub stretch: f32,
}

/// Specify which type of filter used to calculate mipmaps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MipmapFilter {
    /// Use a box filter. This is the default.
    Box,
    /// Use a triangle filter.
    Triangle,
    /// Use a kaiser filter. If the parameters are set, then
    /// they will override the defaults.
    Kaiser(Option<KaiserParameters>),
}

impl From<&'_ MipmapFilter> for NvttMipmapFilter {
    #[inline]
    fn from(filter: &'_ MipmapFilter) -> Self {
        match *filter {
            MipmapFilter::Box => NvttMipmapFilter_NVTT_MipmapFilter_Box,
            MipmapFilter::Triangle => NvttMipmapFilter_NVTT_MipmapFilter_Triangle,
            MipmapFilter::Kaiser(_) => NvttMipmapFilter_NVTT_MipmapFilter_Kaiser,
        }
    }
}

impl From<MipmapFilter> for NvttMipmapFilter {
    #[inline]
    fn from(filter: MipmapFilter) -> Self {
        From::from(&filter)
    }
}

impl From<KaiserParameters> for MipmapFilter {
    #[inline]
    fn from(params: KaiserParameters) -> Self {
        MipmapFilter::Kaiser(Some(params))
    }
}

impl Default for MipmapFilter {
    #[inline]
    fn default() -> Self {
        MipmapFilter::Box
    }
}

decl_enum! {
    /// Specify the quality level of the compression output.
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    pub enum Quality: NvttQuality {
        /// Produces the lowest quality level, but at the fastest speed.
        Fastest = NvttQuality_NVTT_Quality_Fastest,
        /// Produces the highest quality compression output.
        Highest = NvttQuality_NVTT_Quality_Highest,
        /// Provides a medium quality level, trading off between
        /// processing time and output quality.
        ///
        /// This is the default quality.
        Normal = NvttQuality_NVTT_Quality_Normal,
        /// Equivalent to `Quality::Highest`.
        Production = NvttQuality_NVTT_Quality_Production,
    }
}

impl Default for Quality {
    #[inline]
    fn default() -> Self {
        Quality::Normal
    }
}

decl_enum! {
    /// Specify the output format.
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    pub enum Format: NvttFormat {
        Bc1 = NvttFormat_NVTT_Format_BC1,
        Bc1a = NvttFormat_NVTT_Format_BC1a,
        Bc2 = NvttFormat_NVTT_Format_BC2,
        Bc3 = NvttFormat_NVTT_Format_BC3,
        Bc3n = NvttFormat_NVTT_Format_BC3n,
        Bc3_Rgbm = NvttFormat_NVTT_Format_BC3_RGBM,
        Bc4 = NvttFormat_NVTT_Format_BC4,
        Bc5 = NvttFormat_NVTT_Format_BC5,
        Bc6 = NvttFormat_NVTT_Format_BC6,
        Bc7 = NvttFormat_NVTT_Format_BC7,
        Ctx1 = NvttFormat_NVTT_Format_CTX1,
        Dxt1 = NvttFormat_NVTT_Format_DXT1,
        Dxt1a = NvttFormat_NVTT_Format_DXT1a,
        Dxt1n = NvttFormat_NVTT_Format_DXT1n,
        Dxt3 = NvttFormat_NVTT_Format_DXT3,
        Dxt5 = NvttFormat_NVTT_Format_DXT5,
        Dxt5n = NvttFormat_NVTT_Format_DXT5n,
        Etc1 = NvttFormat_NVTT_Format_ETC1,
        Etc2_R = NvttFormat_NVTT_Format_ETC2_R,
        Etc2_Rg = NvttFormat_NVTT_Format_ETC2_RG,
        Etc2_Rgb = NvttFormat_NVTT_Format_ETC2_RGB,
        Etc2_Rgba = NvttFormat_NVTT_Format_ETC2_RGBA,
        Etc2_Rgbm = NvttFormat_NVTT_Format_ETC2_RGBM,
        Etc2_Rgb_A1 = NvttFormat_NVTT_Format_ETC2_RGB_A1,
        Pvr_2Bpp_Rgb = NvttFormat_NVTT_Format_PVR_2BPP_RGB,
        Pvr_2Bpp_Rgba = NvttFormat_NVTT_Format_PVR_2BPP_RGBA,
        Pvr_4Bpp_Rgb = NvttFormat_NVTT_Format_PVR_4BPP_RGB,
        Pvr_4Bpp_Rgba = NvttFormat_NVTT_Format_PVR_4BPP_RGBA,
        Rgb = NvttFormat_NVTT_Format_RGB,
        Rgba = NvttFormat_NVTT_Format_RGBA,
    }
}

decl_enum! {
    /// Specify the color format of the input image.
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    pub enum InputFormat: NvttInputFormat {
        /// 4 unsigned byte channels comprised of `blue`, `green`, `red` and `alpha`.
        Bgra8Ub = NvttInputFormat_NVTT_InputFormat_BGRA_8UB,
        /// 4 16-bit float channels comprised of `red`, `green`, `blue` and `alpha`.
        Rgba16F = NvttInputFormat_NVTT_InputFormat_RGBA_16F,
        /// 4 32-bit float channels comprised of `red`, `green`, `blue` and `alpha`.
        Rgba32F = NvttInputFormat_NVTT_InputFormat_RGBA_32F,
        /// A single 32 bit floating point channel.
        R32F = NvttInputFormat_NVTT_InputFormat_R_32F,
    }
}

decl_enum! {
    /// Controls how the image edge length is rounded when the image is compressed.
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    pub enum RoundMode: NvttRoundMode {
        /// The image size is not changed.
        None = NvttRoundMode_NVTT_RoundMode_None,
        /// Round the size of each edge to the nearest multiple of four.
        ToNearestMultipleOfFour = NvttRoundMode_NVTT_RoundMode_ToNearestMultipleOfFour,
        /// Round the size of each edge to the nearest power of two.
        ToNearestPowerOfTwo = NvttRoundMode_NVTT_RoundMode_ToNearestPowerOfTwo,
        /// Round the size of each edge up to the next highest multiple of four.
        ToNextMultipleOfFour = NvttRoundMode_NVTT_RoundMode_ToNextMultipleOfFour,
        /// Round the size of each edge up to the next highest power of two.
        ToNextPowerOfTwo = NvttRoundMode_NVTT_RoundMode_ToNextPowerOfTwo,
        /// Round the size of each edge down to the next lowest multiple of four.
        ToPreviousMultipleOfFour = NvttRoundMode_NVTT_RoundMode_ToPreviousMultipleOfFour,
        /// Round the size of each edge down to the next lowest power of two.
        ToPreviousPowerOfTwo = NvttRoundMode_NVTT_RoundMode_ToPreviousPowerOfTwo,
    }
}

impl Default for RoundMode {
    #[inline]
    fn default() -> Self {
        RoundMode::None
    }
}

decl_enum! {
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    pub enum TextureType: NvttTextureType {
        D2 = NvttTextureType_NVTT_TextureType_2D,
        Cube = NvttTextureType_NVTT_TextureType_Cube,
        D3 = NvttTextureType_TextureType_3D,
        Array = NvttTextureType_TextureType_Array,
    }
}

impl Default for TextureType {
    #[inline]
    fn default() -> Self {
        TextureType::D2
    }
}

decl_enum! {
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    pub enum WrapMode: NvttWrapMode {
        Clamp = NvttWrapMode_NVTT_WrapMode_Clamp,
        Mirror = NvttWrapMode_NVTT_WrapMode_Mirror,
        Repeat = NvttWrapMode_NVTT_WrapMode_Repeat,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalMapFilter {
    pub small: f32,
    pub medium: f32,
    pub big: f32,
    pub large: f32,
}

impl NormalMapFilter {
    #[inline]
    pub const fn new(small: f32, medium: f32, big: f32, large: f32) -> Self {
        Self {
            small,
            medium,
            big,
            large,
        }
    }
}

/// The `Compressor` is used to perform the texture compression.
#[derive(Debug)]
pub struct Compressor(NonNull<NvttCompressor>);

impl Compressor {
    /// Create a new `Compressor`. If the `Compressor` cannot be created, returns
    /// `Error::Unknown`.
    #[inline]
    pub fn new() -> Result<Self, Error> {
        let compressor = unsafe { nvttCreateCompressor() };
        NonNull::new(compressor).map(Self).ok_or(Error::Unknown)
    }

    /// Returns the underlying `NvttCompressor` pointer type. It is your responsibility
    /// to call `nvttDestroyCompressor` on this value to clean up the `NvttCompressor`
    /// resources.
    #[inline]
    pub fn into_raw(self) -> *mut NvttCompressor {
        let ptr = self.0.as_ptr();
        mem::forget(self);
        ptr
    }

    /// If the platform supports the `cuda` api, this method can be used to enable
    /// gpu compression. This may give different results to a pure cpu implementation,
    /// so this is set to `false` by default.
    ///
    /// On platforms without `cuda`, this function is a no-op.
    #[inline]
    pub fn enable_cuda_acceleration<B: Into<NvttBoolean>>(&mut self, enable: B) -> &mut Self {
        unsafe {
            nvttEnableCudaAcceleration(self.0.as_ptr(), enable.into());
        }
        self
    }

    /// Returns `true` if cuda acceleration has been enabled. Otherwise, returns
    /// false.
    #[inline]
    pub fn is_cuda_acceleration_enabled(&self) -> bool {
        unsafe { nvttIsCudaAccelerationEnabled(self.0.as_ptr()).into() }
    }

    /// Perform the compression.
    pub fn compress(
        &self,
        compress_options: &CompressionOptions,
        input_options: &InputOptions,
        output_options: &OutputOptions,
    ) -> Result<CompressionOutput, Error> {
        thread_local! {
            static ERR: Cell<NvttError> = Cell::new(0);
            static OUT_DATA: RefCell<Vec<u8>> = RefCell::new(vec![]);
            static HEIGHT: Cell<usize> = Cell::new(0);
            static WIDTH: Cell<usize> = Cell::new(0);
            static DEPTH: Cell<usize> = Cell::new(0);
            static FACE: Cell<usize> = Cell::new(0);
            static MIPLEVEL: Cell<usize> = Cell::new(0);
        }

        extern "C" fn err_callback(err: NvttError) {
            error!(
                "nvtt: Encountered an error while compressing: {}",
                Error::try_from(err).unwrap_or(Error::Unknown)
            );
            ERR.with(|e| e.set(err));
        }

        extern "C" fn output_begin_callback(
            size: c_int,
            width: c_int,
            height: c_int,
            depth: c_int,
            face: c_int,
            miplevel: c_int,
        ) {
            trace!("Beginning texture compression with image size {} ({} x {} x {}), face = {}, mip = {}",
                size, width, height, depth, face, miplevel);

            OUT_DATA.with(|d| d.borrow_mut().reserve(size as _));

            WIDTH.with(|w| w.set(width as _));
            HEIGHT.with(|h| h.set(height as _));
            DEPTH.with(|d| d.set(depth as _));
            FACE.with(|f| f.set(face as _));
            MIPLEVEL.with(|ml| ml.set(miplevel as _));
        }

        extern "C" fn output_callback(data_ptr: *const c_void, len: c_int) -> bool {
            let len = match usize::try_from(len) {
                Ok(len) => len,
                Err(_) => {
                    error!("Could not append texture data: len {} is invalid", len);
                    return false;
                }
            };

            let data = unsafe { slice::from_raw_parts(data_ptr as *const u8, len) };
            OUT_DATA.with(|d| d.borrow_mut().extend_from_slice(data));
            true
        }

        OUT_DATA.with(|d| d.borrow_mut().clear());

        let res = unsafe {
            let out_opts_ptr = output_options.out_opts.as_ptr();

            nvttSetOutputOptionsErrorHandler(out_opts_ptr, Some(err_callback));

            if !output_options.write_to_file {
                nvttSetOutputOptionsOutputHandler(
                    out_opts_ptr,
                    Some(output_begin_callback), // begin image
                    Some(output_callback),
                    None, // end image
                );
            }

            nvttCompress(
                self.0.as_ptr(),
                input_options.0.as_ptr(),
                compress_options.0.as_ptr(),
                output_options.out_opts.as_ptr(),
            )
        };

        if res != NvttBoolean::NVTT_True {
            let mut err = 0;
            ERR.with(|e| err = e.get());
            Err(Error::try_from(err).unwrap_or(Error::Unknown))
        } else {
            if !output_options.write_to_file {
                Ok(CompressionOutput::Memory {
                    data: OUT_DATA.with(|d| d.replace(vec![])),
                    width: WIDTH.with(|w| w.get()),
                    height: HEIGHT.with(|h| h.get()),
                    depth: DEPTH.with(|d| d.get()),
                    face: FACE.with(|f| f.get()),
                    miplevel: MIPLEVEL.with(|ml| ml.get()),
                })
            } else {
                Ok(CompressionOutput::File)
            }
        }
    }

    /// Estimate the final compressed size of the output texture.
    #[inline]
    pub fn estimate_size(
        &self,
        input_options: &InputOptions,
        compression_options: &CompressionOptions,
    ) -> usize {
        unsafe {
            nvttEstimateSize(
                self.0.as_ptr(),
                input_options.0.as_ptr(),
                compression_options.0.as_ptr(),
            ) as usize
        }
    }
}

impl Drop for Compressor {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            nvttDestroyCompressor(self.0.as_ptr());
        }
    }
}

// @SAFETY: A `Compressor` cannot be copied or unsafely mutated in a shared way.
unsafe impl Send for Compressor {}

/// Communicates the output of a compressed texture.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CompressionOutput {
    /// The texture was saved into the file specified on the `OutputOptions`.
    File,
    /// The texture was saved into memory.
    Memory {
        /// The bytes of the image.
        data: Vec<u8>,
        /// The width of the texture in pixels.
        width: usize,
        /// The height of the texture in pixels.
        height: usize,
        /// The depth of the texture.
        depth: usize,
        /// The face of the texture.
        face: usize,
        /// The mipmap level of the texture.
        miplevel: usize,
    },
}

/// Object which stores the compression options for the texture.
#[derive(Debug)]
pub struct CompressionOptions(NonNull<NvttCompressionOptions>);

impl CompressionOptions {
    /// Create a new `CompressionOptions`.
    #[inline]
    pub fn new() -> Result<Self, Error> {
        let opts = unsafe { nvttCreateCompressionOptions() };
        NonNull::new(opts).map(Self).ok_or(Error::Unknown)
    }

    #[inline]
    pub fn into_raw(self) -> *mut NvttCompressionOptions {
        let ptr = self.0.as_ptr();
        mem::forget(self);
        ptr
    }

    #[inline]
    pub fn set_color_weights(&mut self, r: f32, g: f32, b: f32, a: f32) -> &mut Self {
        unsafe {
            nvttSetCompressionOptionsColorWeights(self.0.as_ptr(), r, g, b, a);
        }
        self
    }

    #[inline]
    pub fn set_format(&mut self, format: Format) -> &mut Self {
        unsafe {
            nvttSetCompressionOptionsFormat(self.0.as_ptr(), format.into());
        }
        self
    }

    #[inline]
    pub fn set_pixel_format(
        &mut self,
        bitcount: c_uint,
        rmask: c_uint,
        gmask: c_uint,
        bmask: c_uint,
        amask: c_uint,
    ) -> &mut Self {
        unsafe {
            nvttSetCompressionOptionsPixelFormat(
                self.0.as_ptr(),
                bitcount,
                rmask,
                gmask,
                bmask,
                amask,
            )
        }
        self
    }

    #[inline]
    pub fn set_quality(&mut self, quality: Quality) -> &mut Self {
        unsafe {
            nvttSetCompressionOptionsQuality(self.0.as_ptr(), quality.into());
        }
        self
    }

    #[inline]
    pub fn set_quanitzation(
        &mut self,
        color_dithering: impl Into<NvttBoolean>,
        alpha_dithering: impl Into<NvttBoolean>,
        binary_alpha: impl Into<NvttBoolean>,
        alpha_threshold: i32,
    ) -> &mut Self {
        unsafe {
            nvttSetCompressionOptionsQuantization(
                self.0.as_ptr(),
                color_dithering.into(),
                alpha_dithering.into(),
                binary_alpha.into(),
                alpha_threshold,
            )
        }
        self
    }
}

impl Drop for CompressionOptions {
    #[inline]
    fn drop(&mut self) {
        unsafe { nvttDestroyCompressionOptions(self.0.as_ptr()) }
    }
}

// @SAFETY: A `CompressionOptions` cannot be copied or unsafely mutated in a shared way.
unsafe impl Send for CompressionOptions {}

/// Object which stores the input options for the texture.
#[derive(Debug)]
pub struct InputOptions(NonNull<NvttInputOptions>);

impl InputOptions {
    /// Create a new `InputOptions`.
    #[inline]
    pub fn new() -> Result<Self, Error> {
        let opts = unsafe { nvttCreateInputOptions() };
        NonNull::new(opts).map(Self).ok_or(Error::Unknown)
    }

    /// Returns the underlying `NvttInputOptions` pointer type. It is your responsibility
    /// to call `nvttDestroyInputOptions` on this value to clean up the `NvttInputOptions`
    /// resources.
    #[inline]
    pub fn into_raw(self) -> *mut NvttInputOptions {
        let ptr = self.0.as_ptr();
        mem::forget(self);
        ptr
    }

    /// Set the `AlphaMode` on the `InputOptions`.
    #[inline]
    pub fn set_alpha_mode(&mut self, alpha_mode: AlphaMode) -> &mut Self {
        unsafe {
            nvttSetInputOptionsAlphaMode(self.0.as_ptr(), alpha_mode.into());
        }
        self
    }

    /// If this parameter is set, then `nvtt` will convert the image into a normal map.
    #[inline]
    pub fn convert_to_normal_map(
        &mut self,
        convert_to_normal_map: impl Into<NvttBoolean>,
    ) -> &mut Self {
        unsafe {
            nvttSetInputOptionsConvertToNormalMap(self.0.as_ptr(), convert_to_normal_map.into());
        }
        self
    }

    /// Sets the `InputFormat` of the input data. This tells `nvtt` which pixel format
    /// it should interpret the byte data passed in [`InputOptions::set_mipmap_data`] as.
    ///
    /// [`InputOptions::set_mipmap_data`]: struct.InputOptions.html#method.set_mipmap_data
    #[inline]
    pub fn set_format(&mut self, format: InputFormat) -> &mut Self {
        unsafe {
            nvttSetInputOptionsFormat(self.0.as_ptr(), format.into());
        }
        self
    }

    /// Set the `input_gamma` and `output_gamma` on the `InputOptions`.
    #[inline]
    pub fn set_gamma(&mut self, input_gamma: f32, output_gamma: f32) -> &mut Self {
        unsafe {
            nvttSetInputOptionsGamma(self.0.as_ptr(), input_gamma, output_gamma);
        }
        self
    }

    #[inline]
    pub fn set_height_evaluation(
        &mut self,
        red_scale: f32,
        green_scale: f32,
        blue_scale: f32,
        alpha_scale: f32,
    ) -> &mut Self {
        unsafe {
            nvttSetInputOptionsHeightEvaluation(
                self.0.as_ptr(),
                red_scale,
                green_scale,
                blue_scale,
                alpha_scale,
            );
        }
        self
    }

    /// Set the `MipmapFilter` on the `InputOptions`. See the [`MipmapFilter`]
    /// type for more info.
    ///
    /// [`MipmapFilter`]: enum.MipmapFilter.html
    #[inline]
    pub fn set_mipmap_filter(&mut self, mipmap_filter: MipmapFilter) -> &mut Self {
        let opts_ptr = self.0.as_ptr();
        unsafe {
            nvttSetInputOptionsMipmapFilter(opts_ptr, mipmap_filter.into());
        }

        if let MipmapFilter::Kaiser(Some(KaiserParameters {
            width,
            alpha,
            stretch,
        })) = mipmap_filter
        {
            unsafe {
                nvttSetInputOptionsKaiserParameters(opts_ptr, width, alpha, stretch);
            }
        }

        self
    }

    /// Sets the input data which should be compressed.
    ///
    /// The `data` is copied into the `InputOptions` object.
    ///
    /// # Errors
    ///
    /// If the dimensions of the image do not match the length of the `data`,
    /// then this method will fail with [`Error::Unknown`].
    ///
    /// [`Error::Unknown`]: enum.Error.html#variant.Unknown
    #[inline]
    pub fn set_mipmap_data(
        &mut self,
        data: &[u8],
        w: i32,
        h: i32,
        d: i32,
        face: i32,
        mipmap: i32,
    ) -> Result<&mut Self, Error> {
        let result = unsafe {
            nvttSetInputOptionsMipmapData(
                self.0.as_ptr(),
                data.as_ptr() as *const _,
                w,
                h,
                d,
                face,
                mipmap,
            )
        };

        match result {
            NvttBoolean::NVTT_True => Ok(self),
            NvttBoolean::NVTT_False => Err(Error::Unknown),
        }
    }

    /// Resets the `InputOptions` back to the default state.
    #[inline]
    pub fn reset(&mut self) -> &mut Self {
        unsafe { nvttResetInputOptionsTextureLayout(self.0.as_ptr()) }
        self
    }

    /// Sets the input texture data to the data contained by `image`.
    ///
    /// # Notes
    ///
    /// This method requires the [`nvtt_image_integration`] feature.
    ///
    /// [`nvtt_image_integration`]: index.html#nvtt_image_integration
    #[cfg(feature = "nvtt_image_integration")]
    #[inline]
    pub fn set_image<'a, I: Into<ValidImage<'a>>>(
        &mut self,
        image: I,
        face: i32,
        mipmap: i32,
    ) -> Result<&mut Self, Error> {
        let image = image.into();
        let (w, h) = image.image_dimensions();

        self.reset()
            .set_format(image.format())
            .set_texture_layout(TextureType::D2, w as _, h as _, 1, 1)
            .set_mipmap_data(image.data_bytes(), w as _, h as _, 1, face, mipmap)?;

        Ok(self)
    }

    /// Constrain the texture size to the value in `max_extents`.
    #[inline]
    pub fn set_max_extents(&mut self, max_extents: c_int) -> &mut Self {
        unsafe {
            nvttSetInputOptionsMaxExtents(self.0.as_ptr(), max_extents);
        }
        self
    }

    /// Specify whether the image is a normal map. Normal maps may be compressed
    /// differently to better preserve the normal information.
    #[inline]
    pub fn set_normal_map(&mut self, is_normal_map: impl Into<NvttBoolean>) -> &mut Self {
        unsafe {
            nvttSetInputOptionsNormalMap(self.0.as_ptr(), is_normal_map.into());
        }
        self
    }

    #[inline]
    pub fn set_normalize_mipmaps(&mut self, normalize_mips: impl Into<NvttBoolean>) -> &mut Self {
        unsafe {
            nvttSetInputOptionsNormalizeMipmaps(self.0.as_ptr(), normalize_mips.into());
        }
        self
    }

    pub fn set_normal_filter(&mut self, filter: NormalMapFilter) -> &mut Self {
        unsafe {
            nvttSetInputOptionsNormalFilter(
                self.0.as_ptr(),
                filter.small,
                filter.medium,
                filter.big,
                filter.large,
            );
        }

        self
    }

    /// Set the `RoundMode` on the `InputOptions`.
    #[inline]
    pub fn set_round_mode(&mut self, round_mode: RoundMode) -> &mut Self {
        unsafe {
            nvttSetInputOptionsRoundMode(self.0.as_ptr(), round_mode.into());
        }
        self
    }

    #[inline]
    pub fn set_texture_layout(
        &mut self,
        texture_type: TextureType,
        w: c_int,
        h: c_int,
        d: c_int,
        array_size: c_int,
    ) -> &mut Self {
        unsafe {
            nvttSetInputOptionsTextureLayout(
                self.0.as_ptr(),
                texture_type.into(),
                w,
                h,
                d,
                array_size,
            )
        }
        self
    }

    /// Set the `WrapMode` on the `InputOptions`.
    #[inline]
    pub fn set_wrap_mode(&mut self, wrap_mode: WrapMode) -> &mut Self {
        unsafe {
            nvttSetInputOptionsWrapMode(self.0.as_ptr(), wrap_mode.into());
        }
        self
    }
}

impl Drop for InputOptions {
    #[inline]
    fn drop(&mut self) {
        unsafe { nvttDestroyInputOptions(self.0.as_ptr()) }
    }
}

// @SAFETY: An `InputOptions` cannot be copied or unsafely mutated in a shared way.
unsafe impl Send for InputOptions {}

cfg_if! {
    if #[cfg(feature = "nvtt_image_integration")] {
        /// An enumeration of the valid image buffer types which can be
        /// used with nvtt.
        ///
        /// # Notes
        ///
        /// This type requires the [`nvtt_image_integration`] feature.
        ///
        /// [`nvtt_image_integration`]: index.html#nvtt_image_integration
        #[derive(Clone, Debug)]
        pub enum ValidImage<'a> {
            /// A bgra image with byte values for each subpixel.
            Bgra(MaybeOwned<'a, ImageBuffer<Bgra<u8>, Vec<u8>>>),
            /// An rgba image with floats for each subpixel.
            Rgba(MaybeOwned<'a, ImageBuffer<Rgba<f32>, Vec<f32>>>),
            /// A luma image with floats for each subpixel.
            Luma(MaybeOwned<'a, ImageBuffer<Luma<f32>, Vec<f32>>>),
        }

        impl ValidImage<'_> {
            /// Create a new `ValidImage` from `image`.
            #[inline]
            pub fn new<I: Into<Self>>(image: I) -> Self {
                image.into()
            }

            #[inline]
            fn format(&self) -> InputFormat {
                match *self {
                    ValidImage::Bgra(_) => InputFormat::Bgra8Ub,
                    ValidImage::Rgba(_) => InputFormat::Rgba32F,
                    ValidImage::Luma(_) => InputFormat::R32F,
                }
            }

            #[inline]
            fn image_dimensions(&self) -> (u32, u32) {
                match *self {
                    ValidImage::Bgra(ref i) => i.dimensions(),
                    ValidImage::Rgba(ref i) => i.dimensions(),
                    ValidImage::Luma(ref i) => i.dimensions(),
                }
            }

            #[inline]
            fn data_bytes(&self) -> &[u8] {
                match *self {
                    ValidImage::Bgra(ref i) => i.deref(),
                    ValidImage::Rgba(ref i) => transmute_to_bytes(i.deref()),
                    ValidImage::Luma(ref i) => transmute_to_bytes(i.deref()),
                }
            }
        }

        impl From<DynamicImage> for ValidImage<'_> {
            #[inline]
            fn from(img: DynamicImage) -> Self {
                ValidImage::Bgra(MaybeOwned::Owned(img.to_bgra()))
            }
        }

        impl From<&'_ DynamicImage> for ValidImage<'_> {
            #[inline]
            fn from(img: &'_ DynamicImage) -> Self {
                ValidImage::Bgra(MaybeOwned::Owned(img.to_bgra()))
            }
        }

        macro_rules! impl_maybeowned_from {
            ($( ($pix:ident, $subpix:ident) ),+ $(,)?) => {
                $(
                    impl From<ImageBuffer<$pix<$subpix>, Vec<$subpix>>> for ValidImage<'_> {
                        #[inline]
                        fn from(buf: ImageBuffer<$pix<$subpix>, Vec<$subpix>>) -> Self {
                            ValidImage:: $pix (MaybeOwned::from(buf))
                        }
                    }

                    impl<'a> From<&'a ImageBuffer<$pix<$subpix>, Vec<$subpix>>> for ValidImage<'a> {
                        #[inline]
                        fn from(buf: &'a ImageBuffer<$pix<$subpix>, Vec<$subpix>>) -> Self {
                            ValidImage:: $pix (MaybeOwned::from(buf))
                        }
                    }

                    impl<'a> From<MaybeOwned<'a, ImageBuffer<$pix<$subpix>, Vec<$subpix>>>> for ValidImage<'a> {
                        #[inline]
                        fn from(img: MaybeOwned<'a, ImageBuffer<$pix<$subpix>, Vec<$subpix>>>) -> Self {
                            ValidImage::$pix(img)
                        }
                    }
                )*
            };
        }

        impl_maybeowned_from! {
            (Bgra, u8), (Rgba, f32), (Luma, f32),
        }
    }
}

/// Object which stores the output options for the texture.
#[derive(Debug)]
pub struct OutputOptions {
    out_opts: NonNull<NvttOutputOptions>,
    write_to_file: bool,
}

impl OutputOptions {
    /// Create a new `OutputOptions`.
    #[inline]
    pub fn new() -> Result<Self, Error> {
        let opts = unsafe { nvttCreateOutputOptions() };
        let out_opts = NonNull::new(opts).ok_or(Error::Unknown)?;
        Ok(OutputOptions {
            out_opts,
            write_to_file: true,
        })
    }

    /// Returns the underlying `NvttOutputOptions` pointer type. It is your responsibility
    /// to call `nvttDestroyOutputOptions` on this value to clean up the `NvttOutputOptions`
    /// resources.
    #[inline]
    pub fn into_raw(self) -> *mut NvttOutputOptions {
        let ptr = self.out_opts.as_ptr();
        mem::forget(self);
        ptr
    }

    /// Set the output location. This can be either a path or an in-memory
    /// buffer. For more information, see the [`OutputLocation`] type.
    ///
    /// # Notes
    ///
    /// `nvtt` only supports ASCII filenames on Windows. If you need to support
    /// non-ASCII filenames, you will need to pass [`OutputLocation::Buffer`],
    /// and then write the data into the file using another method. An example of
    /// to do this is shown below.
    ///
    /// ## Example
    ///
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use nvtt_rs::{
    ///     CompressionOptions, CompressionOutput, Compressor,
    ///     InputOptions, OutputLocation, OutputOptions,
    /// };
    ///
    /// let compressor = Compressor::new()?;
    /// let input_opts = InputOptions::new()?;
    /// let compression_opts = CompressionOptions::new()?;
    /// let mut output_opts = OutputOptions::new()?;
    ///
    /// output_opts.set_output_location(OutputLocation::Buffer);
    ///
    /// // Set other texture options here...
    ///
    /// match compressor.compress(&compression_opts, &input_opts, &output_opts)? {
    ///     CompressionOutput::Memory { data, .. } => {
    ///         std::fs::write("OutFile.dds", &data[..])?;
    ///     }
    ///     _ => {}
    /// };
    ///
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`OutputLocation`]: enum.OutputLocation.html
    /// [`OutputLocation::Buffer`]: enum.OutputLocation.html#variant.Buffer
    #[inline]
    pub fn set_output_location<'a, T: 'a + ?Sized + Into<OutputLocation<'a>>>(
        &mut self,
        out_location: T,
    ) -> Result<&mut Self, PathConvertError> {
        #[inline(never)]
        fn inner(opts: &mut OutputOptions, loc: OutputLocation<'_>) -> Result<(), PathConvertError> {
            match loc {
                OutputLocation::File(p) => {
                    #[inline(always)]
                    fn to_c_filepath(path: &Path) -> Result<CString, PathConvertError> {
                        cfg_if! {
                            if #[cfg(target_family = "windows")] {
                                match path.to_str() {
                                    Some(s) => {
                                        if !s.is_ascii() {
                                            return Err(PathConvertError::AsciiConvert)
                                        }
                                        CString::new(s.as_bytes()).map_err(From::from)
                                    }
                                    None => Err(PathConvertError::Utf8Convert),
                                }
                            } else if #[cfg(target_family = "unix")] {
                                use std::os::unix::ffi::OsStrExt;
                                CString::new(path.as_os_str().as_bytes()).map_err(From::from)
                            } else {
                                compile_error!("This platform is unsupported");
                            }
                        }
                    }

                    let out_file = to_c_filepath(p)?;
                    unsafe {
                        nvttSetOutputOptionsFileName(opts.out_opts.as_ptr(), out_file.as_ptr());
                    }
                    opts.write_to_file = true;
                    Ok(())
                }
                OutputLocation::Buffer => {
                    opts.write_to_file = false;
                    Ok(())
                }
            }
        }

        inner(self, out_location.into())?;
        Ok(self)
    }

    #[inline]
    pub fn set_write_header<B: Into<NvttBoolean>>(&mut self, write_header: B) -> &mut Self {
        unsafe {
            nvttSetOutputOptionsOutputHeader(self.out_opts.as_ptr(), write_header.into());
        }
        self
    }

    #[inline]
    pub fn set_srgb_flag<B: Into<NvttBoolean>>(&mut self, write_srgb: B) -> &mut Self {
        unsafe {
            nvttSetOutputOptionsSrgbFlag(self.out_opts.as_ptr(), write_srgb.into());
        }
        self
    }

    #[inline]
    pub fn set_container(&mut self, container: Container) -> &mut Self {
        unsafe {
            nvttSetOutputOptionsContainer(self.out_opts.as_ptr(), container.into());
        }
        self
    }
}

impl Drop for OutputOptions {
    #[inline]
    fn drop(&mut self) {
        unsafe { nvttDestroyOutputOptions(self.out_opts.as_ptr()) }
    }
}

// @SAFETY: An `OutputOptions` cannot be copied or unsafely mutated in a shared way.
unsafe impl Send for OutputOptions {}

/// This enum is used to define the output location of the compressed
/// texture data.
///
/// See the method [`OutputOptions::set_output_location`] for more information.
///
/// [`OutputOptions::set_output_location`]: struct.OutputOptions.html#method.set_output_location
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum OutputLocation<'a> {
    /// Output the texture to the file specified by the `Path`.
    File(&'a Path),
    /// Output the texture into an in-memory buffer. This will be returned
    /// by [`Compressor::compress`].
    ///
    /// [`Compressor::compress`]: struct.Compressor.html#method.compress
    Buffer,
}

impl<'a, P: 'a + ?Sized + AsRef<Path>> From<&'a P> for OutputLocation<'a> {
    #[inline]
    fn from(p: &'a P) -> Self {
        OutputLocation::File(p.as_ref())
    }
}

decl_enum! {
    /// An error which may occur during compression.
    #[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
    pub enum Error: NvttError {
        /// An error occurred while running a CUDA kernel.
        CudaError = NvttError_NVTT_Error_CudaError,
        /// An error occurred while opening a file.
        FileOpen = NvttError_NVTT_Error_FileOpen,
        /// An error occurred while writing a file.
        FileWrite = NvttError_NVTT_Error_FileWrite,
        /// The input was invalid.
        InvalidInput = NvttError_NVTT_Error_InvalidInput,
        /// An unknown error occurred.
        Unknown = NvttError_NVTT_Error_Unknown,
        /// The requested feature is not supported.
        UnsupportedFeature = NvttError_NVTT_Error_UnsupportedFeature,
        /// The requested output format is not supported.
        UnsupportedOutputFormat = NvttError_NVTT_Error_UnsupportedOutputFormat,
    }
}

impl Default for Error {
    #[inline]
    fn default() -> Self {
        Error::Unknown
    }
}

impl fmt::Display for Error {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = unsafe { CStr::from_ptr(nvttErrorString(self.into())) };
        f.write_str(&s.to_string_lossy())?;
        Ok(())
    }
}

impl ErrorTrait for Error {
    #[inline]
    fn description(&self) -> &'static str {
        let s = unsafe { CStr::from_ptr(nvttErrorString(self.into())) };
        s.to_str().unwrap_or("An unknown error occurred")
    }
}

/// An error type for when a path could not be converted.
#[derive(Clone, Debug)]
pub enum PathConvertError {
    /// An error occurred while converting the path into utf8.
    Utf8Convert,
    /// An error occurred while converting the path into ASCII.
    AsciiConvert,
    /// The path contained a nul byte and could not be converted
    /// into a C compatible string.
    Nul(NulError),
}

impl fmt::Display for PathConvertError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            PathConvertError::Utf8Convert => {
                f.write_str("Could not convert path losslessly to UTF8 string")
            }
            PathConvertError::AsciiConvert => f.write_str("The given path was not valid ASCII"),
            PathConvertError::Nul(ref e) => fmt::Display::fmt(e, f),
        }
    }
}

impl ErrorTrait for PathConvertError {
    #[inline]
    fn source(&self) -> Option<&(dyn ErrorTrait + 'static)> {
        match *self {
            PathConvertError::Nul(ref e) => Some(e),
            _ => None,
        }
    }
}

impl From<NulError> for PathConvertError {
    #[inline]
    fn from(e: NulError) -> Self {
        PathConvertError::Nul(e)
    }
}
