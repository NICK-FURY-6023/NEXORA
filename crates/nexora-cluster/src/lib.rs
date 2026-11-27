use std::collections::HashMap;
use std::fmt;
use std::sync::RwLock;

use nexora_common::ServiceName;
use nexora_error::{NexoraError, NexoraResult};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkerId(String);

impl WorkerId {
    pub fn new(value: impl Into<String>) -> NexoraResult<Self> {
        let value = value.into();
        if value.is_empty() || value.len() > 128 {
            return Err(NexoraError::invalid_input("invalid worker id"));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WorkerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Decode,
    Encode,
    Resample,
    Source,
    Delivery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerRegistration {
    pub id: WorkerId,
    pub cpu_capacity: u32,
    pub active_jobs: u32,
    pub capabilities: Vec<Capability>,
    pub healthy: bool,
}

impl WorkerRegistration {
    pub fn new(
        id: impl Into<String>,
        cpu_capacity: u32,
        active_jobs: u32,
        capabilities: Vec<Capability>,
    ) -> NexoraResult<Self> {
        if cpu_capacity == 0 {
            return Err(NexoraError::invalid_input(
                "worker cpu capacity must be positive",
            ));
        }
        Ok(Self {
            id: WorkerId::new(id)?,
            cpu_capacity,
            active_jobs,
            capabilities,
            healthy: true,
        })
    }

    pub fn headroom(&self) -> u32 {
        self.cpu_capacity
            .saturating_sub(self.active_jobs.saturating_mul(5))
    }
}

#[derive(Debug, Default)]
pub struct WorkerRegistry {
    workers: RwLock<HashMap<WorkerId, WorkerRegistration>>,
}

impl WorkerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, worker: WorkerRegistration) -> NexoraResult<()> {
        let mut workers = self
            .workers
            .write()
            .map_err(|_| NexoraError::internal("registry lock poisoned"))?;
        if workers.contains_key(&worker.id) {
            return Err(NexoraError::invalid_input("duplicate worker id"));
        }
        workers.insert(worker.id.clone(), worker);
        Ok(())
    }

    pub fn unregister(&self, id: &WorkerId) -> NexoraResult<()> {
        let mut workers = self
            .workers
            .write()
            .map_err(|_| NexoraError::internal("registry lock poisoned"))?;
        workers
            .remove(id)
            .ok_or_else(|| NexoraError::service("worker not found"))?;
        Ok(())
    }

    /// Records how many jobs a worker is running.
    pub fn update_load(&self, id: &WorkerId, active_jobs: u32) -> NexoraResult<()> {
        let mut workers = self
            .workers
            .write()
            .map_err(|_| NexoraError::internal("registry lock poisoned"))?;
        let worker = workers
            .get_mut(id)
            .ok_or_else(|| NexoraError::service("worker not found"))?;
        worker.active_jobs = active_jobs;
        Ok(())
    }

    /// Records whether a worker is taking work.
    pub fn set_healthy(&self, id: &WorkerId, healthy: bool) -> NexoraResult<()> {
        let mut workers = self
            .workers
            .write()
            .map_err(|_| NexoraError::internal("registry lock poisoned"))?;
        let worker = workers
            .get_mut(id)
            .ok_or_else(|| NexoraError::service("worker not found"))?;
        worker.healthy = healthy;
        Ok(())
    }

    /// The registration of one worker.
    pub fn worker(&self, id: &WorkerId) -> Option<WorkerRegistration> {
        self.workers
            .read()
            .ok()
            .and_then(|workers| workers.get(id).cloned())
    }

    pub fn workers(&self) -> Vec<WorkerRegistration> {
        self.workers
            .read()
            .map(|workers| workers.values().cloned().collect())
            .unwrap_or_default()
    }

    pub fn with_capability(
        &self,
        capability: Capability,
    ) -> impl Iterator<Item = WorkerRegistration> {
        self.workers()
            .into_iter()
            .filter(move |worker| worker.capabilities.contains(&capability))
    }

    pub fn service_name() -> ServiceName {
        ServiceName::Cluster
    }
}

#[derive(Debug, Default)]
pub struct WeightedScheduler;

impl WeightedScheduler {
    pub fn new() -> Self {
        Self
    }

    pub fn select(
        &self,
        registry: &WorkerRegistry,
        capability: Capability,
        preferred: Option<&str>,
    ) -> NexoraResult<WorkerRegistration> {
        // The workers come out of the registry in an arbitrary order, so the
        // candidates are sorted by id: the same registry always picks the same
        // worker for the same work.
        let mut candidates: Vec<WorkerRegistration> = registry
            .with_capability(capability)
            .filter(|worker| worker.healthy)
            .collect();
        candidates.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
        candidates
            .into_iter()
            // `min_by_key` keeps the first of the equal candidates, and the
            // score is negated so the worker with the most headroom wins.
            .min_by_key(|worker| {
                let affinity = preferred
                    .map(|value| u32::from(worker.id.as_str() == value))
                    .unwrap_or_default();
                u32::MAX - worker.headroom().saturating_add(affinity)
            })
            .ok_or_else(|| NexoraError::service("no eligible worker"))
    }
}

/// Routes sessions to the workers that run them.
///
/// A session stays on the worker it was routed to, so the work it has already
/// done does not have to be repeated. When that worker goes away or stops
/// taking work, the session is routed again.
#[derive(Debug, Default)]
pub struct ClusterRouter {
    sessions: HashMap<String, WorkerId>,
    scheduler: WeightedScheduler,
}

impl ClusterRouter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Routes a session to a worker that has the capability.
    ///
    /// The worker the session is already on is kept when it can still take
    /// the work; otherwise the session is routed again.
    pub fn route(
        &mut self,
        registry: &WorkerRegistry,
        session: &str,
        capability: Capability,
    ) -> NexoraResult<WorkerId> {
        if let Some(id) = self.sessions.get(session) {
            let eligible = registry
                .worker(id)
                .filter(|worker| worker.healthy && worker.capabilities.contains(&capability));
            if let Some(worker) = eligible {
                return Ok(worker.id);
            }
        }
        let worker = self.scheduler.select(registry, capability, None)?;
        self.sessions.insert(session.to_owned(), worker.id.clone());
        Ok(worker.id)
    }

    /// The worker a session is on.
    pub fn worker_for(&self, session: &str) -> Option<&WorkerId> {
        self.sessions.get(session)
    }

    /// Forgets a session, so the next call routes it again.
    pub fn release(&mut self, session: &str) {
        self.sessions.remove(session);
    }

    /// The sessions the router is holding.
    pub fn sessions(&self) -> usize {
        self.sessions.len()
    }
}
