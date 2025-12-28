use nalgebra::{DMatrix, DVector};
use num_bigint::BigInt;
use num_traits::{ToPrimitive, Zero};
use serde::{Deserialize, Serialize};
use crate::mpc::additive_sharing::AdditiveSecretSharing;
use crate::mpc::base_decomposition::BaseDecomposition;
use crate::mpc::preprocessed_gate::{LessThanZeroFunction, PreprocessedGate, SignFunction};
use crate::mpc::public_params::PublicParameters;

#[derive(Clone, Default, PartialEq)]
pub struct Preprocessing {
    pub params: PublicParameters,
}


#[derive(Clone, Serialize, Deserialize)]
pub struct PreprocessedShare {
    pub s: BigInt,
    pub r: BigInt,
    pub sk: DVector<BigInt>,
    pub ltz: DVector<BigInt>,
    pub signs: DMatrix<BigInt>,     // rows = B =  2^b = 2^(Digit bit length);   columns = d = Number of digits = ceil(l/b)
}

impl Preprocessing {
    pub fn new(params: &PublicParameters) -> Preprocessing {
        Preprocessing {
            params: params.clone()
        }
    }

    pub fn run(&self, s: BigInt, r: BigInt, sk: DVector<BigInt>) -> Vec<PreprocessedShare> {

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

        let mut sk_shares_per_party = DMatrix::<BigInt>::zeros(sk.nrows(), self.params.n);
        for (i, sk_digit) in sk.iter().enumerate() {

            // Share each digit of secret key [sk]_k
            let sk_digit_shares = AdditiveSecretSharing::share(sk_digit, self.params.n, self.params.k);
            sk_shares_per_party.set_row(i, &sk_digit_shares.transpose());
        }

        let mut shares = Vec::new();
        for i in 0..self.params.n {
            let share = PreprocessedShare {
                s: s_shares[i].clone(),
                r: r_shares[i].clone(),
                sk: sk_shares_per_party.column(i).into(),
                ltz: ltz_gate.get_party_shares(i),
                signs: sign_gates_per_party[i].clone(),
            };

            shares.push(share)
        }

        shares
    }
}

