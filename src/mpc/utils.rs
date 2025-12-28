use std::ops::Mul;
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Signed};


#[macro_export]
macro_rules! generate_getters_and_setters {
    ($struct_name:ident, $( $field_name:ident : $field_type:ty ),* ) => {
        impl $struct_name {
            paste! {
                $(
                    // Setter
                    pub fn [<set_ $field_name>](&mut self, value: $field_type) -> &mut Self {
                        self.$field_name = Some(value);
                        self
                    }

                    // Getter with expect
                    pub fn [<get_ $field_name>](&self) -> &$field_type {
                        self.$field_name.as_ref().expect(
                            format!(
                                "Field '{}' is None for {} with party_number {}",
                                stringify!($field_name),
                                stringify!($struct_name),
                                self.party_number
                            ).as_str()
                        )
                    }
                )*
            }
        }
    };
}

/*
pub trait ResidueOperations {
    fn signed_residue(&self, q: &BigInt) -> BigInt;
    fn unsigned_residue(&self, q: &BigInt) -> BigInt;
    fn signed_to_unsigned_residue(&self, q: &BigInt) -> BigInt;
    fn unsigned_to_signed_residue(&self, q: &BigInt) -> BigInt;
}

impl ResidueOperations for BigInt {
    fn signed_residue(&self, q: &BigInt) -> BigInt {
        let q_abs = q.abs();
        let r = (self.rem(&q_abs) + &q_abs).rem(q);
        if &r >= &(&q_abs / &BigInt::from(2)) {
            r - &q_abs
        } else {
            r
        }
    }

    fn unsigned_residue(&self, q: &BigInt) -> BigInt {
        let q_abs = q.abs();
        (self.rem(&q_abs) + &q_abs).rem(&q_abs)
    }

    fn signed_to_unsigned_residue(&self, q: &BigInt) -> BigInt {
        if self < &BigInt::zero() {
            self + q
        } else {
            self.clone()
        }
    }

    fn unsigned_to_signed_residue(&self, q: &BigInt) -> BigInt {
        let half_q = q.div(2);
        if self > &half_q {
            self - q
        } else {
            self.clone()
        }
    }
}



 */






pub trait ToBits {
    fn to_bits(&self, bits: usize) -> String;
}

impl ToBits for BigInt {
    fn to_bits(&self, bits: usize) -> String {
        // Get the absolute value and convert to binary string
        let mut binary_str = format!("{:b}", self.abs());

        // If the number has more bits than requested, truncate it to `bits` length
        if binary_str.len() > bits {
            binary_str = binary_str[binary_str.len() - bits..].to_string();
        }

        // If the number has fewer bits, pad it with leading zeros
        if binary_str.len() < bits {
            let padding = "0".repeat(bits - binary_str.len());
            binary_str = format!("{}{}", padding, binary_str);
        }

        // If the number is negative, we can extend this to support two's complement later
        // but for now, we'll return the absolute value's binary representation.

        format!("{} [{}]", self.abs(), binary_str)
    }
}

pub trait ToBitSegments {
    fn to_bit_segments(&self, upper_bits: usize, lower_bits: usize) -> String;
}

impl ToBitSegments for BigInt {
    fn to_bit_segments(&self, upper_bits: usize, lower_bits: usize) -> String {
        let total_bits = upper_bits + lower_bits;

        // Convert to a binary string with the desired total bit length
        let mut binary_str = format!("{:b}", self.abs());

        // If the binary representation is longer than the total bits, truncate it
        if binary_str.len() > total_bits {
            binary_str = binary_str[binary_str.len() - total_bits..].to_string();
        }

        // Pad with leading zeros if the binary string is too short
        if binary_str.len() < total_bits {
            let padding = "0".repeat(total_bits - binary_str.len());
            binary_str = format!("{}{}", padding, binary_str);
        }

        // Split into upper and lower parts
        let upper_part = &binary_str[..upper_bits];
        let lower_part = &binary_str[upper_bits..];

        // Combine with the "|" separator
        format!("{} [{}|{}]", self.abs() ,upper_part, lower_part)
    }
}


pub fn print_binary_with_bits(x: &BigInt, b: usize) -> String {

    let bytes = x.to_biguint().unwrap().to_bytes_be();


    let mut binary_string = bytes.iter()
        .map(|byte| format!("{:08b}", byte))
        .collect::<String>();


    if binary_string.len() > b {
        binary_string = binary_string[binary_string.len() - b..].to_string();
    }


    if binary_string.len() < b {
        let padding = "0".repeat(b - binary_string.len());
        binary_string = format!("{}{}", padding, binary_string);
    }

    binary_string
}


