pub mod map;
pub mod map_with_dict;
pub mod map_with_dict_bitpacked;
pub mod mphf;
pub mod rank;
pub mod set;

pub use map::*;
pub use map_with_dict::*;
pub use map_with_dict_bitpacked::*;
pub use mphf::*;
pub use rank::*;
pub use set::*;

pub trait IntoGroupSeed: Copy {
    fn into_u32(self) -> u32;
}

impl IntoGroupSeed for u8 {
    #[inline(always)]
    fn into_u32(self) -> u32 {
        self as u32
    }
}

impl IntoGroupSeed for u16 {
    #[inline(always)]
    fn into_u32(self) -> u32 {
        self as u32
    }
}

#[cfg(feature = "rkyv_derive")]
impl IntoGroupSeed for rkyv::rend::u16_le {
    #[inline(always)]
    fn into_u32(self) -> u32 {
        self.to_native() as u32
    }
}

#[cfg(feature = "rkyv_derive")]
impl IntoGroupSeed for rkyv::rend::u16_be {
    #[inline(always)]
    fn into_u32(self) -> u32 {
        self.to_native() as u32
    }
}

impl IntoGroupSeed for u32 {
    #[inline(always)]
    fn into_u32(self) -> u32 {
        self
    }
}

#[cfg(feature = "rkyv_derive")]
impl IntoGroupSeed for rkyv::rend::u32_le {
    #[inline(always)]
    fn into_u32(self) -> u32 {
        self.to_native()
    }
}

#[cfg(feature = "rkyv_derive")]
impl IntoGroupSeed for rkyv::rend::u32_be {
    #[inline(always)]
    fn into_u32(self) -> u32 {
        self.to_native()
    }
}

pub trait IntoRankBits: Copy {
    fn into_u64(self) -> u64;
}

impl IntoRankBits for u64 {
    #[inline(always)]
    fn into_u64(self) -> u64 {
        self
    }
}

#[cfg(feature = "rkyv_derive")]
impl IntoRankBits for rkyv::rend::u64_le {
    #[inline(always)]
    fn into_u64(self) -> u64 {
        self.to_native()
    }
}

#[cfg(feature = "rkyv_derive")]
impl IntoRankBits for rkyv::rend::u64_be {
    #[inline(always)]
    fn into_u64(self) -> u64 {
        self.to_native()
    }
}
