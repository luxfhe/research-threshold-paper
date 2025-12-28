use std::ops::{Add, AddAssign, Mul, Neg, SubAssign};
use nalgebra::{DMatrix, DVector};
use num_bigint::{BigInt};
use num_integer::Integer;
use num_traits::{ToPrimitive, Zero};
use crate::mpc::additive_sharing::AdditiveSecretSharing;
use crate::mpc::base_decomposition::BaseDecomposition;
use crate::mpc::party::Party;
use crate::mpc::preprocessed_gate::{LessThanZeroFunction, PreprocessedGate, SignFunction};
use crate::mpc::public_params::PublicParameters;
use crate::mpc::utils::round_div;

#[derive(Clone)]
pub struct Protocol {
    pub parties: Vec<Party>,
    pub params: PublicParameters,
}

impl Protocol {

    pub fn new(params: &PublicParameters) -> Protocol {
        let mut parties = Vec::new();
        for i in 0..params.n {
            parties.push(Party::new(i,  params))
        }

        Protocol {
            params: params.clone(),
            parties
        }
    }


    pub fn preprocess(&mut self, s: BigInt, r: BigInt) {
        // Initialize empty Party structs

        // Additive secrete sharing of [s]_(d+1)
        let s_shares = AdditiveSecretSharing::share(&s, self.params.n, self.params.d + 1);

        // Build [LTZ(y)]_m gate
        let ltz_function = LessThanZeroFunction {
          modulo: BigInt::from(self.params.big_d)
        };

        let ltz_gate = PreprocessedGate::build(ltz_function,
            s.clone(), self.params.n, self.params.big_d.to_usize().unwrap(), self.params.m);

        // Additive secrete sharing of [r]_k
        let r_shares = AdditiveSecretSharing::share(&r, self.params.n, self.params.k);

        let base_decomposition = BaseDecomposition {
            base: self.params.big_b
        };

        let r_digits = base_decomposition.decompose(&r);

        let r_digits = DVector::<BigInt>::from_fn(self.params.d,|i, _| {
            if i < r_digits.nrows() {
                r_digits[i].clone()
            }
            else {
                BigInt::zero()
            }
        });

        let mut sign_gates_shares = Vec::<PreprocessedGate<SignFunction>>::new();

        for r_digit in r_digits.iter() {
            let sign_gate = PreprocessedGate::build( SignFunction,
                r_digit.clone(), self.params.n, self.params.big_b, self.params.d + 1);
            sign_gates_shares.push(sign_gate);
        }

        // rows = 2^b = 2^(Digit bit length);   columns = d = Number of digits = ceil(l/b)
        let mut sign_gates_per_party = vec![DMatrix::zeros(self.params.big_b, self.params.d); self.params.n];

        for (d, sign_gate) in sign_gates_shares.iter().enumerate() {
            for i in 0..self.params.n {
                let party_digit_share = sign_gate.get_party_shares(i);
                sign_gates_per_party[i].set_column(d, &party_digit_share);
            }
        }

        for (i, party) in self.parties.iter_mut().enumerate() {
            party.set_s(s_shares[i].clone());
            party.set_r(r_shares[i].clone());
            party.set_ltz(ltz_gate.get_party_shares(i));
            party.set_signs(sign_gates_per_party[i].clone());
        }

    }

    pub fn share_sk(&mut self, sk: DVector<BigInt>) {
        let mut sk_shares_per_party = DMatrix::<BigInt>::zeros(sk.nrows(), self.params.n);
        for (i, sk_digit) in sk.iter().enumerate() {

            // Share each digit of secret key [sk]_k
            let sk_digit_shares = AdditiveSecretSharing::share(sk_digit, self.params.n, self.params.k);
            sk_shares_per_party.set_row(i, &sk_digit_shares.transpose());
        }


        for (i, party) in self.parties.iter_mut().enumerate() {
            let party_sk: DVector<BigInt> = sk_shares_per_party.column(i).into();

            party.set_sk(party_sk);
        }
    }