pub fn round_div(x: &BigInt, q: &BigInt) -> BigInt {
    let (quotient, remainder) = &x.div_rem(q);
    let double_remainder = &remainder.mul(&BigInt::from(2));

    if double_remainder.abs().gt(q) || (double_remainder.abs().eq(q) && remainder.is_positive()) {
        if remainder.is_positive() {
            quotient + BigInt::one()
        } else {
            quotient - BigInt::one()
        }
    } else {
        quotient.clone()
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigInt;
    use num_traits::{Zero};

    #[test]
    fn test_round_div_positive_numbers() {
        let x = BigInt::from(7);
        let q = BigInt::from(3);
        let result = round_div(&x, &q);
        assert_eq!(result, BigInt::from(2));
    }

    #[test]
    fn test_round_div_negative_numbers() {
        let x = BigInt::from(-7);
        let q = BigInt::from(3);
        let result = round_div(&x, &q);
        assert_eq!(result, BigInt::from(-2));
    }

    #[test]
    fn test_round_div_round_up() {
        let x = BigInt::from(8);
        let q = BigInt::from(3);
        let result = round_div(&x, &q);
        assert_eq!(result, BigInt::from(3));
    }

    #[test]
    fn test_round_div_round_down() {
        let x = BigInt::from(4);
        let q = BigInt::from(3);
        let result = round_div(&x, &q);
        assert_eq!(result, BigInt::from(1));
    }

    #[test]
    fn test_round_div_exact_division() {
        let x = BigInt::from(6);
        let q = BigInt::from(3);
        let result = round_div(&x, &q);
        assert_eq!(result, BigInt::from(2));
    }

    #[test]
    fn test_round_div_zero_dividend() {
        let x = BigInt::zero();
        let q = BigInt::from(3);
        let result = round_div(&x, &q);
        assert_eq!(result, BigInt::zero());
    }

    #[test]
    fn test_round_div_half_q_round_up() {
        let x = BigInt::from(5);
        let q = BigInt::from(4);
        let result = round_div(&x, &q);
        assert_eq!(result, BigInt::from(1));
    }

    #[test]
    fn test_round_div_half_q_round_down() {
        let x = BigInt::from(-5);
        let q = BigInt::from(4);
        let result = round_div(&x, &q);
        assert_eq!(result, BigInt::from(-1));
    }

    /*
    #[test]
    fn test_residue_in_0_q() {
        // Case: x > 0, q > 0
        assert_eq!(BigInt::from(7).unsigned_residue(&BigInt::from(5)), BigInt::from(2));

        // Case: x < 0, q > 0
        assert_eq!(BigInt::from(-7).unsigned_residue(&BigInt::from(5)), BigInt::from(3));

        // Case: x > 0, q < 0 (negative modulus doesn't make sense mathematically, but we handle)
        assert_eq!(BigInt::from(7).unsigned_residue(&BigInt::from(-5)), BigInt::from(2));

        // Case: x < 0, q < 0
        assert_eq!(BigInt::from(-7).unsigned_residue(&BigInt::from(-5)), BigInt::from(3));

        // Case: x = 0, q > 0
        assert_eq!(BigInt::from(0).unsigned_residue(&BigInt::from(5)), BigInt::from(0));

        // Case: x = 0, q < 0
        assert_eq!(BigInt::from(0).unsigned_residue(&BigInt::from(-5)), BigInt::from(0));

        // Case: x = q (boundary)
        assert_eq!(BigInt::from(5).unsigned_residue(&BigInt::from(5)), BigInt::from(0));

        // Case: x = -q (boundary)
        assert_eq!(BigInt::from(-5).unsigned_residue(&BigInt::from(5)), BigInt::from(0));
    }

    #[test]
    fn test_residue_in_half_range() {
        // Case: x > 0, q > 0
        assert_eq!(BigInt::from(7).signed_residue(&BigInt::from(5)), BigInt::from(2));

        // Case: x < 0, q > 0
        assert_eq!(BigInt::from(-7).signed_residue(&BigInt::from(5)), BigInt::from(-2));

        // Case: x > 0, q < 0 (handling negative q)
        assert_eq!(BigInt::from(7).signed_residue(&BigInt::from(-5)), BigInt::from(2));

        // Case: x < 0, q < 0
        assert_eq!(BigInt::from(-7).signed_residue(&BigInt::from(-5)), BigInt::from(-2));

        // Case: x = 0, q > 0
        assert_eq!(BigInt::from(0).signed_residue(&BigInt::from(5)), BigInt::from(0));

        // Case: x = 0, q < 0
        assert_eq!(BigInt::from(0).signed_residue(&BigInt::from(-5)), BigInt::from(0));

        // Case: x = q/2 (boundary)
        assert_eq!(BigInt::from(2).signed_residue(&BigInt::from(5)), BigInt::from(2));

        // Case: x = -q/2 (boundary)
        assert_eq!(BigInt::from(-2).signed_residue(&BigInt::from(5)), BigInt::from(-2));

        // Case: x = q (boundary)
        assert_eq!(BigInt::from(5).signed_residue(&BigInt::from(5)), BigInt::from(0));

        // Case: x = -q (boundary)
        assert_eq!(BigInt::from(-5).signed_residue(&BigInt::from(5)), BigInt::from(0));
    }

     */

}

