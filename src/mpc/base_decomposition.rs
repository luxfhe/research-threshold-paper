use nalgebra::DVector;
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::Zero;

pub struct BaseDecomposition {
    pub base: usize,
}

impl BaseDecomposition {

    /// result vector is from LSD to MSD
    pub fn decompose(&self, value: &BigInt) -> DVector<BigInt> {

        let mut digits = Vec::new();
        let mut result = value.clone();
        while result > BigInt::zero() {
            let (quotient, remainder) = result.div_rem(&BigInt::from(self.base));
            digits.push(remainder);
            result = quotient;

        }

        DVector::from_vec(digits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigInt;


    #[test]
    fn test_digits_in_base_decimal() {
        let r = BigInt::from(123456789);
        let base = 10;
        let converter = BaseDecomposition { base };

        let mut expected_digits = vec![
            BigInt::from(1),
            BigInt::from(2),
            BigInt::from(3),
            BigInt::from(4),
            BigInt::from(5),
            BigInt::from(6),
            BigInt::from(7),
            BigInt::from(8),
            BigInt::from(9),
        ];

        expected_digits.reverse();
        let expected_digits = DVector::from_vec(expected_digits);
        assert_eq!(converter.decompose(&r), expected_digits);
    }

    #[test]
    fn test_digits_in_base_binary() {
        let r = BigInt::from(13);
        let base = 2;
        let expected_digits = vec![1, 1, 0, 1]; // Binary representation of 13 is 1101
        let mut expected_digits: Vec<BigInt> = expected_digits.into_iter().map(BigInt::from).collect();

        let converter = BaseDecomposition { base };

        expected_digits.reverse();
        let expected_digits = DVector::from_vec(expected_digits);

        assert_eq!(converter.decompose(&r), expected_digits);
    }

    #[test]
    fn test_digits_in_base_hexadecimal() {
        let r = BigInt::from(255);
        let base = 16;
        let expected_digits = vec![15, 15]; // Hexadecimal representation of 255 is FF
        let mut expected_digits: Vec<BigInt> = expected_digits.into_iter().map(BigInt::from).collect();

        let converter = BaseDecomposition { base };

        expected_digits.reverse();
        let expected_digits = DVector::from_vec(expected_digits);

        assert_eq!(converter.decompose(&r), expected_digits);
    }

    #[test]
    fn test_digits_in_base_zero() {
        let r = BigInt::zero();
        let base = 10;
        let expected_digits = vec![];

        let converter = BaseDecomposition { base };

        let expected_digits = DVector::from_vec(expected_digits);

        assert_eq!(converter.decompose(&r), expected_digits);
    }
}
