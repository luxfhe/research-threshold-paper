use serde::{Deserialize, Serialize};

pub mod participant;
pub mod discovery_server;
pub mod common;
pub mod worker;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolTransferredData {
    pub preprocessed: Option<Vec<u8>>,
    pub a: Option<Vec<u8>>,
    pub b: Option<Vec<u8>>,
    pub z_prime: Option<Vec<u8>>,
    pub y_prime: Option<Vec<u8>>,
    pub o_prime: Option<Vec<u8>>,

    // pub random_id: u64,

    // pub alpha: Option<Vec<u8>>,
    pub mac_alpha: Option<Vec<u8>>,
    pub mac_r: Option<Vec<u8>>,
    // pub mac_x_tilde_collection: Option<Vec<u8>>,
    // pub mac_m_tilde_collection: Option<Vec<u8>>,
    pub mac_chi_vals: Option<Vec<u8>>,
    pub mac_z: Option<Vec<u8>>,

}

impl ProtocolTransferredData {
    pub fn empty() -> ProtocolTransferredData {
        ProtocolTransferredData{
            preprocessed:None,

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
            mac_z: None

        }
    }
}