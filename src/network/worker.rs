use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::Read;
use std::time::Instant;
use log::debug;
use nalgebra::DVector;
use num_bigint::BigInt;
use crate::mpc::party::Party;
use crate::mpc::preprocessing::PreprocessedShare;
use crate::mpc::public_params::PublicParameters;
use crate::network::{ProtocolTransferredData};
use crate::network::common::STEP_COUNT;
use crate::network::participant::{send_result_to_everyone};
use crate::network::worker::ExecutionResult::{Finished, NextStep, NoReady};

use bitcode::deserialize;

pub struct Worker {
    steps_bulk_data: HashMap<(usize, usize), Vec<ProtocolTransferredData>>,
    params: PublicParameters,
    mpc_decryptions: Vec<Party>,
    ctxt_per_job: usize,
    start_time: Option<Instant>,
    pub id: usize
}


pub enum ExecutionResult<T> {
    NoReady,
    NextStep(T),
    Finished,
}

impl Worker {
    pub fn new(id: usize, params: PublicParameters, ctxt_per_job: usize) -> Self {

        let mpc_decryptions: Vec<Party> = (0..ctxt_per_job)
            .map(|_| Party::new(id, &params))
            .collect();

        Worker {
            steps_bulk_data: HashMap::new(),
            params,
            mpc_decryptions,
            ctxt_per_job,
            start_time: None,
            id
        }
    }



}

pub fn handle_protocol_start(
    public_parameters: &PublicParameters,
    my_id: usize,
    ctxt_per_job: usize,
)
    -> Result<(Worker, Vec<ProtocolTransferredData>), io::Error> {

    debug!("Starting handle_protocol_start for participant ID: {}", my_id);

    let file_path = format!("/tmp/participant_data/{}.bin", my_id);
    debug!("Attempting to open participant data file: {}", file_path);
    let mut file = File::open(&file_path)?;

    let mut buffer = Vec::new();
    debug!("Reading file content into buffer...");
    file.read_to_end(&mut buffer)?;

    debug!("Deserializing ProtocolTransferredData...");
    let input_data: ProtocolTransferredData = match deserialize(&buffer) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Failed to deserialize ProtocolTransferredData: {}", e);
            return Err(io::Error::new(io::ErrorKind::InvalidData, e));
        }
    };

    debug!("Deserializing PreprocessedShare...");
    let preprocessed: PreprocessedShare = match deserialize(&input_data.preprocessed.clone().unwrap()) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Failed to deserialize PreprocessedShare: {}", e);
            return Err(io::Error::new(io::ErrorKind::InvalidData, e));
        }
    };

    debug!("Deserializing individual fields (a, b, alpha, mac_alpha, mac_r, mac_chi_values)...");
    let a: DVector<BigInt> = deserialize(&input_data.a.unwrap()).unwrap();
    let b: BigInt = deserialize(&input_data.b.unwrap()).unwrap();
    // let alpha: BigInt = deserialize(&input_data.alpha.unwrap()).unwrap();
    let mac_alpha: BigInt = deserialize(&input_data.mac_alpha.unwrap()).unwrap();
    let mac_r: DVector<BigInt> = deserialize(&input_data.mac_r.unwrap()).unwrap();
    let mac_chi_values: DVector<BigInt> = deserialize(&input_data.mac_chi_vals.unwrap()).unwrap();

    let mut worker = Worker::new(my_id, public_parameters.clone(), ctxt_per_job);

    debug!("Setting up MPC decryption values...");
    let message_input_data: Vec<ProtocolTransferredData> = worker.mpc_decryptions.iter_mut()
        .map(|mpc_party| {
            mpc_party.set_r(preprocessed.r.clone());
            mpc_party.set_s(preprocessed.s.clone());
            mpc_party.set_sk(preprocessed.sk.clone());
            mpc_party.set_ltz(preprocessed.ltz.clone());
            mpc_party.set_signs(preprocessed.signs.clone());

            mpc_party.set_a(a.clone());
            mpc_party.set_b(b.clone());
            // mpc_party.set_alpha(alpha.clone());
            mpc_party.set_mac_alpha(mac_alpha.clone());
            mpc_party.set_mac_r(mac_r.clone());
            mpc_party.set_mac_chi_values(mac_chi_values.clone());

            ProtocolTransferredData::empty()
        })
        .collect();

    debug!("MPC decryption setup complete. Setting start_time...");
    worker.start_time = Some(Instant::now());

    debug!("Returning initial ProtocolTransferredBulkData...");
    Ok((worker, message_input_data))
}

