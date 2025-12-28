use std::time::Instant;
use nalgebra::{DMatrix, DVector};
use num_bigint::{BigInt, UniformBigInt};
use num_traits::Zero;
use rand::distributions::uniform::UniformSampler;
use threshold_decryption::mpc::mac_scheme::{AuthenticatedSharingScheme, MACSchemeParams};
use threshold_decryption::mpc::protocol::Protocol;
use threshold_decryption::mpc::public_params::PublicParameters;
use threshold_decryption::mpc::lwe_scheme::init_lwe_with_random_ptxt;

fn decrypt_with_macs(mut protocol: Protocol, mac_scheme: AuthenticatedSharingScheme, a: DVector<BigInt>, b: BigInt, ptxt: BigInt, alpha_shares: DVector<BigInt>) {

    let out = protocol.decrypt(a, b);

    assert_eq!(out, ptxt);

    let x_shares_collection = DMatrix::from_fn(protocol.params.n, 3,|i, j| {
        match j {
            0 => protocol.parties[i].get_z_prime().clone(),
            1 => protocol.parties[i].get_y_prime().clone(),
            2 => protocol.parties[i].get_o_prime().clone(),
            _ => BigInt::zero() // error
        }
    });

    let (x_tilde_shares, m_tilde_collection) = mac_scheme.batch_open(&x_shares_collection);

    let checked_values = mac_scheme.batch_check(&x_tilde_shares, &alpha_shares, &m_tilde_collection);
    assert!(checked_values.is_some());
}

fn main() {
    let mut bench_tuples = Vec::new();
    // Protocol: Number of parties
    for protocol_n in [4] {
        // Protocol: Ciphertext bit length
        for protocol_k in [64] {
            // Protocol: Plaintext bit length
            for protocol_m in [1, 2, 4] {
                // Protocol: "Digit" bit length
                for protocol_b in 5..9 {
                    // MACs: security parameter
                    for mac_s in [80] {
                        // LWE: sample length
                        for lwe_a_len in [777, 870, 1024] {
                            bench_tuples.push((protocol_n, protocol_k, protocol_m, protocol_b, mac_s, lwe_a_len));
                        }
                    }
                }
            }
        }
    }

    for (protocol_n, protocol_k, protocol_m, protocol_b, mac_s, lwe_a_len) in bench_tuples {
        let params = PublicParameters::init(protocol_n, protocol_k, protocol_m, protocol_b, lwe_a_len, mac_s);

        let mut rng = rand::thread_rng();

        let (lwe_scheme, ptxt, lwe_a, lwe_b, ) = init_lwe_with_random_ptxt(params.m, params.k, lwe_a_len, 1);

        let protocol_s = UniformBigInt::new(&BigInt::zero(), &BigInt::from(params.big_d)).sample(&mut rng);

        let protocol_r = UniformBigInt::new(&BigInt::zero(), &params.big_l).sample(&mut rng);

        let mac_alpha = UniformBigInt::new(&BigInt::zero(), &params.big_l).sample(&mut rng);

        let mac_scheme_params = MACSchemeParams::init(protocol_n, protocol_k, mac_s, 3);

        let mac_scheme = AuthenticatedSharingScheme::new(mac_alpha, mac_scheme_params);

        let mut protocol = Protocol::new(&params);

        let mac_alpha_shares = mac_scheme.share_global_key();

        protocol.preprocess(protocol_s, protocol_r);

        protocol.share_sk(lwe_scheme.sk);

        let start = Instant::now();

        decrypt_with_macs(protocol,
                          mac_scheme,
                          lwe_a,
                          lwe_b,
                          ptxt,
                          mac_alpha_shares);

        let duration = start.elapsed();

        let microseconds = duration.as_micros();

        println!("(Protocol n = {}, k = {}, m = {}, b = {}) (MACs s = {}) (LWE a = {}): {} microseconds",
                 protocol_n, protocol_k, protocol_m, protocol_b, mac_s, lwe_a_len, microseconds);
    }
}