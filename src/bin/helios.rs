//! Thin headless CLI for Helios Chronicle Phase A.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use helios_chronicle::{
    load_world, save_world, verify_determinism, LodMode, Operator, World,
};

#[derive(Parser, Debug)]
#[command(name = "helios", about = "Helios Chronicle — headless Phase A kernel")]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run N ticks from a seed (optionally save at the end).
    Run {
        /// Number of LOD steps to advance.
        #[arg(short = 'n', long, default_value_t = 10)]
        ticks: u64,
        /// World seed (same seed ⇒ same outcomes).
        #[arg(short, long, default_value_t = 42)]
        seed: u64,
        /// LOD mode for the run.
        #[arg(long, value_enum, default_value_t = LodCli::Fine)]
        lod: LodCli,
        /// Optional JSON save path after the run.
        #[arg(long)]
        save: Option<PathBuf>,
        /// Optional JSON load path (ignores --seed if set; then ticks further).
        #[arg(long)]
        load: Option<PathBuf>,
        /// Demo operator mutation on first system (binding_remainder).
        #[arg(long, default_value_t = false)]
        demo_operator: bool,
    },
    /// Verify same seed ⇒ same outcomes (exit 0 on match).
    Verify {
        #[arg(short, long, default_value_t = 42)]
        seed: u64,
        #[arg(short = 'n', long, default_value_t = 50)]
        ticks: u64,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum LodCli {
    Fine,
    Coarse,
}

impl From<LodCli> for LodMode {
    fn from(v: LodCli) -> Self {
        match v {
            LodCli::Fine => LodMode::Fine,
            LodCli::Coarse => LodMode::Coarse,
        }
    }
}

fn main() {
    let cli = Cli::parse();
    match cli.cmd {
        Commands::Run {
            ticks,
            seed,
            lod,
            save,
            load,
            demo_operator,
        } => {
            let mut world = if let Some(path) = load {
                println!("loading {}", path.display());
                load_world(&path).expect("load failed")
            } else {
                println!("seed={seed}");
                World::new(seed)
            };
            world.set_lod(lod.into());
            println!(
                "systems={} master_tick={} lod={:?} dt={}",
                world.ledger().len(),
                world.master_tick(),
                world.lod(),
                world.current_dt()
            );

            if demo_operator {
                let first_id = world.ledger().systems().next().map(|(i, _)| *i);
                if let Some(id) = first_id {
                    let mut op = Operator::new(&mut world);
                    op.set_field(id, "binding_remainder", "777")
                        .expect("operator set");
                    println!("operator: set system {id} binding_remainder=777");
                }
            }

            world.tick(ticks);
            println!(
                "after {ticks} steps: master_tick={} outcome_hash={:#x} events={}",
                world.master_tick(),
                world.outcome_hash(),
                world.log().len()
            );

            if let Some(path) = save {
                save_world(&mut world, &path).expect("save failed");
                println!("saved {}", path.display());
            }
        }
        Commands::Verify { seed, ticks } => {
            let (ok, ha, hb) = verify_determinism(seed, ticks);
            println!(
                "verify seed={seed} ticks={ticks}: match={ok} hash_a={ha:#x} hash_b={hb:#x}"
            );
            if !ok {
                std::process::exit(1);
            }
        }
    }
}
