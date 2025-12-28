use serde::{Serialize, Deserialize};

use std::net::{SocketAddr};
use crate::network::{ProtocolTransferredData};

#[derive(Serialize, Deserialize, Debug)]
pub enum Message {
    // To DiscoveryServer
    RegisterParticipant(String, SocketAddr),
    UnregisterParticipant(String),

    // From DiscoveryServer
    ParticipantList(Vec<(String, SocketAddr)>),
    ParticipantNotificationAdded(String, SocketAddr),
    ParticipantNotificationRemoved(String),

    // From Participant to Participant
    ProtocolStart,

    ProtocolExecuteStep(usize, usize, Vec<ProtocolTransferredData>, u64),


}

pub const DISCOVERY_SERVER: &str = "DISCOVERY_SERVER";

pub const STEP_COUNT: usize = 5;


