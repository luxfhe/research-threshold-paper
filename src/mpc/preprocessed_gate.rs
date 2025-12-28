use nalgebra::{DMatrix, DVector};
use num_bigint::{BigInt, Sign};
use num_integer::Integer;
use num_traits::{One, Zero};
use crate::mpc::additive_sharing::AdditiveSecretSharing;

pub trait ProcessingFunction {
    fn apply(&self, index: &BigInt, secret: &BigInt) -> BigInt;
}

pub struct SignFunction;

impl ProcessingFunction for SignFunction {
    fn apply(&self, value: &BigInt, secret: &BigInt) -> BigInt {
        match (value - secret).sign() {
            Sign::Minus => BigInt::from(-1),
            Sign::NoSign => BigInt::from(0),
            Sign::Plus => BigInt::from(1),
        }
    }
}

pub struct LessThanZeroFunction {
    pub(crate) modulo: BigInt
}

impl ProcessingFunction for LessThanZeroFunction {
    fn apply(&self, value: &BigInt, secret: &BigInt) -> BigInt {
        let diff = (value - secret).mod_floor(&self.modulo);

        let half_modulo = &self.modulo >> 1;

        if diff.ge(&half_modulo) {
            BigInt::from(1)
        }
        else {
            BigInt::from(0)
        }
    }
}

// pub struct EqualsFunction;
//
// impl ProcessingFunction for EqualsFunction {
//     fn apply(value: &BigInt, secret: &BigInt) -> BigInt {
//         match (value - secret).sign() {
//             Sign::NoSign => BigInt::from(1),
//             Sign::Minus | Sign::Plus => BigInt::from(0),
//         }
//     }
// }





pub struct PreprocessedGate<F: ProcessingFunction> {
    pub truth_table: DMatrix<BigInt>,
    _func: F
}


impl <F: ProcessingFunction> PreprocessedGate<F> {
    pub fn build(func: F, secret: BigInt, num_parties: usize, num_rows: usize, field_exponent: usize) -> Self {

        let mut truth_table = DMatrix::<BigInt>::zeros(num_rows, num_parties);

        let mut iter = BigInt::zero();
        let one = BigInt::one();

        // 2^(num_bits) rows, num_parties
        for i in 0..num_rows {
            let value_to_share = func.apply(&iter, &secret);

            let row = AdditiveSecretSharing::share(&value_to_share, num_parties, field_exponent).transpose();
            truth_table.set_row(i, &row);

            iter += &one;
        }


        PreprocessedGate {
            truth_table,
            _func: func
        }
    }

    pub fn get_party_shares(&self, party_index: usize) -> DVector<BigInt>{
        let column: DVector<BigInt> = self.truth_table.column(party_index).into();  // Convert into DVector
        column
        // self.truth_table.column(party_index)
        // AdditiveSecretSharing::reconstruct(&row)

    }

    pub fn get_table_index_shares(&self, table_index: usize) -> DVector<BigInt> {
        let row: DVector<BigInt> = self.truth_table.row(table_index).transpose().into();  // Convert into DVector
        row
    }
}


#[cfg(test)]
mod tests {
    use num_integer::Integer;
    use num_traits::ToPrimitive;
    use crate::mpc::public_params::PublicParameters;
    use super::*;

    #[test]
    fn test_sign_gate() {
        let s = BigInt::from(5);
        let params = PublicParameters::default();

        let gate = PreprocessedGate::build(SignFunction, s.clone(), params.n, params.big_b, params.d + 1);

        let field_exponent = params.d as u32 + 1;
        let field_size = BigInt::from(2u32).pow(field_exponent);

        let minus_one = BigInt::from(-1).mod_floor(&field_size);


        //debug!("{}", gate.truth_table);
        let s = s.to_usize().unwrap();
        for i in 0..params.big_b {
            //debug!("index {i}");
            let index_shares = gate.get_table_index_shares(i);
            let reconstruct = AdditiveSecretSharing::reveal(&index_shares, field_exponent as usize);

            if i == s {
                assert_eq!(reconstruct, BigInt::from(0));
            }
            else if i < s {
                assert_eq!(reconstruct, minus_one);
            }
            else if i > s {
                assert_eq!(reconstruct, BigInt::from(1));
            }
        }
    }


    #[test]
    fn test_less_than_zero_gate() {
        let s = BigInt::from(5);

        let params = PublicParameters::default();


        let big_d = BigInt::from(params.big_d);
        let ltz_func = LessThanZeroFunction {
            modulo: big_d
        };

        let gate = PreprocessedGate::build(ltz_func, s.clone(), params.n, params.big_d.to_usize().unwrap(), params.m);

        let s = s.to_usize().unwrap();
        for i in 0..params.n {

            let index_shares = gate.get_table_index_shares(i);
            let reconstruct = AdditiveSecretSharing::reveal(&index_shares, params.m);

            if i < s {
                assert_eq!(reconstruct, BigInt::from(1));
            }
            else if i <= s {
                assert_eq!(reconstruct, BigInt::from(0));
            }
        }
    }
}