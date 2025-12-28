use std::fmt;
use std::fmt::{Debug, Display};
use std::ops::{Add, AddAssign, Mul, Neg, SubAssign};
use std::time::Instant;
use log::debug;
use nalgebra::{DMatrix, DVector};
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{ToPrimitive, Zero};
use crate::mpc::additive_sharing::AdditiveSecretSharing;
use crate::mpc::base_decomposition::BaseDecomposition;
use crate::mpc::public_params::PublicParameters;
use crate::mpc::utils::round_div;
use crate::network::ProtocolTransferredData;

use bitcode::{serialize, deserialize};
use crate::generate_getters_and_setters;
use paste::paste;

#[derive(Debug, Clone, PartialEq)]
pub struct Party {
    pub party_number: usize,

    pub params: PublicParameters,

    a: Option<DVector<BigInt>>,
    b: Option<BigInt>,

    s: Option<BigInt>,  // from preprocessing
    z: Option<BigInt>,  // z = b - <a,sk> + 2^(l)
    r: Option<BigInt>,  // from preprocessing

    y: Option<BigInt>,  // from get_weighted_signs
    u: Option<BigInt>,  // from LT_r_l
    e: Option<BigInt>,  // from Mod_l

    sk: Option<DVector<BigInt>>,
    ltz: Option<DVector<BigInt>>,
    signs: Option<DMatrix<BigInt>>,     // rows = B =  2^b = 2^(Digit bit length);   columns = d = Number of digits = ceil(l/b)


    z_prime: Option<BigInt>,
    y_prime: Option<BigInt>,
    o_prime: Option<BigInt>,

    z_prime_all_parties: Option<DVector<BigInt>>,
    y_prime_all_parties: Option<DVector<BigInt>>,
    o_prime_all_parties: Option<DVector<BigInt>>,

    // MAC key share
    // alpha: Option<BigInt>,
    mac_alpha: Option<BigInt>,

    mac_r: Option<DVector<BigInt>>,
    mac_m_tilde:Option<BigInt>,
    mac_z: Option<BigInt>,
    mac_chi_values: Option<DVector<BigInt>>,

    start_time: Option<Instant>,


}


generate_getters_and_setters! {
    Party,
    a: DVector<BigInt>,
    b: BigInt,
    s: BigInt,
    z: BigInt,
    r: BigInt,
    y: BigInt,
    u: BigInt,
    e: BigInt,
    sk: DVector<BigInt>,
    ltz: DVector<BigInt>,
    signs: DMatrix<BigInt>,
    z_prime: BigInt,
    y_prime: BigInt,
    o_prime: BigInt,
    z_prime_all_parties: DVector<BigInt>,
    y_prime_all_parties: DVector<BigInt>,
    o_prime_all_parties: DVector<BigInt>,
    mac_alpha: BigInt,
    mac_r: DVector<BigInt>,
    mac_z: BigInt,
    mac_chi_values: DVector<BigInt>
}


impl Party {

    pub fn get_params(&self) -> &PublicParameters {
        &self.params
    }

    /// Initialization method
    pub fn new(party_number: usize, params: &PublicParameters) -> Self {
        Party {
            party_number,
            params: params.clone(),
            a: None,
            b: None,
            s: None,
            z: None,
            r: None,
            y: None,
            u: None,
            e: None,
            sk: None,
            signs: None,
            ltz: None,

            z_prime: None,
            y_prime: None,
            o_prime: None,


            z_prime_all_parties: None,
            y_prime_all_parties: None,
            o_prime_all_parties: None,

            // alpha: None,
            mac_alpha: None,
            mac_r: None,
            mac_m_tilde: None,
            mac_z: None,
            mac_chi_values: None,

            start_time: None

        }
    }


    fn get_sign(&self, digit_index: usize, digit_value: usize) -> BigInt {
        assert!(digit_index < self.get_signs().ncols() && digit_value < self.get_signs().nrows());

        self.get_signs()[(digit_value, digit_index)].clone()
    }


