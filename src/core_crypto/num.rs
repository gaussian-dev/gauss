use std::fmt::{Debug, Display};

use num_traits::{
    AsPrimitive, CheckedShl, CheckedShr, Num, NumAssign, PrimInt, ToPrimitive, WrappingAdd,
    WrappingMul, WrappingShl, WrappingShr, WrappingSub,
};

pub trait UnsignedIntegerDoubled {}

pub trait NumericConstants {
    const BITS: u32;
    const MAX: Self;
}

pub trait UnsignedInteger:
    PrimInt
    + Num
    + NumAssign
    + WrappingMul
    + WrappingAdd
    + WrappingSub
    + WrappingShl
    + CheckedShl
    + WrappingShr
    + CheckedShr
    + PartialOrd
    + Display
    + NumericConstants
    + Debug
    + Copy
    + Clone
{
}

impl NumericConstants for u64 {
    const BITS: u32 = u64::BITS;
    const MAX: u64 = u64::MAX;
}

impl NumericConstants for u128 {
    const BITS: u32 = u128::BITS;
    const MAX: u128 = u128::MAX;
}

impl UnsignedInteger for u64 {}
impl UnsignedInteger for u128 {}
