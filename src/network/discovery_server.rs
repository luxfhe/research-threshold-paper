use super::common::{Message};

use message_io::network::{NetEvent, Transport, Endpoint};
use message_io::node::{self, NodeHandler, NodeListener};

use std::net::{SocketAddr};
use std::collections::{HashMap};
use std::fs::{create_dir_all, File};
use std::io::{self, Write};
use std::path::Path;
use std::process::exit;
use std::str::FromStr;
use std::{thread};
use std::time::Duration;
use log::debug;
use nalgebra::{DMatrix, DVector};
use num_bigint::{BigInt, UniformBigInt};
use num_traits::Zero;
use rand::distributions::uniform::UniformSampler;
use crate::mpc::additive_sharing::AdditiveSecretSharing;
use crate::mpc::lwe_scheme::init_lwe_with_random_ptxt;
use crate::mpc::preprocessing::Preprocessing;
use crate::mpc::public_params::PublicParameters;
use crate::network::ProtocolTransferredData;

use bitcode::{serialize, deserialize};

struct ParticipantInfo {
    addr: SocketAddr,
    endpoint: Endpoint,
}

pub struct DiscoveryServer {
    handler: NodeHandler<()>,
    node_listener: Option<NodeListener<()>>,
    participants: HashMap<String, ParticipantInfo>,
    params: PublicParameters,
    preprocessing: Preprocessing,
    // start_time: Option<Instant>,
}

impl DiscoveryServer {
    pub fn new(public_parameters: &PublicParameters, preprocessing: &Preprocessing) -> io::Result<DiscoveryServer> {
        let (handler, node_listener) = node::split::<()>();

        let listen_addr = "127.0.0.1:5000";
        handler.network().listen(Transport::FramedTcp, listen_addr)?;

        //debug!("Discovery server running at {}", listen_addr);

        Ok(DiscoveryServer {
            handler,
            node_listener: Some(node_listener),
            participants: HashMap::new(),
            preprocessing: preprocessing.clone(),
            params: public_parameters.clone(),
            // start_time: None,
        })
    }


    pub fn run(mut self) {
        let node_listener = self.node_listener.take().unwrap();
        node_listener.for_each(move |event| match event.network() {
            NetEvent::Connected(_, _) => unreachable!(), // There is no connect() calls.
            NetEvent::Accepted(_, _) => (),              // All endpoint accepted
            NetEvent::Message(endpoint, input_data) => {
                let message: Message = deserialize(&input_data).unwrap();
                match message {
                    Message::RegisterParticipant(name, addr) => {
                        self.register(&name, addr, endpoint);
                    }
                    Message::UnregisterParticipant(name) => {
                        self.unregister(&name);
                    }
                    _ => unreachable!(),
                }
            }
            NetEvent::Disconnected(endpoint) => {
                // Participant disconnection without explict unregistration.
                // We must remove from the registry too.
                let participant =
                    self.participants.iter().find(|(_, info)| info.endpoint == endpoint);

                if let Some(participant) = participant {
                    let name = participant.0.to_string();
                    self.unregister(&name);
                }
            }
        });
    }