    pub fn calc_weighted_sum(&self, z_prime_digits: DVector<BigInt>) -> BigInt {
        assert_eq!(z_prime_digits.nrows(), self.get_signs().ncols());

        let mut lin_comb = BigInt::zero();

        for (i, digit) in z_prime_digits.iter().enumerate() {

            // get to digit num i and retrieve the share at the digit value
            let digit_sign = self.get_sign(i, digit.to_usize().unwrap());

            // print!("DEBUG: ({digit_sign} * 2^{i}) + ");

            let two_pow = BigInt::from(2u32).pow(i as u32);
            lin_comb.add_assign(digit_sign.clone() * &two_pow);

        }
        lin_comb
    }


    pub fn execute_step(&mut self, step_number: usize, input: Vec<ProtocolTransferredData>)  -> ProtocolTransferredData {
        let output = match step_number {
            0 => {
                self.start_time = Some(Instant::now());

                let out = self.execute_step_one(input);
                out
            },
            1 => self.execute_step_two(input),
            2 => self.execute_step_three(input),
            3 => self.execute_step_four(input),
            4 => {
                let out = self.execute_step_five(input);

                let elapsed = self.start_time.unwrap().elapsed();

                let microseconds = elapsed.as_micros();

                self.start_time = None;

                debug!("party: {}, n: {}, k: {}, m: {}, b: {}, mac_s: {}, lwe_a: {}, microseconds: {}",
                         self.party_number,
                         self.params.n,
                         self.params.k,
                         self.params.m,
                         self.params.b,
                         self.params.mac_s,
                         self.params.lwe_dimension,
                         microseconds);

                out
            },
            _ => unreachable!()
        };


        output
    }



    pub fn execute_step_one(&mut self, _input: Vec<ProtocolTransferredData>)  -> ProtocolTransferredData {

        //debug!("execute_step_one {:?}", self);


        // MPC decryption protocol
        let mut z = BigInt::zero();
        // -<a,sk>
        let neg_a_dot_sk = self.get_a().dot(&self.get_sk())
            .neg()
            .mod_floor(&self.params.q);

        if self.party_number == 0 {

            let add_term = BigInt::from(2u32).pow(self.params.l as u32 - 1);
            z.add_assign(self.get_b());
            z.add_assign(&add_term);
        }

        z.sub_assign(&neg_a_dot_sk);

        self.set_z(z);

        let z_prime = (self.get_z() + self.get_r()).mod_floor(&self.params.big_l);
        self.set_z_prime(z_prime.clone());

        // let start_time = std::time::Instant::now();

        let output = ProtocolTransferredData {
            preprocessed: None,
            a: None,
            b: None,
            z_prime: Some(serialize(&z_prime).unwrap()),
            y_prime: None,
            o_prime: None,
            // alpha: None,
            mac_alpha: None,
            mac_r: None,
            // mac_x_tilde_collection: None,
            // mac_m_tilde_collection: None,
            mac_chi_vals: None,
            mac_z: None
        };

        // let elapsed = start_time.elapsed();

        // let _microseconds = elapsed.as_micros();

        //debug!("Serialize traffic from 'participant {}' to other participants' = {_microseconds} microseconds", self.party_number);

        output

    }

    pub fn execute_step_two(&mut self, input: Vec<ProtocolTransferredData>) -> ProtocolTransferredData {
        //debug!("execute_step_two {:?}", self);
        let mut z_prime_shares = Vec::new();
        z_prime_shares.push(self.get_z_prime().clone());
        for share in input {
            let z_prime_share: BigInt = deserialize(&share.z_prime.unwrap()).unwrap();
            z_prime_shares.push(z_prime_share);
        }

        z_prime_shares.sort();
        self.set_z_prime_all_parties(DVector::from_vec(z_prime_shares));

        let z_prime = AdditiveSecretSharing::reveal(&self.get_z_prime_all_parties(), self.params.l);


        let base_decomposition = BaseDecomposition {
            base: self.params.big_b
        };

        let decomposition = base_decomposition.decompose(&z_prime);


        let z_prime_digits = DVector::<BigInt>::from_fn(self.params.d,|i, _| {
            if i < decomposition.nrows() {
                decomposition[i].clone()
            }
            else {
                BigInt::zero()
            }
        });

        let y = self.calc_weighted_sum(z_prime_digits.clone());
        self.set_y(y);

        let y_prime = (self.get_y() + self.get_s())
            .mod_floor(&BigInt::from(self.params.big_d));

        self.set_y_prime(y_prime.clone());

        let start_time = std::time::Instant::now();

        let output = ProtocolTransferredData {
            preprocessed: None,
            a: None,
            b: None,
            z_prime: None,
            y_prime: Some(serialize(&y_prime).unwrap()),
            o_prime: None,

            // alpha: None,
            mac_alpha: None,
            mac_r: None,
            // mac_x_tilde_collection: None,
            // mac_m_tilde_collection: None,
            mac_chi_vals: None,
            mac_z: None,
        };

        let elapsed = start_time.elapsed();

        let _microseconds = elapsed.as_micros();

        //debug!("Serialize traffic from 'participant {}' to other participants' = {_microseconds} microseconds", self.party_number);


        output
    }

