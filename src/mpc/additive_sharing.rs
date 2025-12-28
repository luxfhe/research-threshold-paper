use num_bigint::{BigInt, UniformBigInt};
use num_traits::Zero;
use rand::distributions::uniform::UniformSampler;

use nalgebra::DVector;
use num_integer::Integer;

pub struct AdditiveSecretSharing;

impl AdditiveSecretSharing {

    pub fn share(secret: &BigInt, num_shares: usize, ring_exponent: usize) -> DVector<BigInt> {
        let mut rng = rand::thread_rng();

        let q = BigInt::from(2u32).pow(ring_exponent as u32);

         let mut shares = DVector::from_fn(num_shares,|i, _| {
            if i < num_shares - 1 {
                UniformBigInt::new(&BigInt::zero(), &q).sample(&mut rng)
            } else {
                BigInt::zero()
            }
        });

        let sum = shares.sum();

        let last_share = (secret - &sum).mod_floor(&q);

        shares[num_shares - 1] = last_share;
        shares
    }

    pub fn reveal(shares: &DVector<BigInt>, ring_exponent: usize) -> BigInt {
        let q = BigInt::from(2u32).pow(ring_exponent as u32);

        let revealed  = shares.sum().mod_floor(&q);

        revealed
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Sub;
    use super::*;
    use num_bigint::BigInt;
    use num_traits::One;


    #[test]
    fn test_add_shares() {
        let s1 = BigInt::from(50);
        let s2 = BigInt::from(100);
        let num_shares = 5;
        let field_exponent = 8;

        let shares1 = AdditiveSecretSharing::share(&s1, num_shares, field_exponent);
        let shares2 = AdditiveSecretSharing::share(&s2, num_shares, field_exponent);

        let s1_add_s2 = (&shares1).sub(&shares2);

        let revealed =  AdditiveSecretSharing::reveal(&s1_add_s2, field_exponent);
        //debug!("s1 = {}",shares1.transpose());
        //debug!("s2 = {}",shares2.transpose());
        //debug!("(s1 + s2) = {}",s1_add_s2.transpose());
        //debug!("reveal(s1 + s2) = {}",revealed);

    }
    #[test]
    fn test_share_reveal() {
        for s in [-257, -5, 0, 5, 257] {
            let secret = BigInt::from(s);
            let num_shares = 5;
            let field_exponent = 8;

            let shares = AdditiveSecretSharing::share(&secret, num_shares, field_exponent);

            // Verify that the number of shares is correct
            assert_eq!(shares.len(), num_shares);

            // Verify that the sum of shares is equal to the secret
            let revealed = AdditiveSecretSharing::reveal(&shares, field_exponent);

            let expected = secret.mod_floor(&(BigInt::one() << field_exponent));
            assert_eq!(revealed, expected)
        }
    }
}