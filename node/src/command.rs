use crate::{
	chain_spec,
	cli::{Cli, Subcommand},
	service,
};
use creditcoin_node_runtime::Block;
use futures::{future, future::FutureExt, pin_mut, select, Future};
use sc_cli::{build_runtime, ChainSpec, Role, RuntimeVersion, SubstrateCli};
use sc_service::{Configuration, Error as ServiceError, PartialComponents};

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
			let runner = cli.create_runner(&cli.run)?;
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

#[allow(dead_code)]
pub fn build_tokio_runtime() -> std::result::Result<tokio::runtime::Runtime, std::io::Error> {
	build_runtime()
}

/// The recommended open file descriptor limit to be configured for the process.
const RECOMMENDED_OPEN_FILE_DESCRIPTOR_LIMIT: u64 = 10_000;

#[allow(dead_code)]
pub fn run_internal(
	args: Vec<std::ffi::OsString>,
	tokio_runtime: &tokio::runtime::Runtime,
) -> sc_cli::Result<()> {
	let cli = Cli::from_iter(args.into_iter());
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

	run_node_until_exit(
		|config| async move {
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
		},
		config,
		&tokio_runtime,
	)
}

fn set_sp_panic_handler<C: SubstrateCli>() {
	sp_panic_handler::set(&C::support_url(), &C::impl_version());
}

fn run_node_until_exit<F, E>(
	initialize: impl FnOnce(Configuration) -> F,
	config: Configuration,
	tokio_runtime: &tokio::runtime::Runtime,
) -> std::result::Result<(), E>
where
	F: Future<Output = std::result::Result<sc_service::TaskManager, E>>,
	E: std::error::Error + Send + Sync + 'static + From<ServiceError>,
{
	sc_cli::print_node_infos::<Cli>(&config);
	let mut task_manager = tokio_runtime.block_on(initialize(config))?;
	let res = tokio_runtime.block_on(main(task_manager.future().fuse()));
	Ok(res?)
}

#[cfg(target_family = "unix")]
async fn main<F, E>(func: F) -> std::result::Result<(), E>
where
	F: Future<Output = std::result::Result<(), E>> + future::FusedFuture,
	E: std::error::Error + Send + Sync + 'static + From<ServiceError>,
{
	// use tokio::signal::unix::{signal, SignalKind};

	let mut stream_int = signal(SignalKind::interrupt()).map_err(ServiceError::Io)?;
	let mut stream_term = signal(SignalKind::terminate()).map_err(ServiceError::Io)?;

	let t1 = stream_int.recv().fuse();
	let t2 = stream_term.recv().fuse();
	let t3 = func;

	pin_mut!(t1, t2, t3);

	select! {
		_ = t1 => {},
		_ = t2 => {},
		res = t3 => res?,
	}

	Ok(())
}

#[cfg(not(unix))]
async fn main<F, E>(func: F) -> std::result::Result<(), E>
where
	F: Future<Output = std::result::Result<(), E>> + future::FusedFuture,
	E: std::error::Error + Send + Sync + 'static + From<ServiceError>,
{
	use tokio::signal::ctrl_c;

	let t1 = ctrl_c().fuse();
	let t2 = func;

	pin_mut!(t1, t2);

	select! {
		_ = t1 => {},
		res = t2 => res?,
	}

	Ok(())
}
