use clap::{Args, Parser, Subcommand};
use serde::Serialize;
use std::process::ExitCode;

use checklister::{connect, Check, Checklist, ChecklistDetails, Store};

#[derive(Parser)]
#[command(name = "checklister", about = "Manage checklists and checks")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create, list, inspect, or delete a checklist.
    Checklist {
        #[command(subcommand)]
        command: ChecklistCommand,
    },
    /// Add, list, complete, or undo checks.
    Check {
        #[command(subcommand)]
        command: CheckCommand,
    },
}

#[derive(Subcommand)]
enum ChecklistCommand {
    Create { name: String },
    List,
    Show { checklist_id: i64 },
    Delete { checklist_id: i64 },
}

#[derive(Subcommand)]
enum CheckCommand {
    Add(AddCheckArgs),
    List { checklist_id: i64 },
    Undone { checklist_id: i64 },
    Complete {
        checklist_id: i64,
        check_id: i64,
    },
    Undo {
        checklist_id: i64,
        check_id: i64,
    },
}

#[derive(Args)]
struct AddCheckArgs {
    checklist_id: i64,
    #[arg(long)]
    title: String,
    #[arg(long)]
    description: String,
}

#[derive(Serialize)]
struct Success<T: Serialize> {
    success: bool,
    message: &'static str,
    result: T,
}

fn print_json<T: Serialize>(value: &T) -> Result<(), serde_json::Error> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let store = Store::new(connect().await?);

    match cli.command {
        Command::Checklist { command } => match command {
            ChecklistCommand::Create { name } => {
                let checklist = store.create_checklist(&name).await?;
                print_json(&Success {
                    success: true,
                    message: "Checklist created",
                    result: checklist,
                })?;
            }
            ChecklistCommand::List => print_json(&store.list_checklists().await?)?,
            ChecklistCommand::Show { checklist_id } => {
                let details: ChecklistDetails = store.show_checklist(checklist_id).await?;
                print_json(&details)?;
            }
            ChecklistCommand::Delete { checklist_id } => {
                let checklist: Checklist = store.delete_checklist(checklist_id).await?;
                print_json(&Success {
                    success: true,
                    message: "Checklist deleted",
                    result: checklist,
                })?;
            }
        },
        Command::Check { command } => match command {
            CheckCommand::Add(args) => {
                let check = store
                    .add_check(args.checklist_id, &args.title, &args.description)
                    .await?;
                print_json(&Success {
                    success: true,
                    message: "Check added",
                    result: check,
                })?;
            }
            CheckCommand::List { checklist_id } => {
                let checks: Vec<Check> = store.list_checks(checklist_id, false).await?;
                print_json(&checks)?;
            }
            CheckCommand::Undone { checklist_id } => {
                let checks: Vec<Check> = store.list_checks(checklist_id, true).await?;
                print_json(&checks)?;
            }
            CheckCommand::Complete {
                checklist_id,
                check_id,
            } => {
                let check = store
                    .set_check_status(checklist_id, check_id, true)
                    .await?;
                print_json(&Success {
                    success: true,
                    message: "Check completed",
                    result: check,
                })?;
            }
            CheckCommand::Undo {
                checklist_id,
                check_id,
            } => {
                let check = store
                    .set_check_status(checklist_id, check_id, false)
                    .await?;
                print_json(&Success {
                    success: true,
                    message: "Check marked undone",
                    result: check,
                })?;
            }
        },
    }
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!(
                "{}",
                serde_json::json!({ "success": false, "error": error.to_string() })
            );
            ExitCode::FAILURE
        }
    }
}
