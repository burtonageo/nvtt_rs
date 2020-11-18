// Copyright © 2019-2020 George Burton
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

#![allow(unused)]

use bindgen;
use cfg_if::cfg_if;
use std::{env, error::Error, path::PathBuf};

#[inline(always)]
fn e(
    val: impl Into<Box<dyn Error + Send + Sync + 'static>>,
) -> Box<dyn Error + Send + Sync + 'static> {
    val.into()
}

cfg_if! {
    if #[cfg(target_os = "windows")] {
        use semver::Version;
        use std::process::Command;
        use vswhere::{Config, FourPointVersion, InstallInfo};

        fn build_nvtt() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
            // let min_version = FourPoint
            let vs_path = Config::new()
                .run_default_path()?
                .iter()
                .filter(|&info| {
                    let maj = info.installation_version().major();
                    maj >= 2013 && maj <= 2017
                })
                .nth(0)
                .map(InstallInfo::installation_path)
                .map(PathBuf::from)
                .ok_or_else(|| e("Could not find Visual Studio installation info"))?;

            Ok(())
        }
    } else {
        use cmake;
        fn build_nvtt() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
            let dst = cmake::build("./nvidia-texture-tools");

            println!("cargo:rustc-link-search={}", dst.join("lib").join("static").display());

            // @TODO(burtonageo): Is this necessary???
            let src_dir = dst.join("build").join("src");
            let extern_dir = dst.join("build").join("extern");

            println!("cargo:rustc-link-search={}", src_dir.join("bc7").display());
            println!("cargo:rustc-link-search={}", src_dir.join("bc6h").display());
            println!("cargo:rustc-link-search={}", src_dir.join("nvtt").join("squish").display());
            println!("cargo:rustc-link-search={}", extern_dir.join("rg_etc1_v104").display());

            let libs = &[
                "nvcore",
                "nvimage",
                "nvmath",
                "nvthread",
                "nvtt",

                // @TODO(burtonageo): Is this necessary???
                "bc7",
                "bc6h",
                "squish",
                "rg_etc1",
            ];

            for lib in &libs[..] {
                println!("cargo:rustc-link-lib=static={}", lib);
            }

            // Need to link to the c++ stdlib
            if cfg!(target_os = "macos") {
                println!("cargo:rustc-link-lib=dylib=c++");
            } else {
                println!("cargo:rustc-link-lib=dylib=stdc++");
            }

            Ok(())
        }
    }
}

fn main() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    println!("cargo:rerun-if-changed=./nvidia-texture-tools");
    println!("cargo:rerun-if-changed=./wrapper.h");

    if !cfg!(target_os = "windows") {
        build_nvtt()?;
    }

    let bindings = bindgen::builder()
        .header("./wrapper.h")
        .ctypes_prefix("libc")
        .rustified_enum("NvttBoolean")
        .use_core()
        .generate()
        .map_err(|_| e("Could not generate bindings"))?;

    let out_path = PathBuf::from(env::var("OUT_DIR")?);
    bindings.write_to_file(out_path.join("nvtt_bindings.rs"))?;

    Ok(())
}
