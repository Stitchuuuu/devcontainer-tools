use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "notif", version, about = "Cross-platform notification CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Send {
        #[arg(long)]
        title: String,
        #[arg(long)]
        body: String,
    },
}

const HOST: &str = if cfg!(target_os = "macos") {
    "macos"
} else if cfg!(target_os = "windows") {
    "windows"
} else if cfg!(target_os = "linux") {
    "linux"
} else {
    "unknown"
};

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Send { title, body } => {
            println!("[stub] would dispatch: title={title}, body={body}, host={HOST}");
        }
    }
}
