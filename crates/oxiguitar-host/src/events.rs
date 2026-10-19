// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The queue of events waiting to be pushed to plugins
//! ([`crate::Plugins::push`]).
//!
//! One thread delivers them, in order. It sleeps on a condition variable
//! until an event arrives, so nothing polls. Events that only say where
//! something is now (the selection, the playback position, the score's
//! revision) replace one of the same kind still waiting at the end of the
//! queue: a plugin that is slow to answer gets the latest, not a backlog.

use std::collections::VecDeque;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};

use oxiguitar_plugin_api::event::Event;

/// Events waiting, and whether one is being delivered.
#[derive(Debug, Default)]
pub(crate) struct Queue {
    state: Mutex<State>,
    changed: Condvar,
}

#[derive(Debug, Default)]
struct State {
    waiting: VecDeque<Event>,
    /// An event has been taken and is being delivered.
    busy: bool,
    /// The delivery thread has been started.
    started: bool,
    /// The host has gone (or the thread couldn't start): nothing more is
    /// delivered.
    closed: bool,
}

/// Whether `newer` replaces `older` while both wait: both say where
/// something is now.
fn replaces(older: &Event, newer: &Event) -> bool {
    matches!(
        (older, newer),
        (Event::ScoreChanged { .. }, Event::ScoreChanged { .. })
            | (Event::Selection(_), Event::Selection(_))
            | (Event::Selection2(_), Event::Selection2(_))
            | (Event::Playback(_), Event::Playback(_))
    )
}

impl Queue {
    fn lock(&self) -> MutexGuard<'_, State> {
        // Every change below is complete before the lock is released.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Adds `event`. True when the caller must start the delivery thread
    /// (the first event ever).
    pub(crate) fn push(&self, event: Event) -> bool {
        let mut s = self.lock();
        if s.closed {
            return false;
        }
        // The selection is pushed in both its forms, one after the other
        // (Event::Selection and Event::Selection2): each replaces its own
        // kind, just behind the other.
        let n = s.waiting.len();
        let pair = matches!(
            (s.waiting.back(), &event),
            (Some(Event::Selection(_)), Event::Selection2(_))
                | (Some(Event::Selection2(_)), Event::Selection(_))
        );
        let at = if pair {
            n.checked_sub(2)
        } else {
            n.checked_sub(1)
        };
        match at.and_then(|i| s.waiting.get_mut(i)) {
            Some(older) if replaces(older, &event) => *older = event,
            _ => s.waiting.push_back(event),
        }
        let start = !s.started;
        s.started = true;
        drop(s);
        self.changed.notify_all();
        start
    }

    /// The next event to deliver, waiting until there is one; `None` once
    /// the queue is closed. Call [`Queue::done`] after delivering it.
    pub(crate) fn next(&self) -> Option<Event> {
        let mut s = self.lock();
        loop {
            if s.closed {
                return None;
            }
            if let Some(event) = s.waiting.pop_front() {
                s.busy = true;
                return Some(event);
            }
            s = self.changed.wait(s).unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// The event [`Queue::next`] gave has been delivered.
    pub(crate) fn done(&self) {
        self.lock().busy = false;
        self.changed.notify_all();
    }

    /// Delivers nothing more, and wakes everyone waiting.
    pub(crate) fn close(&self) {
        let mut s = self.lock();
        s.closed = true;
        s.waiting.clear();
        drop(s);
        self.changed.notify_all();
    }

    /// Waits until nothing is waiting or being delivered (or the queue is
    /// closed).
    pub(crate) fn wait_idle(&self) {
        let mut s = self.lock();
        while !s.closed && (s.busy || !s.waiting.is_empty()) {
            s = self.changed.wait(s).unwrap_or_else(PoisonError::into_inner);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxiguitar_plugin_api::event::{Playback, Position};
    use std::sync::Arc;

    fn playing(bar: u32) -> Event {
        Event::Playback(Playback {
            playing: true,
            at: Position { bar, beat: 0 },
            seconds: f64::from(bar),
        })
    }

    #[test]
    fn positions_replace_each_other_while_they_wait_and_scores_do_not() {
        let q = Queue::default();
        assert!(q.push(Event::ScoreOpened { revision: 1 }));
        assert!(!q.push(playing(1)));
        q.push(playing(2));
        q.push(Event::ScoreChanged { revision: 2 });
        q.push(Event::ScoreChanged { revision: 3 });
        q.push(Event::ScoreClosed);
        q.push(Event::ScoreClosed);
        q.push(playing(3));
        let mut got = Vec::new();
        while q.lock().waiting.front().is_some() {
            got.push(q.next().unwrap());
            q.done();
        }
        assert_eq!(
            got,
            [
                Event::ScoreOpened { revision: 1 },
                playing(2),
                Event::ScoreChanged { revision: 3 },
                Event::ScoreClosed,
                Event::ScoreClosed,
                playing(3),
            ]
        );
    }

    #[test]
    fn a_thread_waits_for_events_and_ends_when_closed() {
        let q = Arc::new(Queue::default());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let thread = {
            let (q, seen) = (Arc::clone(&q), Arc::clone(&seen));
            std::thread::spawn(move || {
                while let Some(e) = q.next() {
                    seen.lock().unwrap().push(e);
                    q.done();
                }
            })
        };
        q.push(Event::ScoreOpened { revision: 7 });
        q.wait_idle();
        assert_eq!(*seen.lock().unwrap(), [Event::ScoreOpened { revision: 7 }]);
        q.close();
        thread.join().unwrap();
        assert!(!q.push(Event::ScoreClosed));
        q.wait_idle();
    }
}
