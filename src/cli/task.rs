use crate::task::{Alias, CmdArgs, Execute, Task};
use crate::Project;
use clap::Parser;
use itertools::Itertools;
use rattler_conda_types::Platform;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub enum Operation {
    /// Add a command to the project
    #[clap(alias = "a")]
    Add(AddArgs),

    /// Remove a command from the project
    #[clap(alias = "r")]
    Remove(RemoveArgs),

    /// Alias another specific command
    #[clap(alias = "@")]
    Alias(AliasArgs),

    /// List all tasks
    #[clap(alias = "l")]
    List,
}

#[derive(Parser, Debug)]
#[clap(arg_required_else_help = true)]
pub struct RemoveArgs {
    /// Task names to remove
    pub names: Vec<String>,

    /// The platform for which the task should be removed
    #[arg(long, short)]
    pub platform: Option<Platform>,
}

#[derive(Parser, Debug, Clone)]
#[clap(arg_required_else_help = true)]
pub struct AddArgs {
    /// Task name
    pub name: String,

    /// One or more commands to actually execute
    #[clap(required = true, num_args = 1..)]
    pub commands: Vec<String>,

    /// Depends on these other commands
    #[clap(long)]
    #[clap(num_args = 1..)]
    pub depends_on: Option<Vec<String>>,

    /// The platform for which the task should be added
    #[arg(long, short)]
    pub platform: Option<Platform>,
}

#[derive(Parser, Debug, Clone)]
#[clap(arg_required_else_help = true)]
pub struct AliasArgs {
    /// Alias name
    pub alias: String,

    /// Depends on these tasks to execute
    #[clap(required = true, num_args = 1..)]
    pub depends_on: Vec<String>,

    /// The platform for which the alias should be added
    #[arg(long, short)]
    pub platform: Option<Platform>,
}

impl From<AddArgs> for Task {
    fn from(value: AddArgs) -> Self {
        let depends_on = value.depends_on.unwrap_or_default();

        if depends_on.is_empty() {
            Self::Plain(if value.commands.len() == 1 {
                value.commands[0].clone()
            } else {
                shlex::join(value.commands.iter().map(AsRef::as_ref))
            })
        } else {
            Self::Execute(Execute {
                cmd: CmdArgs::Single(if value.commands.len() == 1 {
                    value.commands[0].clone()
                } else {
                    shlex::join(value.commands.iter().map(AsRef::as_ref))
                }),
                depends_on,
            })
        }
    }
}

impl From<AliasArgs> for Task {
    fn from(value: AliasArgs) -> Self {
        Self::Alias(Alias {
            depends_on: value.depends_on,
        })
    }
}

/// Command management in project
#[derive(Parser, Debug)]
#[clap(trailing_var_arg = true, arg_required_else_help = true)]
pub struct Args {
    /// Add, remove, or update a task
    #[clap(subcommand)]
    pub operation: Operation,

    /// The path to 'pixi.toml'
    #[arg(long)]
    pub manifest_path: Option<PathBuf>,
}

pub fn execute(args: Args) -> miette::Result<()> {
    let mut project = Project::load_or_else_discover(args.manifest_path.as_deref())?;
    match args.operation {
        Operation::Add(args) => {
            let name = &args.name;
            let task: Task = args.clone().into();
            project.add_task(name, task.clone(), args.platform)?;
            eprintln!(
                "{}Added task {}: {}",
                console::style(console::Emoji("✔ ", "+")).green(),
                console::style(&name).bold(),
                task,
            );
        }
        Operation::Remove(args) => {
            let mut to_remove = Vec::new();
            for name in args.names.iter() {
                if let Some(platform) = args.platform {
                    if !project
                        .target_specific_tasks(platform)
                        .contains_key(name.as_str())
                    {
                        eprintln!(
                            "{}Task '{}' does not exist on {}",
                            console::style(console::Emoji("❌ ", "X")).red(),
                            console::style(&name).bold(),
                            console::style(platform.as_str()).bold(),
                        );
                        continue;
                    }
                } else if !project.tasks(None).contains_key(name.as_str()) {
                    eprintln!(
                        "{}Task {} does not exist",
                        console::style(console::Emoji("❌ ", "X")).red(),
                        console::style(&name).bold(),
                    );
                    continue;
                }

                // Check if task has dependencies
                let depends_on = project.task_names_depending_on(name);
                if !depends_on.is_empty() && !args.names.contains(name) {
                    eprintln!(
                        "{}: {}",
                        console::style("Warning, the following task/s depend on this task")
                            .yellow(),
                        console::style(depends_on.iter().to_owned().join(", ")).bold()
                    );
                    eprintln!(
                        "{}",
                        console::style("Be sure to modify these after the removal\n").yellow()
                    );
                }
                // Safe to remove
                to_remove.push((name, args.platform));
            }

            for (name, platform) in to_remove {
                project.remove_task(name, platform)?;
                eprintln!(
                    "{}Removed task {} ",
                    console::style(console::Emoji("❌ ", "X")).yellow(),
                    console::style(&name).bold(),
                );
            }
        }
        Operation::Alias(args) => {
            let name = &args.alias;
            let task: Task = args.clone().into();
            project.add_task(name, task.clone(), args.platform)?;
            eprintln!(
                "{} Added alias {}: {}",
                console::style("@").blue(),
                console::style(&name).bold(),
                task,
            );
        }
        Operation::List => {
            let tasks = project.task_names(Some(Platform::current()));
            if tasks.is_empty() {
                eprintln!("No tasks found",);
            } else {
                let mut formatted = String::new();
                for name in tasks {
                    formatted.push_str(&format!("* {}\n", console::style(name).bold(),));
                }
                eprintln!("{}", formatted);
            }
        }
    };

    Ok(())
}