    pub fn execute_step_three(&mut self, input: Vec<ProtocolTransferredData>) -> ProtocolTransferredData {

        //debug!("execute_step_three {:?}", self);
        let mut y_prime_shares = Vec::new();
        y_prime_shares.push(self.get_y_prime().clone());
        for share in input {
            let y_prime_share: BigInt = deserialize(&share.y_prime.unwrap()).unwrap();
            y_prime_shares.push(y_prime_share);
        }

        y_prime_shares.sort();
        self.set_y_prime_all_parties(DVector::from_vec(y_prime_shares));

        let y_prime = AdditiveSecretSharing::reveal(&self.get_y_prime_all_parties(), self.params.d + 1);

        let y_prime = y_prime.to_usize().unwrap();
        let u = self.get_ltz()[y_prime].clone();

        self.set_u(u);

        let mut e = BigInt::zero();

        let neg_r = self.get_r()
            .neg()
            .mod_floor(&self.params.q);

        if self.party_number == 0 {
            e.add_assign(self.get_z_prime());
        }

        e.add_assign(&neg_r);

        let big_l_mul_u = self.get_u()
            .mul(&self.params.big_l); //.mod_floor(&self.params.p);

        e.add_assign(&big_l_mul_u);

        self.set_e(e.clone());


        let neg_e = self.get_e()
            .neg()
            .mod_floor(&self.params.q);

        let o_prime = self.get_z().add(&neg_e).mod_floor(&self.params.q);

        self.set_o_prime(o_prime.clone());


        let start_time = std::time::Instant::now();


        let output = ProtocolTransferredData {
            preprocessed: None,
            a: None,
            b: None,
            z_prime: None,
            y_prime: None,
            o_prime: Some(serialize(&o_prime).unwrap()),

            // alpha: None,
            mac_alpha: None,
            mac_r: None,
            // mac_x_tilde_collection: None,
            // mac_m_tilde_collection: None,
            mac_chi_vals: None,
            mac_z: None
        };

        let elapsed = start_time.elapsed();

        let _microseconds = elapsed.as_micros();

        //debug!("Serialize traffic from 'participant {}' to other participants' = {_microseconds} microseconds", self.party_number);


        output
    }

