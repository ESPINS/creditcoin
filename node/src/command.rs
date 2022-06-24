use crate::{
	chain_spec,
	cli::{Cli, Subcommand},
	service,
};
use creditcoin_node_runtime::Block;
use sc_cli::{build_runtime, ChainSpec, Role, Runner, RuntimeVersion, SubstrateCli};
use sc_service::PartialComponents;

impl SubstrateCli for Cli {
	fn impl_name() -> String {
		"Creditcoin Node".into()
	}

	fn impl_version() -> String {
		env!("SUBSTRATE_CLI_IMPL_VERSION").into()
	}

	fn description() -> String {
		env!("CARGO_PKG_DESCRIPTION").into()
	}

	fn author() -> String {
		env!("CARGO_PKG_AUTHORS").into()
	}

	fn support_url() -> String {
		"support.anonymous.an".into()
	}

	fn copyright_start_year() -> i32 {
		2017
	}

	fn load_spec(&self, id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
		Ok(match id {
			"" => {
				let msg =
					"Please specify the chain with '--chain main' or '--chain test'".to_owned();
				log::error!("{}", msg);
				return Err(msg);
			},
			"dev" => Box::new(chain_spec::development_config()?),
			"local" => Box::new(chain_spec::local_testnet_config()?),
			"test" | "testnet" => Box::new(chain_spec::testnet_config()?),
			"main" | "mainnet" => Box::new(chain_spec::mainnet_config()?),
			path => {
				Box::new(chain_spec::ChainSpec::from_json_file(std::path::PathBuf::from(path))?)
			},
		})
	}

	fn native_runtime_version(_: &Box<dyn ChainSpec>) -> &'static RuntimeVersion {
		&creditcoin_node_runtime::VERSION
	}
}

#[allow(dead_code)]
pub fn from_args() -> Cli {
	Cli::from_args()
}

/// Parse and run command line arguments
pub fn run() -> sc_cli::Result<()> {
	let cli = Cli::from_args();

	match &cli.subcommand {
		Some(Subcommand::Key(cmd)) => cmd.run(&cli),
		Some(Subcommand::BuildSpec(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
		},
		Some(Subcommand::CheckBlock(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, import_queue, .. } =
					service::new_partial(&config)?;
				Ok((cmd.run(client, import_queue), task_manager))
			})
		},
		Some(Subcommand::ExportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, .. } = service::new_partial(&config)?;
				Ok((cmd.run(client, config.database), task_manager))
			})
		},
		Some(Subcommand::ExportState(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, .. } = service::new_partial(&config)?;
				Ok((cmd.run(client, config.chain_spec), task_manager))
			})
		},
		Some(Subcommand::ImportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, import_queue, .. } =
					service::new_partial(&config)?;
				Ok((cmd.run(client, import_queue), task_manager))
			})
		},
		Some(Subcommand::PurgeChain(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.database))
		},
		Some(Subcommand::Revert(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, backend, .. } =
					service::new_partial(&config)?;
				Ok((cmd.run(client, backend), task_manager))
			})
		},
		Some(Subcommand::Benchmark(cmd)) => {
			if cfg!(feature = "runtime-benchmarks") {
				let runner = cli.create_runner(cmd)?;

				runner.sync_run(|config| cmd.run::<Block, service::ExecutorDispatch>(config))
			} else {
				Err("Benchmarking wasn't enabled when building the node. You can enable it with \
					`--features runtime-benchmarks`."
					.into())
			}
		},
		None => {
			let runner = if cli.logger_mode == "WithoutLoggerInit" {
				create_runner_without_logger_init(&cli)?
			} else {
				cli.create_runner(&cli.run)?
			};
			runner.run_node_until_exit(|config| async move {
				match config.role {
					Role::Light => Err("Light clients are not supported at this time".into()),
					_ => service::new_full(
						config,
						cli.mining_key.as_deref(),
						cli.mining_threads,
						cli.rpc_mapping,
					),
				}
				.map_err(sc_cli::Error::Service)
			})
		},
	}
}

/// The recommended open file descriptor limit to be configured for the process.
const RECOMMENDED_OPEN_FILE_DESCRIPTOR_LIMIT: u64 = 10_000;

fn create_runner_without_logger_init(cli: &Cli) -> sc_cli::Result<Runner<Cli>> {
	let tokio_runtime = build_runtime()?;
	let config = cli.create_configuration(&cli.run, tokio_runtime.handle().clone())?;

	set_sp_panic_handler::<Cli>();

	if let Some(new_limit) = fdlimit::raise_fd_limit() {
		if new_limit < RECOMMENDED_OPEN_FILE_DESCRIPTOR_LIMIT {
			log::warn!(
				"Low open file descriptor limit configured for the process. \
				Current value: {:?}, recommended value: {:?}.",
				new_limit,
				RECOMMENDED_OPEN_FILE_DESCRIPTOR_LIMIT,
			);
		}
	}

	Runner::new(config, tokio_runtime)
}

fn set_sp_panic_handler<C: SubstrateCli>() {
	sp_panic_handler::set(&C::support_url(), &C::impl_version());
}
