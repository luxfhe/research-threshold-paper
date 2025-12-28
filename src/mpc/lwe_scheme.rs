use std::fmt;
use std::ops::Neg;
use nalgebra::{DMatrix, DVector, dvector, stack};
use num_bigint::{BigInt, UniformBigInt};
use num_traits::{One, Zero};
use num_integer::{Integer};
use rand::distributions::uniform::UniformSampler;


pub struct LweScheme {
    pub q: BigInt,
    pub p: BigInt,
    pub sk: DVector<BigInt>,
    // pub pk: DMatrix<BigInt>,

    pub dimension: usize,
    // pub pk_rows: usize,

    p_exponent: usize,
    q_exponent: usize,
}



impl LweScheme {
    pub fn new(p_exponent: usize, q_exponent: usize, dimension: usize, pk_rows: usize) -> Self {
        assert!(p_exponent <= q_exponent);

        let p = BigInt::one() << p_exponent;
        let q = BigInt::one() << q_exponent;

        let mut rng = rand::thread_rng();

        // s random elements in [0, q) of size n
        let sk = DVector::from_fn(dimension, |_i, _| {
            UniformBigInt::new(BigInt::zero(), &q).sample(&mut rng)
        });

        // A random elements in [0, q) of size N x n
        let big_a = DMatrix::from_fn(pk_rows, dimension, |_, _| {
            UniformBigInt::new(BigInt::zero(), &q).sample(&mut rng)
        });

        let q_div_2p: BigInt = &q / (&p * 2);
        // random elements |e| < q/2p
        let  e = DVector::from_fn(pk_rows, |_, _| {
            UniformBigInt::new_inclusive((&q_div_2p).neg(), &q_div_2p).sample(&mut rng)
        });





        // b = -(A * s) + e in [0, q)
        let b = (-(&big_a * &sk) + &e)
            .map(|x| x.mod_floor(&p));





        let _pk = stack![big_a, b];

        LweScheme {
            q,
            p,
            sk,
            // pk,
            dimension,
            // pk_rows,
            p_exponent,
            q_exponent,
        }
    }

    pub fn encrypt(&self, m: &BigInt) -> (DVector<BigInt>, BigInt) {
        let mut rng = rand::thread_rng();

        let q_div_2p: BigInt = &self.q / (&self.p * 2);
        let e = UniformBigInt::new_inclusive(BigInt::zero(), &q_div_2p).sample(&mut rng);

        // a random elements in [0, q)
        let a = DVector::from_fn(self.dimension, |_, _| {
            UniformBigInt::new(BigInt::zero(), &self.q).sample(&mut rng)
        });

        // (q/p) * m
        let m_scaled = (&self.q / &self.p) * m;

        // b = (-<a,sk> + e + (q/p) * m) in [0, q)
        let b = ((&a).dot(&self.sk).neg() + &e + &m_scaled).mod_floor(&self.q);

        (a, b)
    }

    pub fn decrypt(&self, a: &DVector<BigInt>, b: &BigInt) -> BigInt {
        // c = (a, b)
        let c = stack![a.clone(); dvector![b.clone()]];

        // s = (sk, 1)
        let s = stack![self.sk; dvector![BigInt::one()]];

        // m1 = <(a,b),(sk,1)> in [0,q)
        let mut m = (&c).dot(&s).mod_floor(&self.q);

        // m2 = m1 + q/2p in [0,q)
        m = m + (&self.q / (&self.p * BigInt::from(2))).mod_floor(&self.q);

        m = m >> (self.q_exponent - self.p_exponent);

        m
    }
}

impl fmt::Debug for LweScheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LweScheme")
            .field("q", &self.q)
            .field("p", &self.p)
            .field("sk", &self.sk)
            .field("dimension", &self.dimension)
            .finish()
    }
}

impl fmt::Display for LweScheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LweScheme {{ q: {}, p: {}, dimension: {}, }}\nsk: {}\n",
            self.q, self.p, self.dimension, self.sk,
        )
    }
}


pub fn init_lwe_with_random_ptxt(p_exponent: usize, q_exponent: usize, dimension: usize, pk_rows: usize) -> (LweScheme, BigInt, DVector<BigInt>, BigInt, ) {
    let scheme = LweScheme::new(p_exponent, q_exponent, dimension, pk_rows);
    let mut rng = rand::thread_rng();

    let ptxt = UniformBigInt::new(&BigInt::zero(), &BigInt::one() << p_exponent).sample(&mut rng);

    let (a, b) = scheme.encrypt(&ptxt);



    let test_decrypt = scheme.decrypt(&a, &b);

    assert_eq!(ptxt, test_decrypt);

    (scheme, ptxt, a, b,)
}


#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test() {
        loop {
            let q_exponent = 32;
            let p_exponent = 1;
            let dimension = 1024;
            let pk_rows = 1;

            let lwe = LweScheme::new(p_exponent, q_exponent, dimension, pk_rows);



            for m in 0..(1 << p_exponent) {
                let m_in = BigInt::from(m);
                // let (a, b) = lwe.encrypt(&m_in);

                // let m_out = lwe.decrypt(&a, &b);

                // assert_eq!(&m_in, &m_out);

                let (a, b) = lwe.encrypt(&m_in);
                let m_out = lwe.decrypt(&a, &b);


                assert_eq!(m_in, m_out);
            }
        }
    }
}