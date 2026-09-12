use std::time::Duration;

use chrono::{DateTime, Local};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, error, warn};

use crate::parser::Reminder;

/// Event emitted when a reminder fires. Owned strings so the receiver can
/// outlive the scheduler tasks.
#[derive(Debug, Clone)]
pub struct ReminderEvent {
    pub schedule: String,
    pub icon: String,
    pub body: String,
    pub scheduled_for: DateTime<Local>,
    pub fired_at: DateTime<Local>,
}

/// Owns the per-reminder tokio tasks. Drop or call [`Scheduler::shutdown`]
/// to cancel them all.
pub struct Scheduler {
    handles: Vec<JoinHandle<()>>,
}

impl Scheduler {
    /// Spawn one async task per reminder. Each task loops:
    /// `next_after(now) → sleep → send event`. The reference time
    /// is recomputed from `Local::now()` on every iteration so wake-from-sleep
    /// fires once for the next future occurrence rather than catching up on
    /// missed ones (docs/specs/spec.md § 5.1).
    pub fn spawn(reminders: Vec<Reminder>, tx: mpsc::Sender<ReminderEvent>) -> Self {
        let mut handles = Vec::with_capacity(reminders.len());
        for reminder in reminders {
            let tx = tx.clone();
            handles.push(tokio::spawn(run_one(reminder, tx)));
        }
        Self { handles }
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        for h in &self.handles {
            h.abort();
        }
    }
}

async fn run_one(reminder: Reminder, tx: mpsc::Sender<ReminderEvent>) {
    loop {
        let now = Local::now();
        let Some(next) = reminder.cron.next_after(now) else {
            error!(
                schedule = %reminder.schedule,
                "schedule has no upcoming occurrence; stopping this reminder"
            );
            return;
        };

        let delta = (next - now).to_std().unwrap_or(Duration::ZERO);
        debug!(
            schedule = %reminder.schedule,
            next = %next,
            delta_secs = delta.as_secs(),
            "next fire scheduled"
        );

        tokio::time::sleep(delta).await;

        let event = ReminderEvent {
            schedule: reminder.schedule.clone(),
            icon: reminder.icon.clone(),
            body: reminder.body.clone(),
            scheduled_for: next,
            fired_at: Local::now(),
        };
        if tx.send(event).await.is_err() {
            // Receiver gone; we're shutting down.
            warn!(
                schedule = %reminder.schedule,
                "event channel closed; reminder task exiting"
            );
            return;
        }
    }
}
