use crossbeam_channel::{unbounded, Receiver, Sender};
use tokio::runtime::Handle;
use tracing::info;

use rib_db::Pool as DbPool;

use crate::job::{ParseJob, SolveJob, WarmJob};
use crate::processors::{process_parse, process_solve, process_warm};

/// A fixed pool of dedicated OS threads, each of which loops pulling
/// whichever of the three job queues currently has work via
/// `crossbeam_channel::select!`. This is the "dynamic worker loads that can
/// shift between parsing and computing" piece: because every worker thread
/// can serve *any* queue, capacity isn't statically partitioned into "N
/// parser threads + M solver threads" -- if hand-history uploads spike,
/// every idle worker picks up parse jobs; if a burst of live solves comes
/// in, every idle worker picks up solve jobs instead. `crossbeam_channel`'s
/// `select!` picks pseudo-randomly among whichever channels are ready,
/// which is exactly the fairness property that makes this work without any
/// explicit load-balancing logic.
///
/// Library-warm jobs (pre-solving the curated preflop seed list) are kept
/// in their own lowest-priority queue: `select!` only considers `warm_rx`
/// when polled, and because it's listed last among equally-ready arms it
/// gets a fair, but not preferential, share -- in practice that means warm
/// jobs fill in the gaps between user-facing parse/solve traffic rather
/// than ever blocking it, since there's always far more warm-queue backlog
/// than urgency behind it.
#[derive(Clone)]
pub struct WorkerPool {
    parse_tx: Sender<ParseJob>,
    solve_tx: Sender<SolveJob>,
    warm_tx: Sender<WarmJob>,
    n_workers: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PoolStats {
    pub parse_queue_len: usize,
    pub solve_queue_len: usize,
    pub warm_queue_len: usize,
    pub n_workers: usize,
}

impl WorkerPool {
    /// Spawns `n_workers` dedicated threads (pass `0` to default to the
    /// number of logical CPUs) sharing a handle into the caller's tokio
    /// runtime for the (comparatively cheap, I/O-bound) database calls each
    /// job needs to make around its CPU-bound core work.
    pub fn new(n_workers: usize, db: DbPool) -> Self {
        let n_workers = if n_workers == 0 { num_cpus::get() } else { n_workers };
        let (parse_tx, parse_rx) = unbounded::<ParseJob>();
        let (solve_tx, solve_rx) = unbounded::<SolveJob>();
        let (warm_tx, warm_rx) = unbounded::<WarmJob>();
        let rt = Handle::current();

        for id in 0..n_workers {
            let parse_rx = parse_rx.clone();
            let solve_rx = solve_rx.clone();
            let warm_rx = warm_rx.clone();
            let db = db.clone();
            let rt = rt.clone();
            std::thread::Builder::new()
                .name(format!("rib-worker-{id}"))
                .spawn(move || worker_loop(parse_rx, solve_rx, warm_rx, db, rt))
                .expect("failed to spawn worker thread");
        }

        info!(n_workers, "worker pool started");
        Self { parse_tx, solve_tx, warm_tx, n_workers }
    }

    pub fn submit_parse(&self, job: ParseJob) {
        let _ = self.parse_tx.send(job);
    }

    pub fn submit_solve(&self, job: SolveJob) {
        let _ = self.solve_tx.send(job);
    }

    pub fn submit_warm(&self, job: WarmJob) {
        let _ = self.warm_tx.send(job);
    }

    pub fn stats(&self) -> PoolStats {
        PoolStats {
            parse_queue_len: self.parse_tx.len(),
            solve_queue_len: self.solve_tx.len(),
            warm_queue_len: self.warm_tx.len(),
            n_workers: self.n_workers,
        }
    }
}

fn worker_loop(
    parse_rx: Receiver<ParseJob>,
    solve_rx: Receiver<SolveJob>,
    warm_rx: Receiver<WarmJob>,
    db: DbPool,
    rt: Handle,
) {
    loop {
        crossbeam_channel::select! {
            recv(solve_rx) -> job => match job {
                Ok(j) => rt.block_on(process_solve(&db, j)),
                Err(_) => return,
            },
            recv(parse_rx) -> job => match job {
                Ok(j) => rt.block_on(process_parse(&db, j)),
                Err(_) => return,
            },
            recv(warm_rx) -> job => match job {
                Ok(j) => rt.block_on(process_warm(&db, j)),
                Err(_) => return,
            },
        }
    }
}