    pub fn noisy_decrypt(&mut self, z: BigInt) -> BigInt{
        let z_shares = AdditiveSecretSharing::share(&z, self.params.n, self.params.k);

        for (i, party) in self.parties.iter_mut().enumerate() {
            party.set_z(z_shares[i].clone());
        }

        self.mod_l_protocol();

        // assert_eq!(e, z.mod_floor(&self.params.big_l));

        let o_shares = DVector::<BigInt>::from_fn(self.params.n, |i, _| {
            let neg_e = self.parties[i].get_e()
                .neg()
                .mod_floor(&self.params.q);

            let o = self.parties[i].get_z().add(&neg_e);

            o
        });

        AdditiveSecretSharing::reveal(&o_shares, self.params.k)
    }


    pub fn decrypt(&mut self, a: DVector<BigInt>, b: BigInt) -> BigInt {
        //--------------------------------------------------------------------------------------------------------------------------------
        //                         Start Step 1
        //--------------------------------------------------------------------------------------------------------------------------------

        // Each party calculates z = b - <a, s> + 2^(l-1)
        // and sets its share of z
        for (i, party) in self.parties.iter_mut().enumerate() {
            let mut z = BigInt::zero();
            // -<a,sk>
            let neg_a_dot_sk = a.dot(&party.get_sk())
                .neg()
                .mod_floor(&self.params.q);

            if i == 0 {
                let add_term = BigInt::from(2u32).pow(self.params.l as u32 - 1);
                z.add_assign(&b);
                z.add_assign(&add_term);
            }

            z.sub_assign(&neg_a_dot_sk);

            party.set_z(z);
        }

        // All parties have a share of z, and LTZ + Sign gates
        self.mod_l_protocol();

        let o_prime_shares = DVector::<BigInt>::from_fn(self.params.n, |i, _| {
            let neg_e = self.parties[i].get_e()
                .neg()
                .mod_floor(&self.params.q);

            let o_prime = self.parties[i].get_z().add(&neg_e).mod_floor(&self.params.q);

            self.parties[i].set_o_prime(o_prime.clone());
            o_prime
        });

        //--------------------------------------------------------------------------------------------------------------------------------
        //                        End Step 3
        //--------------------------------------------------------------------------------------------------------------------------------

        // Reveal(z - e)
        let o_prime = AdditiveSecretSharing::reveal(&o_prime_shares, self.params.k);

        let msg = round_div(&o_prime, &self.params.big_l);

        msg
    }

    // returns sharing [e] where:
    // e = z_prime - r + L * u
    // parties already have shares of [z] and [r] are already
    pub fn mod_l_protocol(&mut self, )  {
        // z' =  [z] + [r] (just the lower 'l' bits)
        let z_prime_lower_bit_shares = DVector::<BigInt>::from_fn(self.parties.len(), |i, _| {
            let z_prime = (self.parties[i].get_z() + self.parties[i].get_r()).mod_floor(&self.params.big_l);
            self.parties[i].set_z_prime(z_prime.clone());
            z_prime
        });

        //--------------------------------------------------------------------------------------------------------------------------------
        //                         End Step 1
        //--------------------------------------------------------------------------------------------------------------------------------


        // z' lower l bits are revealed
        let z_prime = AdditiveSecretSharing::reveal(&z_prime_lower_bit_shares, self.params.l);

        // [u] = [(z' <? r)]

        //--------------------------------------------------------------------------------------------------------------------------------
        //                         Start Step 2
        //--------------------------------------------------------------------------------------------------------------------------------


        // return u_shares only for debug, remove after correctness
        self.lt_r_l_protocol(&z_prime);

        // [e] = z' - [r] + L * [u] (mod q)
        for (i, party) in self.parties.iter_mut().enumerate() {
            let mut e = BigInt::zero();

            let neg_r = party.get_r()
                .neg()
                .mod_floor(&self.params.q);

            if i == 0 {
                e.add_assign(&z_prime);
            }

            e.add_assign(&neg_r);

            let big_l_mul_u = party.get_u()
                .mul(&self.params.big_l); //.mod_floor(&self.params.p);

            e.add_assign(&big_l_mul_u);

            party.set_e(e.clone());
        }

    }


