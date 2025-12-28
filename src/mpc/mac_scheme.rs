use nalgebra::{DMatrix, DVector};
use num_bigint::{BigInt, UniformBigInt};
use num_integer::Integer;
use num_traits::{One, Zero};
use rand::distributions::uniform::UniformSampler;
use crate::mpc::additive_sharing::AdditiveSecretSharing;


#[derive(Default, Clone, Debug, PartialEq)]

pub struct MACSchemeParams {
    /// number of players
    pub n: usize,

    /// smaller ring bits
    pub k: usize,

    /// security parameter bits
    pub s: usize,

    /// larger ring bits
    pub ks: usize,

    /// number of batch shares
    pub t: usize,

    /// smaller ring size
    big_k: BigInt,

    /// security parameter size
    big_s: BigInt,

    /// larger ring size
    big_ks: BigInt,
}

impl MACSchemeParams {
    pub fn init(players_count: usize, ring_bits: usize, security_parameter_bits: usize, jobs_per_worker: usize) -> Self {
        Self {
            n: players_count,
            k: ring_bits,
            s: security_parameter_bits,
            ks: ring_bits + security_parameter_bits,
            t: jobs_per_worker,
            big_k: BigInt::one() << ring_bits,
            big_s: BigInt::one() << security_parameter_bits,
            big_ks: BigInt::one() << (ring_bits + security_parameter_bits),
        }
    }
}

// see: https://eprint.iacr.org/2018/482.pdf - 3. Information-Theoretic MAC Scheme
#[derive(Default, Clone)]
pub struct AuthenticatedSharingScheme {

    pub params: MACSchemeParams,
    /// global key
    pub alpha: BigInt,
}

impl AuthenticatedSharingScheme {

    pub fn new(global_key_share: BigInt, params: MACSchemeParams) -> AuthenticatedSharingScheme {

        Self {
            params,
            alpha: global_key_share
        }
    }


    // For centralized benchmarks
    pub fn share_global_key(&self) -> DVector<BigInt> {
        AdditiveSecretSharing::share(&self.alpha, self.params.n, self.params.ks)
    }


    pub fn batch_open(&self, x_shares_collection: &DMatrix<BigInt>) -> (DVector<BigInt>, DMatrix<BigInt>) {


        assert_eq!(x_shares_collection.nrows(), self.params.n);
        let t = x_shares_collection.ncols();

        let mut rng = rand::thread_rng();

        let mut x_tilde_shares_collection = Vec::new();

        for i in 0..t {
            let r_i = UniformBigInt::new(BigInt::zero(), &self.params.big_s).sample(&mut rng);
            let r_i_shares = AdditiveSecretSharing::share(&r_i, self.params.n, self.params.s);

            let x_tilde_i: DVector<BigInt> = x_shares_collection.column(i) +
                (&r_i_shares.map(|x| &x * &self.params.big_k));    // = [x_i] + 2^k * [r_i]

            x_tilde_shares_collection.push(x_tilde_i)
        }

        let x_tilde_shares_collection = DMatrix::from_columns(&x_tilde_shares_collection);



        let m_tilde_collection = DMatrix::from_fn(self.params.n, t, |i ,j|{
            let val = &self.alpha * &x_tilde_shares_collection[(i, j)];
            val.mod_floor(&self.params.big_ks)
        });

        // Broadcast x_tilde_shares_collection between players

        let x_tilde_shares = x_tilde_shares_collection.row_sum().map(|val| val.mod_floor(&self.params.big_ks));

        (x_tilde_shares.transpose(), m_tilde_collection)
    }

    //
    pub fn batch_check(&self, x_tilde_shares: &DVector<BigInt>, global_key_shares: &DVector<BigInt>,
                       m_tilde_collection: &DMatrix<BigInt>) -> Option<DVector<BigInt>> {
        let t = x_tilde_shares.nrows();



        assert_eq!(global_key_shares.nrows(), self.params.n);
        assert_eq!(m_tilde_collection.nrows(), self.params.n);
        assert_eq!(m_tilde_collection.ncols(), t);




        let mut rng = rand::thread_rng();

        let chi_vals = DVector::from_fn(t, |_i, _| {
            UniformBigInt::new(BigInt::zero(), &self.params.big_s).sample(&mut rng)
        });





        let y_tilde = (&chi_vals).dot(&x_tilde_shares).mod_floor(&self.params.big_ks);



        let combined_mac_shares = DVector::from_fn(self.params.n, |i,_| {
            let player_mac_shares = m_tilde_collection.row(i).transpose();

            (&chi_vals).dot(&player_mac_shares).mod_floor(&self.params.big_ks)
        });

        let z_shares = DVector::from_fn(self.params.n, |i, _| {
            let z = &combined_mac_shares[i] - (&global_key_shares[i] * &y_tilde);
            z.mod_floor(&self.params.big_ks)
        });


        if z_shares.sum().mod_floor(&self.params.big_ks) == BigInt::zero() {
            let values = DVector::from_fn(t, |i, _| {
                x_tilde_shares[i].mod_floor(&self.params.big_k)
            });

            Some(values)
        }
        else {
            None
        }
    }


