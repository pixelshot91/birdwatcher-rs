use clap::{CommandFactory, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask")]
#[command(about = "A custom build command for the project", long_about = None)]
struct XTask {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate shell completions
    Completions {
        /// The shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    ManPage,
}

fn main() {
    let xtask = XTask::parse();

    match xtask.command {
        Commands::Completions { shell } => {
            generate_completion(
                birdwatcher_rs::daemon_clap_command::Cli::command(),
                "birdwatcher-daemon",
                shell,
            );
        }
        Commands::ManPage => {
            generate_man_page(
                birdwatcher_rs::daemon_clap_command::Cli::command(),
                "birdwatcher-daemon",
            );
        }
    }
}

fn generate_man_page(clap_command: clap::Command, name: &str) {
    let man_page = clap_mangen::Man::new(clap_command);
    let out_dir: String = format!("share/{name}/man");
    std::fs::create_dir_all(&out_dir).expect("Failed to create directories for man-pages");
    let generated_man_path = man_page
        .generate_to(out_dir)
        .expect("Failed to generate man page");
    println!("Man page generated: {}", generated_man_path.display());
}

fn generate_completion(mut clap_command: clap::Command, name: &str, shell: clap_complete::Shell) {
    let out_dir = format!("share/{name}/completions");
    std::fs::create_dir_all(&out_dir).expect("Failed to create directories for completions");
    let completion_path = clap_complete::generate_to(shell, &mut clap_command, name, out_dir)
        .expect("Failed to generate completions");
    println!(
        "Completions generated for {:?} at: {}",
        shell,
        completion_path.display()
    );
}
