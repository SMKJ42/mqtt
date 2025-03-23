mod config;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use sheesh::harness::stateless::{
    init_stateless_sqlite_config, init_tables_stateless_sqlite_config,
};

const SECRET_WARN: &'static str = "just to warn you, your secrets aren't safe in the CLI yet...";

#[derive(Debug, Parser)]
#[command(name = "rdme")]
#[command(about = "CLI tool for the RDME broker ecosystem", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
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

    UserInfo {
        username: String,
    },

    // #[command(arg_required_else_help = true)]
    // Login {
    //     #[arg(value_name = "USERNAME")]
    //     username: String,
    //     #[arg(value_name = "PASSWORD")]
    //     password: String,
    // },

    // Logout,
    #[command(arg_required_else_help = true)]
    Init {
        path: PathBuf,
    },
}

fn main() {
    let args = Cli::parse();

    // TODO: need to setup the config file to hold variable for db_path
    let pool = Pool::new(SqliteConnectionManager::file(""))
        .expect("Could not connect to sqlite database for users.");

    match args.command {
        Commands::AddUser {
            username,
            password,
            role,
        } => {
            let (user_manager, _) = init_stateless_sqlite_config(pool);
            user_manager
                // TODO: Sheesh-auth needs an update to not have the Role {name: } style instatiation... oops...
                .create_user(username, &password, sheesh::user::Role { name: role })
                .expect("Error creating user");
        }
        Commands::ModUser {
            username,
            new_username,
            password,
            role,
            ban,
        } => {
            let (user_manager, _) = init_stateless_sqlite_config(pool);
            let user = user_manager
                .get_user_by_username(&username)
                .expect("Error Querying user");
            if let Some(mut user) = user {
                if let Some(new_username) = new_username {
                    user_manager
                        .update_username(user.id(), &new_username)
                        .expect("Could not update username.");
                }

                if let Some(password) = password {
                    user_manager
                        // TODO: sheesh needs an update here...
                        .update_password(user.clone(), password)
                        .expect("Could not update password.");
                }

                if let Some(role) = role {
                    // TODO: need to have preconfig'd roles setup in .env file.
                    user.set_role(sheesh::user::Role { name: role });
                    user_manager
                        .update_user(&user)
                        .expect("Could not update role.")
                }

                if let Some(ban) = ban {
                    if ban == true {
                        user_manager
                            .set_user_ban(user.id(), ban)
                            .expect("Could not update user's ban.")
                    }
                }
            } else {
                eprintln!("Could not find user.")
            }
        }
        Commands::UserInfo { username } => {
            let (user_manager, _) = init_stateless_sqlite_config(pool);
            let user = user_manager
                .get_user_by_username(&username)
                .expect("Error Querying user");
            if let Some(user) = user {
                println!("{:?}", user);
            } else {
                eprintln!("Could not find user.")
            }
        }
        Commands::DelUser { username } => {
            let (user_manager, _) = init_stateless_sqlite_config(pool);
            let user = user_manager
                .get_user_by_username(&username)
                .expect("Error Querying user");
            if let Some(user) = user {
                user_manager
                    .delete_user(user.id())
                    .expect("Could not Delete user");
            } else {
                eprintln!("Could not find user.")
            }
        }
        Commands::Init { path } => {
            init_tables_stateless_sqlite_config(pool).expect("Error establishing ");
        } // Commands::Login { username, password } => {
          //     println!("Logging in. {}", SECRET_WARN);
          //     todo!("Need authenticate against the harness, then store the credentials safely");
          // }

          // Commands::Logout {} => {
          //     todo!();
          // }
    }
}
