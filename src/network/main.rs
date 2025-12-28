use clap::{Parser, Subcommand,};

use threshold_decryption::mpc::preprocessing::Preprocessing;
use threshold_decryption::mpc::public_params::PublicParameters;

use threshold_decryption::network::discovery_server::DiscoveryServer;
use threshold_decryption::network::participant::Participant;


#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short = 'n', long = "parties")]
    n: usize,

    #[arg(short = 'k', long = "ptxt-bits")]
    k: usize,

    #[arg(short = 'm', long = "ctxt-bits")]
    m: usize,

    #[arg(short = 'b', long = "digit-bits")]
    b: usize,

    #[arg(long = "lwe-bits")]
    lwe_dimension: usize,

    #[arg(long = "mac-s")]
    mac_s: usize,

}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    DiscoveryServer,

    Participant{
        id: usize,
    }
}





pub  fn main() {
    env_logger::builder().filter_level(log::LevelFilter::Info).init();
    let cli = Cli::parse();

    let public_parameters =
        PublicParameters::init(cli.n, cli.k ,cli.m, cli.b, cli.lwe_dimension, cli.mac_s);

    match &cli.command {
        Commands::DiscoveryServer => {
            let preprocessing = Preprocessing::new(&public_parameters);
            match DiscoveryServer::new(&public_parameters, &preprocessing) {
                Ok(discovery_server) => discovery_server.run(),
                Err(_err) => { //debug!("Can not run the discovery server: {}", _err)
                },
            }
        }
        Commands::Participant{id} => {
            // let party = Party::new(id.clone(), &public_parameters);

            match Participant::new(id.clone(), &public_parameters) {
                Ok(participant) => {
                    participant.run()
                },
                Err(_err) => {
                    //debug!("Can not run the participant: {}", _err);
                }
            }
        }
    }
}