    pub fn lt_r_l_protocol(&mut self, z_prime: &BigInt) -> DVector<BigInt> {
        // y' = y + s (mod 2^(d+1))
        let y_shares = self.weighted_signs_protocol(z_prime);

        let u_shares = self.ltz_protocol(y_shares);

        for (i, party) in self.parties.iter_mut().enumerate() {
            party.set_u(u_shares[i].clone());
        }

        u_shares
    }


    pub fn ltz_protocol(&mut self, y_shares: DVector<BigInt>) -> DVector<BigInt> {

        // Each party gets a share of y
        for (i, party) in self.parties.iter_mut().enumerate() {
            party.set_y(y_shares[i].clone());
        }

        // shares of y_prime = [y] + [s] are received from all parties
        let y_prime_shares = DVector::<BigInt>::from_fn(self.parties.len(), |i, _| {
            let y_prime = (self.parties[i].get_y() + self.parties[i].get_s())
                .mod_floor(&BigInt::from(self.params.big_d));

            self.parties[i].set_y_prime(y_prime.clone());
            y_prime
        });

        //--------------------------------------------------------------------------------------------------------------------------------
        //                         End Step 2
        //--------------------------------------------------------------------------------------------------------------------------------


        // y_prime := y mod 2^(d+1) is revealed
        let y_prime = AdditiveSecretSharing::reveal(&y_prime_shares, self.params.d + 1);

        //--------------------------------------------------------------------------------------------------------------------------------
        //                         Start Step 3
        //--------------------------------------------------------------------------------------------------------------------------------


        let y_prime = y_prime.to_usize().unwrap();

        // shares of [LTZ(y)] = [LTZ(y' - s)] are received from all parties
        let ltz_y = DVector::<BigInt>::from_fn(self.parties.len(), |i, _| {
            self.parties[i].get_ltz()[y_prime].clone()
        });

        ltz_y
    }


    pub fn weighted_signs_protocol(&self, z_prime: &BigInt) -> DVector<BigInt>{
        // Base decomposition in base B of public z_prime
        let base_decomposition = BaseDecomposition {
            base: self.params.big_b
        };

        let decomposition = base_decomposition.decompose(z_prime);


        let z_prime_digits = DVector::<BigInt>::from_fn(self.params.d,|i, _| {
            if i < decomposition.nrows() {
                decomposition[i].clone()
            }
            else {
                BigInt::zero()
            }
        });

        // Each party executes locally WeightedSigns function and the result is assigned into [y] share
        let y_shares = DVector::<BigInt>::from_fn(self.parties.len(), |i, _| {

            self.parties[i].calc_weighted_sum(z_prime_digits.clone())
        });

        y_shares
    }
}


#[cfg(test)]
mod tests {
    use std::ops::{Div, Neg};
    use nalgebra::DMatrix;
    use num_bigint::{BigInt, UniformBigInt};
    
    use num_traits::Zero;
    use rand::distributions::uniform::UniformSampler;
    use crate::mpc::additive_sharing::AdditiveSecretSharing;
    use crate::mpc::lwe_scheme::init_lwe_with_random_ptxt;
    use crate::mpc::mac_scheme::{AuthenticatedSharingScheme, MACSchemeParams};
    use crate::mpc::protocol::Protocol;
    use crate::mpc::public_params::PublicParameters;