    pub fn execute_step_four(&mut self, input: Vec<ProtocolTransferredData>) -> ProtocolTransferredData {

        //debug!("execute_step_four {:?}", self);

        let mut o_prime_shares = Vec::new();
        o_prime_shares.push(self.get_o_prime().clone());
        for share in input {
            let o_prime_share: BigInt = deserialize(&share.o_prime.unwrap()).unwrap();
            o_prime_shares.push(o_prime_share);
        }

        o_prime_shares.sort();
        self.set_o_prime_all_parties(DVector::from_vec(o_prime_shares));

        let o_prime = AdditiveSecretSharing::reveal(self.get_o_prime_all_parties(), self.params.k);

        //debug!("o_prime = {o_prime}");

        let msg = round_div(&o_prime, &self.params.big_l);

        debug!("Party {} msg = {msg}", self.party_number);

        let t =  3;

        // MAC scheme

        let x_shares_collection = DMatrix::from_fn(self.params.n, t,|i, j| {
            match j {
                0 => self.get_z_prime_all_parties()[i].clone(),
                1 => self.get_y_prime_all_parties()[i].clone(),
                2 => self.get_o_prime_all_parties()[i].clone(),
                _ => BigInt::zero() // error
            }
        });


        // x_tilde_shares_collection [n rows, t columns]

        let x_tilde_shares_collection = DMatrix::from_fn(self.params.n, t,|i, j| {
            &x_shares_collection[(i,j)] + (&self.get_mac_r()[j] * self.params.mac_big_k.clone())
        });


        // let m_tilde_collection = DMatrix::from_fn(self.params.n, t, |i ,j|{
        //     let val = self.get_alpha() * &x_tilde_shares_collection[(i, j)];
        //     val.mod_floor(&self.params.mac_big_ks)
        // });

        let x_tilde_shares = x_tilde_shares_collection.row_sum()
            .map(|val| val.mod_floor(&self.params.mac_big_ks));


        let y_tilde = self.get_mac_chi_values().dot(&x_tilde_shares.transpose())
            .mod_floor(&self.params.mac_big_ks);


        let party_x_tilde_macs = x_tilde_shares_collection.row(self.party_number)
            .map(|x| {
                let val = self.get_mac_alpha() * x;
                val.mod_floor(&self.params.mac_big_ks)
            });

        let m_tilde = self.get_mac_chi_values().dot(&party_x_tilde_macs.transpose()).mod_floor(&self.params.mac_big_ks);

        let z = (m_tilde - self.get_mac_alpha() * y_tilde).mod_floor(&self.params.mac_big_ks);


        self.set_mac_z(z.clone());


        // let combined_mac_shares = DVector::from_fn(self.params.n, |i,_| {
        //     let player_mac_shares = m_tilde_collection.row(i).transpose();
        //
        //     self.get_mac_chi_values().dot(&player_mac_shares).mod_floor(&self.params.mac_big_ks)
        // });
        //




        // let z = &combined_mac_shares - (&self.mac_alpha.clone().unwrap() * &y_tilde).mod_floor(&self.params.mac_big_ks);


        // let final_sum = z_shares.sum().mod_floor(&self.params.mac_big_ks);


        // let result = if z_shares.sum().mod_floor(&self.params.mac_big_ks) == BigInt::zero() {
        //     let values = DVector::from_fn(t, |i, _| {
        //         x_tilde_shares[i].mod_floor(&self.params.mac_big_k)
        //     });

            // Some(values)
        // }
        // else {
        //     None
        // };

        let output = ProtocolTransferredData {
            preprocessed: None,
            a: None,
            b: None,
            z_prime: None,
            y_prime: None,
            o_prime: None,
            // alpha: None,
            mac_alpha: None,
            mac_r: None,
            // mac_x_tilde_collection: None,
            // mac_m_tilde_collection: None,
            mac_chi_vals: None,
            mac_z: Some(serialize(&z).unwrap()),
        };

        //debug!("Serialize traffic from 'participant {}' to other participants' = {_microseconds} microseconds", self.party_number);


        output


    }


    pub fn execute_step_five(&mut self, input: Vec<ProtocolTransferredData>) -> ProtocolTransferredData {
        let mut z_sum = BigInt::zero();
        for i in input {

            let z: BigInt = deserialize(&i.mac_z.unwrap()).unwrap();

            z_sum.add_assign(z);
        }

        z_sum.add_assign(self.get_mac_z());





        ProtocolTransferredData::empty()
    }

}




impl Display for Party {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Party {}:\n\
            sk: {}\n\
            z: {}\n,
            s: {}\n\
            r: {}\n\
            ltz: {}\n\
            signs: {}\n\
            y: {}\n\
            u: {}\n\
            e: {}\n\
            alpha: {}\n\
            a: {}\n\
            b: {}\n\
            z_prime: {}\n\
            y_prime: {}\n\
            o_prime: {}\n\
            ",
            self.party_number,
            self.get_sk().transpose(),
            self.get_z(),
            self.get_s(),
            self.get_r(),
            self.get_ltz().transpose(),
            self.get_signs(),
            self.get_y(),
            self.get_u(),
            self.get_e(),
            self.get_mac_alpha(),
            self.get_a(),
            self.get_b(),
            self.get_z_prime(),
            self.get_y_prime(),
            self.get_o_prime()
        )
    }
}


