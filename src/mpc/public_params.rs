use std::fmt;
use num_bigint::BigInt;
use num_traits::One;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PublicParameters {
    /// Number of parties
    pub n: usize,

    /// Ciphertext bit length
    pub k: usize,

    /// Plaintext bit length
    pub m: usize,

    /// Ciphertext modulus = 2^k
    pub q: BigInt,

    /// Plaintext modulus = 2^m
    pub p: BigInt,

    /// Noise budget l = k - m
    pub l : usize,

    /// L = 2^l
    pub big_l: BigInt,

    /// b = "Digit" bit length
    pub b: usize,   // should be small

    /// B = 2^b
    pub big_b: usize,

    /// Number of digits = ceil(l/b)
    pub d: usize,

    /// 2^(d+1)
    pub big_d: usize,

    /// Top digit length b' = l - (d - 1)*b
    pub b_prime: usize,

    /// B' = 2^b'
    pub big_b_prime: BigInt,

    /// LWE scheme key size
    pub lwe_dimension: usize,

    /// MAC security parameter bits
    pub mac_s: usize,

    /// 2 ^ (mac_s)
    pub mac_big_s: BigInt,

    pub mac_k: usize,

    pub mac_big_k: BigInt,

    /// MAC smaller ring bits + security parameter bits
    pub mac_ks: usize,

    /// 2 ^ (mac_ks)
    pub mac_big_ks: BigInt,

}




impl PublicParameters {


    pub fn init(n:usize, k:usize, m: usize, b: usize, lwe_dimension: usize, mac_s: usize) -> PublicParameters {
        let l = k - m;
        let d = f64::ceil(l as f64 / b as f64) as usize;
        let big_d = 2usize.pow(d as u32 + 1);
        let b_prime = l - (d - 1) * b;

        let q = BigInt::from(2u32).pow(k as u32);
        let p = BigInt::from(2u32).pow(m as u32);
        let big_l = BigInt::from(2u32).pow(l as u32);
        let big_b = 2usize.pow(b as u32);
        let big_b_prime = BigInt::from(2u32).pow(b_prime as u32);

        let mac_ks = k + mac_s;

        let mac_k = k;

        let mac_big_k = BigInt::one() << mac_k;


        let mac_big_s = BigInt::one() << mac_s;

        let mac_big_ks = BigInt::one() << mac_ks;


        PublicParameters {
            n,
            k,
            m,
            q,
            p,
            l,
            big_l,
            b,
            big_b,
            d,
            big_d,
            b_prime,
            big_b_prime,
            lwe_dimension,
            mac_k,
            mac_big_k,
            mac_s,
            mac_ks,
            mac_big_s,
            mac_big_ks
        }
    }


    // Experimented values:
    // k = 64
    // m = 1, 2, 4
    // b = 5, 6, 7, 8, 9
    pub fn default() -> PublicParameters {
        let (n, k, m, b, lwe_dimension, mac_s) = (4, 64, 4, 7, 1024, 80);
        PublicParameters::init(n, k, m, b, lwe_dimension, mac_s)
    }



}

impl fmt::Display for PublicParameters {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PublicParameters {{ \n
                n: {}\t Number of parties,\n
                k: {}\t Ciphertext bit length\n
                m: {}\t Plaintext bit length\n
                q: {}\t Ciphertext modulus = 2^k\n
                p: {}\t Plaintext modulus = 2^m\n
                l: {}\t l = k - m\n
                L: {}\t L = 2^l\n
                b: {}\t 'Digit' bit length\n
                B: {}\t B = 2^b\n
                d: {}\t Number of digits d = ceil(l/b)\n
                D: {}\t 2^(d+1)\n
                b': {}\t Top digit bit length b' = l - (d - 1)*b\n
                B': {}\t B' = 2^b'\n
                lwe_dimension: {} LWE scheme key size\n
                mac_ks: {}\t TODO + MAC scheme security parameter\n
            }}",
            self.n,
            self.k,
            self.m,
            self.q,
            self.p,
            self.l,
            self.big_l,
            self.b,
            self.big_b,
            self.d,
            self.big_d,
            self.b_prime,
            self.big_b_prime,
            self.lwe_dimension,
            self.mac_ks
        )
    }
}