    pub fn single_open(&self, x_shares: &DVector<BigInt>) -> DVector<BigInt> {


        assert_eq!(x_shares.nrows(), self.params.n);

        let mut rng = rand::thread_rng();

        // Generate random shared value r of size s bits - mimics "re-shuffle"
        let r = UniformBigInt::new(BigInt::zero(), &self.params.big_s).sample(&mut rng);
        let r_shares =  AdditiveSecretSharing::share(&r, self.params.n, self.params.s);




        // [y] = [x + 2^k * r]
        let opened_secrets = DVector::from_fn(self.params.n, |i, _| {
            let y = &x_shares[i] + (&self.params.big_k * &r_shares[i]);
            y.mod_floor(&self.params.big_ks)
        });



        opened_secrets
    }


    pub fn single_check(&self, y: &BigInt, alpha_shares: &DVector<BigInt>) -> Option<BigInt> {

        assert_eq!(alpha_shares.nrows(), self.params.n);



        let y_mac = (y * &self.alpha).mod_floor(&self.params.big_ks);
        let y_mac_shares = AdditiveSecretSharing::share(&y_mac, self.params.n, self.params.ks);




        let z_shares = DVector::from_fn(self.params.n, |i, _| {
            let z = &y_mac_shares[i] - (&alpha_shares[i] * y);
            z.mod_floor(&self.params.big_ks)
        });







        if z_shares.sum().mod_floor(&self.params.big_ks) == BigInt::zero() {
            let value = y.mod_floor(&self.params.big_k);
            Some(value)
        }
        else { None }
    }
}


#[cfg(test)]
mod tests {
    use nalgebra::DMatrix;
    use num_bigint::{BigInt, UniformBigInt};
    use num_traits::{One, Zero};
    use rand::distributions::uniform::UniformSampler;
    use crate::mpc::additive_sharing::AdditiveSecretSharing;
    use crate::mpc::mac_scheme::{AuthenticatedSharingScheme, MACSchemeParams};

    #[test]
    fn test_batch_check() {
        let mut rng = rand::thread_rng();

        let n = 4;
        let k = 8;      // computation ring size = 2^k
        let s = 16;      // security param
        let t = 3;      // secret values count

        let params = MACSchemeParams::init(n, k, s, t);

        // s bits
        let alpha = UniformBigInt::new(BigInt::zero(), &BigInt::one() << params.s).sample(&mut rng);


        let alpha_shares = AdditiveSecretSharing::share(&alpha, n, params.ks);


        let scheme  = AuthenticatedSharingScheme::new(alpha.clone(), params);


        let mut x_shares_collection = Vec::new();
        let mut x_values = Vec::new();
        for i in 0..t {
            // k bits
            let x = UniformBigInt::new(BigInt::zero(), &BigInt::one() << k).sample(&mut rng);

            let x_shares = AdditiveSecretSharing::share(&x, scheme.params.n, scheme.params.ks);

            // shares l bits
            x_values.push(x);

            x_shares_collection.push(x_shares);
        }

        // n rows, t columns
        let x_shares_collection = DMatrix::from_columns(&x_shares_collection);



        let (x_tilde_shares, m_tilde_collection) = scheme.batch_open(&x_shares_collection);






        let checked_values = scheme.batch_check(&x_tilde_shares, &alpha_shares, &m_tilde_collection);
        assert!(checked_values.is_some());

        for (i, checked_value) in checked_values.unwrap().iter().enumerate() {
            assert_eq!(*checked_value, x_values[i]);

        }

    }



    #[test]
    fn test_single_check() {
        let mut rng = rand::thread_rng();

        let n = 4;
        let k = 8;
        let s = 16;
        let t = 1;

        let params = MACSchemeParams::init(n, k, s, t);

        // s bits
        let alpha = UniformBigInt::new(BigInt::zero(), &BigInt::one() << params.s).sample(&mut rng);


        let alpha_shares = AdditiveSecretSharing::share(&alpha, params.n, params.ks);


        let scheme  = AuthenticatedSharingScheme::new(alpha.clone(), params);



        // k bits
        let x = UniformBigInt::new(BigInt::zero(), &BigInt::one() << k).sample(&mut rng);



        // shares l bits
        let x_shares = AdditiveSecretSharing::share(&x, scheme.params.n, scheme.params.k);





        let y_shares = scheme.single_open(&x_shares);

        let y = AdditiveSecretSharing::reveal(&y_shares, scheme.params.ks);



        // Broadcast y
        let checked_value = scheme.single_check(&y, &alpha_shares);



        assert!(checked_value.is_some());
        assert_eq!(checked_value.unwrap(), x);
    }
}