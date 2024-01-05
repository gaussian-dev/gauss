use crate::{core_crypto::num::UnsignedInteger, utils::FastModularInverse};
use itertools::izip;
use num_traits::AsPrimitive;

/// Wrapper around `MontgomeryScalar`s.
///
/// This is just to prevent cross-arithmatic between Scalars in Montgomery space and Scalars in Normal space.
#[derive(Clone, Copy)]
pub struct MontgomeryScalar<Scalar: UnsignedInteger>(pub(crate) Scalar);

impl<Scalar: UnsignedInteger> MontgomeryScalar<Scalar> {
    pub fn zero() -> Self {
        MontgomeryScalar(Scalar::zero())
    }
}

impl<Scalar: UnsignedInteger> std::fmt::Display for MontgomeryScalar<Scalar> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MontgomeryScalar({})", self.0)?;
        Ok(())
    }
}

pub trait MontgomeryBackendConfig<Scalar, ScalarDoubled>
where
    Scalar: UnsignedInteger + AsPrimitive<ScalarDoubled> + FastModularInverse + 'static,
    ScalarDoubled: UnsignedInteger + AsPrimitive<Scalar> + 'static,
{
    fn initialise(modulus: Scalar) -> (Scalar, Scalar) {
        let n_inv_modr = Scalar::fast_inverse(modulus);

        // r^2 (mod n) where `r` is either 2^{32/64/128}
        let r_modn = (ScalarDoubled::one() << (Scalar::BITS as usize)) % modulus.as_();
        let r_square_modn = (r_modn * r_modn) % modulus.as_();
        (n_inv_modr, r_square_modn.as_())
    }
}

