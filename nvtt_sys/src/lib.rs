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

#![allow(nonstandard_style)]
#![no_std]

use core::{cmp::PartialEq, ops::Not};
use libc;

include!(concat!(env!("OUT_DIR"), "/nvtt_bindings.rs"));

impl PartialEq<bool> for NvttBoolean {
    #[inline]
    fn eq(&self, rhs: &bool) -> bool {
        bool::from(*self) == *rhs
    }
}

impl PartialEq<NvttBoolean> for bool {
    #[inline]
    fn eq(&self, rhs: &NvttBoolean) -> bool {
        PartialEq::eq(rhs, self)
    }
}

impl From<bool> for NvttBoolean {
    #[inline]
    fn from(b: bool) -> Self {
        if b {
            NvttBoolean::NVTT_True
        } else {
            NvttBoolean::NVTT_False
        }
    }
}

impl From<NvttBoolean> for bool {
    #[inline]
    fn from(b: NvttBoolean) -> Self {
        b == NvttBoolean::NVTT_True
    }
}

impl Not for NvttBoolean {
    type Output = Self;
    #[inline]
    fn not(self) -> Self::Output {
        match self {
            NvttBoolean::NVTT_False => NvttBoolean::NVTT_True,
            NvttBoolean::NVTT_True => NvttBoolean::NVTT_False,
        }
    }
}