pub fn handle_protocol_execute_step(
    worker_data: &mut Worker,
    job_id: u64,
    my_participant_id: usize,
    received_from_participant: usize,
    step_num: usize,
    input_data: Vec<ProtocolTransferredData>,
) -> ExecutionResult<(usize, Vec<ProtocolTransferredData>)> {

    debug!(
            "Executing step: received_from_participant={}, step_num={}, job_id={}",
            received_from_participant,  step_num, job_id
        );

    // Insert new data
    worker_data.steps_bulk_data.insert((step_num, received_from_participant), input_data);
    debug!(
            "JOB {}: Inserted data for key ({}, {}). Current size of steps_bulk_data: {}",
        job_id,
        step_num,
            received_from_participant,
            worker_data.steps_bulk_data.len()
        );

    // Collect current step bulk data
    let step_bulk_data: Vec<_> = worker_data.steps_bulk_data
        .iter()
        .filter(|((step, _), _)| step == &step_num)
        .map(|(_, value)| value.clone())
        .collect();

    // Wait for enough data to proceed
    if step_bulk_data.len() < worker_data.params.n - 1 {
        debug!(
                "JOB {}: Waiting for more data: collected {} of required {}",
                job_id,
                step_bulk_data.len(),
                worker_data.params.n - 1
            );
        return NoReady;
    }

    // Check if `self.ctxt_per_job` and `step_bulk_data` contain data to prevent out-of-bounds
    let mut output_data = Vec::new();
    for ctxt_index in 0..worker_data.ctxt_per_job {
        let step_input: Vec<_> = step_bulk_data
            .iter()
            .filter_map(|data| data.get(ctxt_index).cloned()) // Safely get data at `ctxt_index`
            .collect();


        // Ensure `step_input` has the expected number of items
        if step_input.len() < worker_data.params.n - 1 {
            debug!("Insufficient step input data at ctxt_index {}: expected {}, found {}", ctxt_index, worker_data.params.n - 1, step_input.len());
            continue;
        }

        // Execute step if `worker_data.mpc_decryptions` has enough data and can be accessed mutably
        if let Some(mpc_decryption) = worker_data.mpc_decryptions.get_mut(ctxt_index) {
            output_data.push(mpc_decryption.execute_step(step_num, step_input));
        } else {
            debug!("Warning: No MPC decryption data available for ctxt_index {}", ctxt_index);
        }
    }

    let next_step_num = if step_num == STEP_COUNT { 0 } else { step_num + 1 };

    if next_step_num == STEP_COUNT {
        if let Some(start) = worker_data.start_time {
            let elapsed = start.elapsed();
            println!(
                "Completed all iterations. job: {} ctxt_per_job: {}, n: {}, k: {}, m: {}, b: {}, mac_s: {}, lwe_a: {}, microseconds: {}",
                job_id,
                &worker_data.ctxt_per_job,
                &worker_data.params.n,
                &worker_data.params.k,
                &worker_data.params.m,
                &worker_data.params.b,
                &worker_data.params.mac_s,
                &worker_data.params.lwe_dimension,
                elapsed.as_micros()
            );
        }
        Finished
    } else {
        debug!(
                "JOB {}: Proceeding to next next_step_num={}",
            job_id,
            next_step_num
            );
        // NextStep((next_step_num, output_data))
        send_result_to_everyone(&output_data, next_step_num, job_id as usize, my_participant_id);
        NextStep((next_step_num, output_data))
    }
}