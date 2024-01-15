use super::{
    barrett::BarrettBackend,
    montgomery::{MontgomeryBackend, MontgomeryBackendConfig},
    ModulusBackendConfig, ModulusRandomVecInDistGenerator, ModulusVecBackend,
};
use itertools::{izip, Itertools};
use rand::{
    distributions::{DistIter, Uniform},
    CryptoRng, Rng, RngCore,
};

pub struct NativeModulusBackend {
    pub(crate) modulus: u64,
    modulus_twice: u64,

    barrett_constant: u64,
    barrett_alpha: usize,
    modulus_bits: usize,

    /// Montgomery constant `n^{-1} (mod r)`
    n_inv_modr_mont: u64,
    /// Montgomery constant `r^2 (mod n)`
    r_square_modn_mont: u64,
}

impl ModulusBackendConfig<u64> for NativeModulusBackend {
    fn initialise(modulus: u64) -> NativeModulusBackend {
        let (alpha, mu) = <NativeModulusBackend>::precompute_alpha_and_barrett_constant(modulus);
        let (n_inv_modr_mont, r_square_modn_mont) =
            <NativeModulusBackend as MontgomeryBackendConfig<u64, u128>>::initialise(modulus);

        NativeModulusBackend {
            modulus,
            modulus_twice: modulus * 2,
            barrett_alpha: alpha,
            barrett_constant: mu,
            modulus_bits: 64 - modulus.leading_zeros() as usize,

            n_inv_modr_mont,
            r_square_modn_mont,
        }
    }
}

impl BarrettBackend<u64, u128> for NativeModulusBackend {
    #[inline]
    fn barrett_alpha(&self) -> usize {
        self.barrett_alpha
    }
    #[inline]
    fn modulus(&self) -> u64 {
        self.modulus
    }
    #[inline]
    fn modulus_bits(&self) -> usize {
        self.modulus_bits
    }
    #[inline]
    fn barrett_constant(&self) -> u64 {
        self.barrett_constant
    }
}

impl ModulusVecBackend<u64> for NativeModulusBackend {
    fn add_mod_vec(&self, a: &mut [u64], b: &[u64]) {
        izip!(a.iter_mut(), b.iter()).for_each(|(a0, b0)| {
            *a0 = self.add_mod_fast(*a0, *b0);
        })
    }

    fn sub_mod_vec(&self, a: &mut [u64], b: &[u64]) {
        izip!(a.iter_mut(), b.iter()).for_each(|(a0, b0)| {
            *a0 = self.sub_mod_fast(*a0, *b0);
        })
    }

    fn mul_mod_vec(&self, a: &mut [u64], b: &[u64]) {
        izip!(a.iter_mut(), b.iter()).for_each(|(a0, b0)| {
            *a0 = self.mul_mod_fast(*a0, *b0);
        })
    }
}

impl MontgomeryBackendConfig<u64, u128> for NativeModulusBackend {}
impl MontgomeryBackend<u64, u128> for NativeModulusBackend {
    #[inline]
    fn modulus(&self) -> u64 {
        self.modulus
    }

    fn modulus_twice(&self) -> u64 {
        self.modulus_twice
    }

    #[inline]
    fn n_inverse_modr(&self) -> u64 {
        self.n_inv_modr_mont
    }

    #[inline]
    fn r_square_modn(&self) -> u64 {
        self.r_square_modn_mont
    }
}

