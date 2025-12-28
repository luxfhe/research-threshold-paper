use super::common::Message;
use message_io::network::{NetEvent, Transport, Endpoint, SendStatus};
use message_io::node::{self, NodeHandler, NodeListener};
use std::net::SocketAddr;
use std::collections::HashMap;
use dashmap::DashMap;

use std::io::{self, Read};
use std::sync::{Arc, Mutex, RwLock};
use std::{fs, thread};
use std::time::Duration;
use log::debug;
use rayon::prelude::*;
use rayon::{ThreadPool, ThreadPoolBuilder};
use crate::mpc::public_params::PublicParameters;
use crate::network::{ProtocolTransferredData};
use crate::network::worker::{handle_protocol_execute_step, handle_protocol_start, Worker};

use bitcode::serialize as serialize;
use bitcode::deserialize as deserialize;

#[derive(Debug, Deserialize)]
struct ParticipantConfig {
    thread_count: usize,
    ctxt_per_job: usize,
    jobs_per_worker: usize,
}

fn load_config(path: &str) -> ParticipantConfig {
    let config_content = fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("Failed to read the configuration file: {}", path));
    toml::from_str(&config_content)
        .unwrap_or_else(|_| panic!("Failed to parse the configuration file: {}", path))
}

pub struct NetworkListener {
    node_listener: NodeListener<()>,
}

pub struct NetworkSender {
    handler: NodeHandler<()>,
    discovery_endpoint: Endpoint,
    public_addr: SocketAddr,
}

impl NetworkSender {
    pub fn send(&self, endpoint: Endpoint, data: &[u8]) -> SendStatus {
        self.handler.network().send(endpoint, data)
    }
}

pub fn init_network() -> (NetworkSender, NetworkListener) {
    let (handler, node_listener) = node::split();
    let listen_addr = "127.0.0.1:0";
    let (_, listen_addr) = handler.network().listen(Transport::FramedTcp, listen_addr).unwrap();

    let discovery_addr = "127.0.0.1:5000";
    let (endpoint, _) = handler.network().connect(Transport::FramedTcp, discovery_addr).unwrap();

    (NetworkSender {
        handler,
        discovery_endpoint: endpoint,
        public_addr: listen_addr,
    },
     NetworkListener {
        node_listener,
    })
}

use lazy_static::lazy_static;
use serde::Deserialize;

lazy_static! {
    static ref NODE_LISTENER: Mutex<Option<Box<NetworkListener>>> = Mutex::new(None);
}

lazy_static! {
    static ref NETWORK_SENDER: Mutex<Option<Box<NetworkSender>>> = Mutex::new(None);
}

lazy_static! {
    static ref known_participants: RwLock<HashMap<String, Endpoint>> = RwLock::new(HashMap::new());
}

pub struct Participant {
    id: usize,
    greetings: HashMap<Endpoint, String>,
    // workers: Vec<Worker>,
    job_data: Arc<DashMap<u64, Worker>>,
    thread_pool: ThreadPool,
    public_parameters: PublicParameters,

    config: ParticipantConfig
}

impl Participant {
    pub fn new(id: usize, params: &PublicParameters) -> io::Result<Participant> {
        let config: ParticipantConfig = load_config("participant_config.toml");
        let job_data = DashMap::new();


        let thread_pool = ThreadPoolBuilder::new().num_threads(config.thread_count).build().unwrap();

        let (sender, listener) = init_network();

        {
            let mut network_sender = NETWORK_SENDER.lock().unwrap();
            *network_sender = Some(Box::new(sender));
        }

        {
            let mut node_listener = NODE_LISTENER.lock().unwrap();
            *node_listener = Some(Box::new(listener));
        }

        debug!("Done initialized network");
        // ********

        /// Load configuration from a file



        Ok(Participant {
            id,
            greetings: HashMap::new(),
            public_parameters: params.clone(),
            job_data: job_data.into(),
            thread_pool,
            config
        })
    }

