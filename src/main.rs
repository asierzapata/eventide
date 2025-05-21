use clap::Parser;
use clap::Subcommand;

#[derive(Parser)]
#[command(version, about, long_about = None)] // Reads it from `Cargo.toml`
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Stack {
        /// The input files to stack
        #[arg(short, long, value_name = "LIGHTS_FOLDER")]
        lights_folder: String,

        /// The input darks to calibrate with (Optional)
        #[arg(short, long, value_name = "DARKS_FOLDER")]
        darks_folder: Option<String>,

        /// The input flats to calibrate with (Optional)
        #[arg(short, long, value_name = "FLATS_FOLDER")]
        flats_folder: Option<String>,

        /// The input bias to calibrate with (Optional)
        #[arg(short, long, value_name = "BIAS_FOLDER")]
        bias_folder: Option<String>,

        /// The output file name
        #[arg(short, long, value_name = "OUTPUT")]
        output: String,

        /// The number of threads to use
        #[arg(short, long, value_name = "THREADS")]
        threads: Option<usize>,
    },
}

fn main() {
    let args = Cli::parse();

    match args.command {
        Commands::Stack {
            lights_folder,
            darks_folder,
            flats_folder,
            bias_folder,
            output,
            threads,
        } => {
            println!("Lights folder: {}", lights_folder);
            println!("Darks folder: {:?}", darks_folder);
            println!("Flats folder: {:?}", flats_folder);
            println!("Bias folder: {:?}", bias_folder);
            println!("Output file: {}", output);
            println!("Threads: {:?}", threads);
        }
    }
}