    fn register(&mut self, name: &str, addr: SocketAddr, endpoint: Endpoint) {
        if !self.participants.contains_key(name) {
            // Update the new participant with the whole participants information
            let list =
                self.participants.iter().map(|(name, info)| (name.clone(), info.addr)).collect();

            let message: Message = Message::ParticipantList(list);
            let output_data = serialize(&message).unwrap();
            self.handler.network().send(endpoint, &output_data);

            // Notify other participants about this new participant
            let message : Message = Message::ParticipantNotificationAdded(name.to_string(), addr);
            let output_data = serialize(&message).unwrap();
            for (_, info) in &mut self.participants {
                self.handler.network().send(info.endpoint, &output_data);
            }

            // Register participant
            self.participants.insert(name.to_string(), ParticipantInfo { addr, endpoint });
            //debug!("Added participant '{}' with ip {}", name, addr);

            if self.participants.len() == self.params.n {
                thread::sleep(Duration::from_millis(100));

                // self.start_time = Some(std::time::Instant::now());


                let (lwe_scheme, _ptxt, a, b) = init_lwe_with_random_ptxt(self.params.m, self.params.k, self.params.lwe_dimension, 1);


                println!("_ptxt = {_ptxt}");
                let mut rng = rand::thread_rng();
                let s = UniformBigInt::new(&BigInt::zero(), &BigInt::from(self.params.big_d)).sample(&mut rng);
                let r = UniformBigInt::new(&BigInt::zero(), &self.params.big_l).sample(&mut rng);

                let preprocessing_shares = self.preprocessing.run(s, r, lwe_scheme.sk);
                let a = serialize(&a).unwrap();
                let b = serialize(&b).unwrap();

                let alpha = UniformBigInt::new(&BigInt::zero(), &self.params.mac_big_ks).sample(&mut rng);
                let mac_alpha_shares = AdditiveSecretSharing::share(&alpha, self.params.n, self.params.mac_ks);

                let mut mac_r_shares_collection = Vec::new();
                let t = 3;
                for _i in 0..t {
                    let r = UniformBigInt::new(BigInt::zero(), &self.params.mac_big_s).sample(&mut rng);
                    let r_shares = AdditiveSecretSharing::share(&r, self.params.n, self.params.mac_s);

                    let _ = r_shares.push(r.clone());

                    mac_r_shares_collection.push(r_shares);
                }

                let mac_r_shares_collection = DMatrix::from_columns(&mac_r_shares_collection);

                let chi_vals = DVector::from_fn(t, |_i, _| {
                    UniformBigInt::new(BigInt::zero(), &self.params.mac_big_s).sample(&mut rng)
                });

                // Set up a directory for participant data files
                let dir_path = Path::new("/tmp/participant_data");
                create_dir_all(dir_path).expect("Failed to create participant data directory");

                // For each participant, prepare data and save it to a unique file
                for (participant_name, info) in self.participants.iter() {
                    let i = usize::from_str(participant_name.as_str()).unwrap();
                    let mac_r_shares: DVector<BigInt> = mac_r_shares_collection.row(i).transpose().into();

                    // Create participant-specific data
                    let participant_data = ProtocolTransferredData {
                        preprocessed: Some(serialize(&preprocessing_shares[i]).unwrap()),
                        a: Some(a.clone()),
                        b: Some(b.clone()),
                        z_prime: None,
                        y_prime: None,
                        o_prime: None,
                        // alpha: Some(serialize(&alpha).unwrap()),
                        mac_alpha: Some(serialize(&mac_alpha_shares[i]).unwrap()),
                        mac_r: Some(serialize(&mac_r_shares).unwrap()),
                        // mac_x_tilde_collection: None,
                        // mac_m_tilde_collection: None,
                        mac_chi_vals: Some(serialize(&chi_vals).unwrap()),
                        mac_z: None,
                    };

                    // Serialize and write the data to a file for this participant
                    let file_path = dir_path.join(format!("{}.bin", participant_name));
                    let mut file = File::create(&file_path).expect("Failed to create participant data file");
                    file.write_all(&serialize(&participant_data).unwrap())
                        .expect("Failed to write participant data to file");

                   debug!("Data for participant '{}' written to file {:?}", participant_name, file_path);

                    // Send a notification message to each participant to load data from the file
                    let message: Message = Message::ProtocolStart;
                    let output_data = serialize(&message).unwrap();
                    self.handler.network().send(info.endpoint, &output_data);
                }
            }
        }
        else {
            //debug!("Participant with name '{}' already exists, please registry with another name",name);
        }
    }



    fn unregister(&mut self, name: &str) {
        if let Some(_info) = self.participants.remove(name) {
            // Notify other participants about this removed participant
            let message: Message = Message::ParticipantNotificationRemoved(name.to_string());
            let output_data = serialize(&message).unwrap();
            for participant in &mut self.participants {
                self.handler.network().send(participant.1.endpoint, &output_data);
            }
            //debug!("Removed participant '{}' with ip {}", name, info.addr);

            if self.participants.len() == 0 {
                // let elapsed = self.start_time.unwrap().elapsed();

                // let _microseconds = elapsed.as_micros();

                //debug!("All unregistered (n = {}, k = {}, m = {}, b = {}) (MACs s = {}) (LWE a = {}): {} microseconds",
                //          self.params.n, self.params.k, self.params.m, self.params.b, self.params.mac_s, self.params.lwe_dimension, _microseconds);

                exit(0);

            }
        }
        else {
            //debug!("Can not unregister an non-existent participant with name '{}'", name);
        }
    }
}