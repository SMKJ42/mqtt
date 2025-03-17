use std::path::PathBuf;

use clap::{Parser, Subcommand};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use sheesh::harness::stateless::init_tables_stateless_sqlite_config;

#[derive(Debug, Parser)]
#[command(name = "rdme")]
#[command(about = "CLI tool for the RDME broker ecosystem", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    // #[command(arg_required_else_help = true)]
    // Login {
    //     #[arg(value_name = "USERNAME")]
    //     username: String,
    //     #[arg(value_name = "PASSWORD")]
    //     password: String,
    // },
    #[command(arg_required_else_help = true)]
    AddUser {
        #[arg(value_name = "USERNAME")]
        username: String,
        #[arg(value_name = "PASSWORD")]
        password: String,
        #[arg(value_name = "ROLE")]
        role: String,
    },

    #[command(arg_required_else_help = true)]
    Init {
        path: PathBuf,
    },

    #[command(arg_required_else_help = true)]
    ModUser {
        #[arg(
            value_name = "USERNAME",
            help = "The user's current username before the command is executed."
        )]
        username: String,

        #[arg(
            value_name = "NEW_USERNAME",
            short = 'u',
            long = "username",
            help = "The user's new username after the command is executed"
        )]
        new_username: Option<String>,

        #[arg(value_name = "PASSWORD", short = 'p', long = "password")]
        password: Option<String>,

        #[arg(value_name = "ROLE", short = 'r', long = "role")]
        role: Option<String>,

        #[arg(value_name = "BAN_USER", short = 'b', long = "ban")]
        ban: Option<bool>,
    },

    DelUser {
        username: String,
    },

    FindUser {
        username: String,
    },
}

fn main() {
    let args = Cli::parse();

    let pool = Pool::new(SqliteConnectionManager::file(""))
        .expect("Could not connect to sqlite database for users.");

    match args.command {
        // Commands::Login { username, password } => {}
        Commands::AddUser {
            username,
            password,
            role,
        } => {
            println!("username {username}, password: {password}, role: {role}");
        }
        Commands::ModUser {
            username,
            new_username,
            password,
            role,
            ban,
        } => {}
        Commands::FindUser { username } => {}
        Commands::DelUser { username } => {}
        Commands::Init { .. } => {
            let (user_manager, session_manager) =
                init_tables_stateless_sqlite_config(pool).expect("Error establishing ");
        }
    }
}

fn create_user() {}

fn update_user() {}

fn delete_user() {}