    #[test]
    fn test_decrypt_mac() {
        let k = 64;     // Ciphertext bit length
        let mac_s = 80; // MAC Security parameter bit length
        let mac_t = 3;  // MAC batch count
        let n = 4;      // Number of parties
        let m = 1;      // Plaintext bit length
        let b = 8;      // "Digit" bit length
        let lwe_dimension = 1024;


        let params = PublicParameters::init(n, k, m, b, lwe_dimension,0);
        //debug!("{params}");

        let mut rng = rand::thread_rng();

        let (lwe_scheme, ptxt, a, b, ) = init_lwe_with_random_ptxt(params.m, params.k, params.lwe_dimension, 1);

        let s = UniformBigInt::new(&BigInt::zero(), &BigInt::from(params.big_d)).sample(&mut rng);

        let r = UniformBigInt::new(&BigInt::zero(), &params.big_l).sample(&mut rng);

        let alpha = UniformBigInt::new(&BigInt::zero(), &params.big_l).sample(&mut rng);

        let mac_scheme_params = MACSchemeParams::init(n, k, mac_s, mac_t);

        let mac_scheme = AuthenticatedSharingScheme::new(alpha.clone(), mac_scheme_params.clone());

        let alpha_shares = AdditiveSecretSharing::share(&alpha, params.n, mac_scheme_params.ks);

        let mut protocol = Protocol::new(&params);

        for (i, party) in protocol.parties.iter_mut().enumerate() {
            party.set_mac_alpha(alpha_shares[i].clone());
        }

        protocol.preprocess(s, r);

        protocol.share_sk(lwe_scheme.sk);

        let out = protocol.decrypt(a, b);

        assert_eq!(out, ptxt);


        let x_shares_collection = DMatrix::from_fn(n, 3,|i, j| {
            match j {
                0 => protocol.parties[i].get_z_prime().clone(),
                1 => protocol.parties[i].get_y_prime().clone(),
                2 => protocol.parties[i].get_o_prime().clone(),
                _ => BigInt::zero() // error
            }
        });


        let (x_tilde_shares, m_tilde_collection) = mac_scheme.
            batch_open(&x_shares_collection);

        let checked_values = mac_scheme.batch_check(&x_tilde_shares, &alpha_shares, &m_tilde_collection);
        assert!(checked_values.is_some());
    }

    #[test]
    fn test_decrypt() {

        // Experimented values:
        // k = 64
        // m = 1, 2, 4
        // b = 5, 6, 7, 8, 9

        // loop {
            let k = 64;     // Ciphertext bit length
            let mac_s = 80; // MAC Security parameter bit length
            let mac_t = 3;  // MAC batch count
            let n = 4;      // Number of parties
            let m = 1;      // Plaintext bit length
            let b = 7;      // "Digit" bit length
            let lwe_dimension = 1024;


            let params = PublicParameters::init(n, k, m, b, lwe_dimension,0);
            //debug!("{params}");

            let mut rng = rand::thread_rng();

            let (lwe_scheme, ptxt, a, b, ) = init_lwe_with_random_ptxt(params.m, params.k, params.lwe_dimension, 1);

            let s = UniformBigInt::new(&BigInt::zero(), &BigInt::from(params.big_d)).sample(&mut rng);

            let r = UniformBigInt::new(&BigInt::zero(), &params.big_l).sample(&mut rng);

            let global_mac_key = UniformBigInt::new(&BigInt::zero(), &params.big_l).sample(&mut rng);


            let mut protocol = Protocol::new(&params);

            protocol.preprocess(s, r);

            protocol.share_sk(lwe_scheme.sk);

            let out = protocol.decrypt(a, b);

            println!("{:#?}", out);
            println!("{:#?}", ptxt);
            assert_eq!(out, ptxt);
        // }
    }

    #[test]
    fn debug_template() {
        // loop {
            let k = 64;         // Ciphertext bit length
            let mac_s = 16;     // MAC Security parameter bit length
            let mac_t = 3;      // MAC batch count
            let n = 4;          // Number of parties
            let m = 4;          // Plaintext bit length
            let b = 8;          // "Digit" bit length
            let lwe_dimension = 1024;




            let params = PublicParameters::init(n, k, m, b, lwe_dimension,0);
            //debug!("{params}");

            let mut rng = rand::thread_rng();


            let s = UniformBigInt::new(&BigInt::zero(), &BigInt::from(params.big_d)).sample(&mut rng);
            // let s = BigInt::from(6);


            let r = UniformBigInt::new(&BigInt::zero(), &params.big_l).sample(&mut rng);


            // z = random in (-q/2, q/2)
            let z = UniformBigInt::new(&(&params.q).div(BigInt::from(2)).neg(), &(&params.q).div(2)).sample(&mut rng);





            let mut protocol = Protocol::new(&params);
            protocol.preprocess(s, r);

            let z_sub_e = protocol.noisy_decrypt(z);



        // }
    }
}