    pub fn run(mut self) {

        let handler = NODE_LISTENER.lock().unwrap().take().unwrap();
        handler.node_listener.for_each(move |event| {
            match event.network() {

                NetEvent::Connected(endpoint, established) => {
                    let mut network_sender = NETWORK_SENDER.lock().unwrap();
                    let sender_mut = network_sender.as_mut().unwrap();

                    if endpoint == sender_mut.discovery_endpoint {
                        if established {
                           debug!("Connected to discovery server. Registering participant {}", self.id);
                            let message = Message::RegisterParticipant(self.id.to_string(), sender_mut.public_addr);
                            let output_data = serialize(&message).unwrap();
                            sender_mut.handler.network().send(sender_mut.discovery_endpoint, &output_data);
                        }
                        else {
                           debug!("Can not connect to the discovery server");
                        }
                    } else {
                        let name = self.greetings.remove(&endpoint).unwrap();
                        if established {
                            let mut participants_lock = known_participants.write().unwrap();
                            participants_lock.insert(name.clone(), endpoint);
                        }
                    }
                }

                NetEvent::Message(_endpoint, input_data) => {
                    let message: Message = match deserialize(&input_data) {
                        Ok(msg) => msg,
                        Err(e) => {
                            eprintln!("Failed to deserialize message: {}", e);
                            return;
                        }
                    };

                   // debug!("Deserialized message of type {:?}", message);
                    match message {
                        Message::ParticipantList(participants) => {
                            //debug!("Participant list received ({} participants)", participants.len());
                            for (name, addr) in participants {
                                self.discovered_participant(&name, addr);
                            }
                        }
                        Message::ParticipantNotificationAdded(other_participant_name, addr) => {
                            //debug!("New participant '{}' in the network", other_participant_name);
                            self.discovered_participant(&other_participant_name, addr);
                        }
                        Message::ParticipantNotificationRemoved(other_participant_name) => {
                            //debug!("Removed participant '{}' from the network", other_participant_name);
                            let mut participants_lock = known_participants.write().unwrap();
                            let mut network_sender = NETWORK_SENDER.lock().unwrap();
                            let sender_mut = network_sender.as_mut().unwrap();
                            if let Some(endpoint) = participants_lock.remove(&other_participant_name) {
                                sender_mut.handler.network().remove(endpoint.resource_id());
                            }
                        }
                        Message::ProtocolStart => {
                            for batch in 0..self.config.jobs_per_worker as u64 {

                                let job_data = Arc::clone(&self.job_data);
                                let params = self.public_parameters.clone();
                                self.thread_pool.spawn(move || {
                                    // Update job_data using DashMap's concurrent API
                                    let (worker, bulk_data) = handle_protocol_start(&params, self.id, self.config.ctxt_per_job).map_err(|e| {
                                        eprintln!("Worker failed to handle ProtocolStart: {}", e);
                                        let mut network_sender = NETWORK_SENDER.lock().unwrap();
                                        let sender_mut = network_sender.as_mut().unwrap();
                                        sender_mut.handler.stop();
                                    }).unwrap();
                                    job_data.insert(batch, worker);
                                    debug!("Worker batch {} started.", batch);
                                    // Send ProtocolExecuteStep to known participants for each worker
                                    send_result_to_everyone(&bulk_data, 0, batch as usize, self.id);
                                });
                            }
                        }

                        Message::ProtocolExecuteStep(participant_num, step_num, input_data, job_id) => {

                            let job_data = Arc::clone(&self.job_data);
                            self.thread_pool.spawn(move || {
                                // Update job_data using DashMap's concurrent API
                                // job_data.entry(job_id).and_modify(|worker| {
                                //     handle_protocol_execute_step(worker, job_id, self.id, participant_num, step_num, input_data);
                                // });

                                // in the multithreaded case it's possible the worker needs to receive data but we didn't even finish initalizing it yet
                                loop {
                                    if let Some(mut worker) = job_data.get_mut(&job_id) {
                                        handle_protocol_execute_step(&mut *worker, job_id, self.id, participant_num, step_num, input_data);
                                        break;
                                    } else {
                                        // Entry not found yet, wait before retrying
                                        thread::sleep(Duration::from_millis(10)); // Adjust delay as needed
                                    }
                                }
                            });

                            // self.thread_pool.spawn(|| {
                            //     handle_protocol_execute_step(&mut self.job_data, job_id, self.id, participant_num, step_num, input_data);
                            // });

                        }
                        _ => {}
                    }
                }

                NetEvent::Disconnected(endpoint) => {
                    let mut network_sender = NETWORK_SENDER.lock().unwrap();
                    let sender_mut = network_sender.as_mut().unwrap();
                    if endpoint == sender_mut.discovery_endpoint {
                       debug!("Disconnected from discovery server. Stopping handler.");
                        sender_mut.handler.stop();
                    }
                }
                _ => {}
            }
        });
    }

    fn discovered_participant(&mut self, name: &str, addr: SocketAddr) {
        let mut network_sender = NETWORK_SENDER.lock().unwrap();
        let sender_mut = network_sender.as_mut().unwrap();

        let (endpoint, _) = sender_mut.handler.network().connect(Transport::FramedTcp, addr).unwrap();
        self.greetings.insert(endpoint, name.into());
    }
}

pub fn send_result_to_everyone(data: &Vec<ProtocolTransferredData>, step: usize, job_id: usize, participant_id: usize) {
    let participants = known_participants.read().unwrap();
    let mut network_sender = NETWORK_SENDER.lock().unwrap();
    let sender_mut = network_sender.as_mut().unwrap();

    for (participant, info) in participants.iter() {
        debug!("JOB {}, Sending ProtocolExecuteStep {} to participant '{}'", job_id, step, participant);

        let message = Message::ProtocolExecuteStep(participant_id, step, data.clone(), job_id as u64);
        let output_data = match bitcode::serialize(&message) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("Failed to serialize ProtocolExecuteStep message for worker {}", e);
                continue;
            }
        };

        match sender_mut.handler.network().send(info.clone(), &output_data) {
            SendStatus::Sent => debug!("Successfully sent ProtocolExecuteStep to participant '{}'", participant),
            _ => eprintln!("Failed to send ProtocolExecuteStep to participant '{}'", participant),
        }
    }
}