impl<'a, R> ModulusRandomVecInDistGenerator<'a, u64, R> for NativeModulusBackend
where
    R: CryptoRng + RngCore + 'a,
{
    type IteratorGaussian =
        std::iter::Take<DistIter<rand::distributions::Uniform<u64>, &'a mut R, u64>>;
    type IteratorUniform =
        std::iter::Take<DistIter<rand::distributions::Uniform<u64>, &'a mut R, u64>>;

    fn random_vec_gaussian_dist_modulus(
        &self,
        std_dev: usize,
        size: usize,
        rng: &mut R,
    ) -> Self::IteratorGaussian {
        todo!()
    }

    fn random_vec_unifrom_dist_modulus(
        &self,
        size: usize,
        rng: &'a mut R,
    ) -> Self::IteratorUniform {
        rng.sample_iter(Uniform::new(0, self.modulus)).take(size)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::core_crypto::modulus::MontgomeryScalar;
    use crate::utils::test_utils::random_vec_in_fq;
    use itertools::Itertools;
    use rand::{thread_rng, Rng};

    const PRIME_60_BITS: u64 = 1152921504606748673;

    const K: usize = 1000;

    #[test]
    fn native_modulus_backend_works() {
        let p = PRIME_60_BITS;
        let mut rng = thread_rng();
        let modulus_backend = <NativeModulusBackend as ModulusBackendConfig<u64>>::initialise(p);
        for _ in 0..K {
            // Case when a,b < p
            let a = rng.gen::<u64>() % p;
            let b = rng.gen::<u64>() % p;

            let c = modulus_backend.mul_mod_fast(a, b);
            let c_expected = ((a as u128 * b as u128) % p as u128) as u64;
            assert_eq!(c, c_expected);

            let c = modulus_backend.add_mod_fast(a, b);
            let c_expected = (a + b) % p;
            assert_eq!(c, c_expected);

            let c = modulus_backend.sub_mod_fast(a, b);
            let c_expected = if a > b { a - b } else { p - (b - a) };
            assert_eq!(c, c_expected);

            // Case when a,b < 2p
            let a = rng.gen_range(0..(2 * p));
            let b = rng.gen_range(0..(2 * p));

            let c = modulus_backend.mul_mod_fast(a, b);
            let c_expected = ((a as u128 * b as u128) % p as u128) as u64;
            assert_eq!(c, c_expected);
        }
    }

    #[test]
    fn native_modulus_montogomery_backend_works() {
        let p = PRIME_60_BITS;
        let mut rng = thread_rng();
        let modulus_backend = <NativeModulusBackend as ModulusBackendConfig<u64>>::initialise(p);
        for _ in 0..1 {
            // Case a,b < p
            let a = rng.gen::<u64>() % p;
            let b = rng.gen::<u64>() % p;

            // To montgomery space
            let a_mont = modulus_backend.normal_to_mont_space(a);
            let b_mont = modulus_backend.normal_to_mont_space(b);

            // Check map to montgomery space inverts to normal space
            assert_eq!(
                a,
                modulus_backend.mont_to_normal(a_mont),
                "Map from normal to montgomery and back is invalid"
            );

            // montgomery multiplication
            let c_mont = modulus_backend.mont_mul(a_mont, b_mont);
            // Back to normal space
            let c = modulus_backend.mont_to_normal(c_mont);
            let c_expected = ((a as u128 * b as u128) % p as u128) as u64;
            assert_eq!(c, c_expected);

            // montgomery addition
            let c_mont = modulus_backend.mont_add(a_mont, b_mont);
            let c = modulus_backend.mont_to_normal(c_mont);
            let c_expected = (a + b) % p;
            assert_eq!(c, c_expected);

            // montgomery subtraction
            let c_mont = modulus_backend.mont_sub(a_mont, b_mont);
            let c = modulus_backend.mont_to_normal(c_mont);
            let c_expected = if a > b { a - b } else { p - (b - a) };
            assert_eq!(c, c_expected);
        }
    }

    #[test]
    fn native_modulus_montgomery_backend_multiplication_with_lazy_input() {
        let p = PRIME_60_BITS;
        let mut rng = thread_rng();
        let modulus_backend = <NativeModulusBackend as ModulusBackendConfig<u64>>::initialise(p);

        for _ in 0..K {
            // montgomery multiplication input case: [0, 2q)
            let a0 = rng.gen::<u64>() % p;
            let a1 = rng.gen::<u64>() % p;
            let b0 = rng.gen::<u64>() % p;
            let b1 = rng.gen::<u64>() % p;

            let a0_mont = modulus_backend.normal_to_mont_space(a0);
            let a1_mont = modulus_backend.normal_to_mont_space(a1);
            let b0_mont = modulus_backend.normal_to_mont_space(b0);
            let b1_mont = modulus_backend.normal_to_mont_space(b1);

            // add a0,a1 / b0,b1 to set them in range [0,2q)
            let a_mont = MontgomeryScalar(a0_mont.0 + a1_mont.0);
            let b_mont = MontgomeryScalar(b0_mont.0 + b1_mont.0);

            let c_mont = modulus_backend.mont_mul(a_mont, b_mont);
            let c = modulus_backend.mont_to_normal(c_mont);
            let c_expected = (((a0 + a1) as u128 * (b0 + b1) as u128) % p as u128) as u64;
            assert_eq!(c, c_expected);
        }
    }

    #[test]
    fn native_modulus_montgomery_native_modulus_montgomery_backend_fma() {
        let q = PRIME_60_BITS;
        let modulus_backend = <NativeModulusBackend as ModulusBackendConfig<u64>>::initialise(q);

        let size = 1 << 15;

        for _ in 0..10 {
            let a_vec = random_vec_in_fq(size, q);
            let a_vec_mont = a_vec
                .iter()
                .map(|v| modulus_backend.normal_to_mont_space(*v))
                .collect_vec();
            let b_vec = random_vec_in_fq(size, q);
            let b_vec_mont = b_vec
                .iter()
                .map(|v| modulus_backend.normal_to_mont_space(*v))
                .collect_vec();

            let c =
                modulus_backend.mont_to_normal(modulus_backend.mont_fma(&a_vec_mont, &b_vec_mont));
            let c_expected = izip!(a_vec.iter(), b_vec.iter()).fold(0u128, |sum, (a0, b0)| {
                sum + ((*a0 as u128 * *b0 as u128) % q as u128)
            });

            assert_eq!(c, (c_expected % (q as u128)) as u64)
        }
    }
}