pub trait MontgomeryBackend<Scalar, ScalarDoubled>
where
    Scalar: UnsignedInteger + AsPrimitive<ScalarDoubled> + 'static,
    ScalarDoubled: UnsignedInteger + AsPrimitive<Scalar> + 'static,
{
    fn modulus(&self) -> Scalar;

    fn modulus_twice(&self) -> Scalar;

    /// n^{-1} (mod r) where n is the original modulus
    fn n_inverse_modr(&self) -> Scalar;

    /// r^2 (mod n)
    fn r_square_modn(&self) -> Scalar;

    fn mont_sub(
        &self,
        a: MontgomeryScalar<Scalar>,
        b: MontgomeryScalar<Scalar>,
    ) -> MontgomeryScalar<Scalar> {
        debug_assert!(a.0 < self.modulus(), "Input {a} >= {}", self.modulus());
        debug_assert!(b.0 < self.modulus(), "Input {b} >= {}", self.modulus());

        let o = if a.0 >= b.0 {
            a.0 - b.0
        } else {
            (a.0 + self.modulus()) - b.0
        };

        MontgomeryScalar(o)
    }

    fn mont_add(
        &self,
        a: MontgomeryScalar<Scalar>,
        b: MontgomeryScalar<Scalar>,
    ) -> MontgomeryScalar<Scalar> {
        debug_assert!(a.0 < self.modulus(), "Input {a} >= {}", self.modulus());
        debug_assert!(b.0 < self.modulus(), "Input {b} >= {}", self.modulus());

        let mut c = a.0 + b.0;
        if c >= self.modulus() {
            c -= self.modulus();
        }
        MontgomeryScalar(c)
    }

    /// For a & b in Montgomery space, outputs `o = ab*r^{-1} (mod n)`,
    /// where `n` is the original modulus and `o \in [0,n)`
    ///
    /// In general, a*b should in range [0, qr). We assume q is smaller 2^{60}.
    /// If r = 2^64, \log{qr} = 64 + 60 = 124. Therefore a and b can be range [0, 2q)
    ///
    /// - [Reference 1](https://en.algorithmica.org/hpc/number-theory/montgomery/)
    /// - [Reference 2](https://jeffhurchalla.com/2022/04/29/optimized-montgomery-multiplication-with-smaller-modulus-sizes/)
    fn mont_mul(
        &self,
        a: MontgomeryScalar<Scalar>,
        b: MontgomeryScalar<Scalar>,
    ) -> MontgomeryScalar<Scalar> {
        let r = Scalar::BITS as usize;
        let n_inv = self.n_inverse_modr();
        let n = self.modulus();

        let ab = a.0.as_() * b.0.as_();

        // ab * n' (mod r)
        let q = ab.as_().wrapping_mul(&n_inv);

        // (q * n) / r
        let m = ((q.as_() * n.as_()) >> r).as_();

        // ab / r
        let ab_hi = (ab >> r).as_();

        let out = if ab_hi < m { ab_hi + n - m } else { ab_hi - m };
        MontgomeryScalar(out)
    }

    /// For a & b in Montgomery space, outputs `o = ab*r^{-1} (mod n)`,
    /// where `n` is the original modulus and `o \in [0,2n-2)`
    fn mont_mul_lazy(
        &self,
        a: MontgomeryScalar<Scalar>,
        b: MontgomeryScalar<Scalar>,
    ) -> MontgomeryScalar<Scalar> {
        let r = Scalar::BITS as usize;
        let n_inv = self.n_inverse_modr();
        let n = self.modulus();

        let ab = a.0.as_() * b.0.as_();

        // ab * n' (mod r)
        let q = ab.as_().wrapping_mul(&n_inv);

        // (q * n) / r
        let m = ((q.as_() * n.as_()) >> r).as_();

        // ab / r
        let ab_hi = (ab >> r).as_();

        MontgomeryScalar(ab_hi + n - m)
    }

    /// Caculates $\sum{a_i * b_i} \mod{q}$ in Montgomery space.
    ///
    /// Inputs should be in range [0, 2q) and output is in range [0, q)
    ///
    /// TODO (Jay):
    ///     - Apparently FMA can be optmised further. Check https://jeffhurchalla.com/2022/05/01/the-montgomery-multiply-accumulate/
    fn mont_fma(
        &self,
        a: &[MontgomeryScalar<Scalar>],
        b: &[MontgomeryScalar<Scalar>],
    ) -> MontgomeryScalar<Scalar> {
        debug_assert!(a.len() == b.len(), "Length of a and b are not equal");

        let q = self.modulus();

        let mut sum = MontgomeryScalar::zero();
        izip!(a.iter(), b.iter()).for_each(|(a0, b0)| {
            let tmp = self.mont_mul_lazy(*a0, *b0);

            // This is same as mont_add but we need this because `mont_add` assumes inputs are \in [0,q). Whereas tmp + sum
            // is in range [0, 3q). It works because the output `sum` is in range [0, 2q), the expected input range for `mul_lazy`
            sum.0 = sum.0 + tmp.0;
            if sum.0 > q {
                sum.0 -= q;
            }
        });
        sum
    }

    /// Transforms input scalar from normal space to montogmery space.
    /// Calculates and returns a * r (mod n), where n is original modulus.
    ///
    /// We precompute r^2 (mod n) and calculate
    /// (a * r^2)r^{-1} = a *r (mod n) instead
    fn normal_to_mont_space(&self, a: Scalar) -> MontgomeryScalar<Scalar> {
        self.mont_mul(MontgomeryScalar(a), MontgomeryScalar(self.r_square_modn()))
    }

    /// Transforms input from montogmery space to normal space.
    /// Calculates a * r^{-1} (mod n).
    fn mont_to_normal(&self, a: MontgomeryScalar<Scalar>) -> Scalar {
        // since (a * 1) r^{-1} (mod n) = a * r^{-1} (mod n),
        // one can transform `a` from montgomery space to normal
        // space via calling `mont_mul` routine with `1` as second
        // operand.
        self.mont_mul(a, MontgomeryScalar(Scalar::one())).0
    }
}