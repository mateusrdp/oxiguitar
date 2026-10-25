// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The order bars are played in: repeats, alternate endings and the
//! navigation signs (PLAY-4).
//!
//! The rules, as Guitar Pro plays them (docs/design/player.md):
//!
//! - A closing repeat goes back to the last opening repeat; without one,
//!   to the bar after the previous closing repeat, or the first bar (as
//!   MuseScore and Finale do). The section plays as many times as the
//!   closing repeat says, 2 for an ordinary repeat.
//! - A bar with alternate endings plays only on the passes it names.
//! - D.C. goes back to the first bar, D.S. to the segno, D.S.S. to the
//!   double segno, at the end of the bar that holds the jump, each jump
//!   once. After a jump, repeats play once and endings play their last
//!   ending; "al coda" turns on To Coda (a jump, at the end of its bar, to
//!   the bar with the coda sign) and "al fine" turns on Fine (the music ends
//!   at the end of its bar). Before any jump, To Coda and Fine are ignored.

use oxiguitar_model::{Direction, MasterBar};

/// One bar as played.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Visit {
    /// Index of the master bar.
    pub bar: usize,
    /// Which time through its repeat section, from 1.
    pub pass: u8,
}

/// Where a jump goes and what it turns on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Jump {
    /// To the first bar.
    Start(After),
    /// To the bar with the segno.
    Segno(After),
    /// To the bar with the double segno.
    SegnoSegno(After),
}

/// What a jump turns on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum After {
    /// Play to the end.
    End,
    /// Follow To Coda to the coda.
    Coda,
    /// Follow To Double Coda to the double coda.
    DoubleCoda,
    /// Stop at Fine.
    Fine,
}

fn jump_of(d: Direction) -> Option<Jump> {
    use Direction as D;
    Some(match d {
        D::DaCapo => Jump::Start(After::End),
        D::DaCapoAlCoda => Jump::Start(After::Coda),
        D::DaCapoAlDoubleCoda => Jump::Start(After::DoubleCoda),
        D::DaCapoAlFine => Jump::Start(After::Fine),
        D::DalSegno => Jump::Segno(After::End),
        D::DalSegnoAlCoda => Jump::Segno(After::Coda),
        D::DalSegnoAlDoubleCoda => Jump::Segno(After::DoubleCoda),
        D::DalSegnoAlFine => Jump::Segno(After::Fine),
        D::DalSegnoSegno => Jump::SegnoSegno(After::End),
        D::DalSegnoSegnoAlCoda => Jump::SegnoSegno(After::Coda),
        D::DalSegnoSegnoAlDoubleCoda => Jump::SegnoSegno(After::DoubleCoda),
        D::DalSegnoSegnoAlFine => Jump::SegnoSegno(After::Fine),
        D::Segno
        | D::SegnoSegno
        | D::Coda
        | D::DoubleCoda
        | D::Fine
        | D::ToCoda
        | D::ToDoubleCoda => {
            return None;
        }
    })
}

/// The first bar holding `sign`, if any.
fn find(bars: &[MasterBar], sign: Direction) -> Option<usize> {
    bars.iter().position(|b| b.directions.contains(&sign))
}

/// The highest ending number in the run of bars with endings that `bar`
/// belongs to (1-based): the ending played after a jump.
fn last_ending(bars: &[MasterBar], bar: usize) -> u8 {
    let mut first = bar;
    while first > 0 && bars[first - 1].alternate_endings != 0 {
        first -= 1;
    }
    let mut mask = 0u16;
    for b in &bars[first..] {
        if b.alternate_endings == 0 {
            break;
        }
        mask |= b.alternate_endings;
    }
    // 16 - leading zeros = the highest bit's number, from 1.
    (16 - mask.leading_zeros()) as u8
}

/// The bars in the order they are played.
///
/// Never loops forever: each jump is taken once and each repeat counts its
/// passes; as a last guard, it stops after 320 times the number of bars.
pub fn playing_order(bars: &[MasterBar]) -> Vec<Visit> {
    // Repeats count up to 255, and each jump is taken once: generous room.
    let limit = bars.len().saturating_mul(320).max(320);
    let mut visits = Vec::with_capacity(bars.len());
    let mut i = 0;
    let mut pass: u8 = 1;
    // Where a closing repeat goes back to.
    let mut repeat_start = 0;
    // True when the bar at `i` is reached by going back for a repeat, so
    // its opening repeat doesn't start the count again.
    let mut returning = false;
    // After a D.C. or D.S.: repeats once, last endings, To Coda and Fine.
    let mut after: Option<After> = None;
    let mut jumped = vec![false; bars.len()];
    // A repeat section has ended: its endings may still follow, and the
    // first bar without endings starts counting passes again.
    let mut completed = false;

    while i < bars.len() && visits.len() < limit {
        let bar = &bars[i];
        if bar.repeat_start && !returning {
            repeat_start = i;
            pass = 1;
        }
        returning = false;
        if completed && bar.alternate_endings == 0 {
            completed = false;
            pass = 1;
        }

        if bar.alternate_endings != 0 {
            let ending = if after.is_some() {
                last_ending(bars, i)
            } else {
                pass
            };
            let plays =
                (1..=16).contains(&ending) && bar.alternate_endings & (1 << (ending - 1)) != 0;
            if !plays {
                // A closing repeat skipped on its last pass still ends its
                // section.
                if bar.repeat_end.is_some_and(|times| pass >= times) {
                    completed = true;
                    repeat_start = i + 1;
                }
                i += 1;
                continue;
            }
        }

        visits.push(Visit { bar: i, pass });

        if after == Some(After::Fine) && bar.directions.contains(&Direction::Fine) {
            break;
        }
        let to = match after {
            Some(After::Coda) if bar.directions.contains(&Direction::ToCoda) => {
                find(bars, Direction::Coda)
            }
            Some(After::DoubleCoda) if bar.directions.contains(&Direction::ToDoubleCoda) => {
                find(bars, Direction::DoubleCoda)
            }
            _ => None,
        };
        if let Some(to) = to {
            after = Some(After::End);
            i = to;
            pass = 1;
            continue;
        }

        if let Some(times) = bar.repeat_end
            && after.is_none()
        {
            if pass < times {
                pass += 1;
                i = repeat_start;
                returning = true;
                continue;
            }
            // The next closing repeat without an opening one goes back to
            // the bar after this one.
            repeat_start = i + 1;
            completed = true;
        }

        let jump = bar.directions.iter().find_map(|d| jump_of(*d));
        if let Some(jump) = jump
            && !jumped[i]
        {
            jumped[i] = true;
            let (target, then) = match jump {
                Jump::Start(t) => (Some(0), t),
                Jump::Segno(t) => (find(bars, Direction::Segno), t),
                Jump::SegnoSegno(t) => (find(bars, Direction::SegnoSegno), t),
            };
            if let Some(target) = target {
                after = Some(then);
                i = target;
                pass = 1;
                completed = false;
                continue;
            }
        }
        i += 1;
    }
    visits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bars(n: usize) -> Vec<MasterBar> {
        vec![MasterBar::default(); n]
    }

    fn order(b: &[MasterBar]) -> Vec<usize> {
        playing_order(b).iter().map(|v| v.bar + 1).collect()
    }

    #[test]
    fn straight_through() {
        assert_eq!(order(&bars(3)), [1, 2, 3]);
        assert!(playing_order(&[]).is_empty());
    }

    #[test]
    fn a_repeat_plays_twice() {
        let mut b = bars(4);
        b[1].repeat_start = true;
        b[2].repeat_end = Some(2);
        assert_eq!(order(&b), [1, 2, 3, 2, 3, 4]);
        let passes: Vec<u8> = playing_order(&b).iter().map(|v| v.pass).collect();
        assert_eq!(passes, [1, 1, 1, 2, 2, 1], "bar 4 is after the repeat");
    }

    #[test]
    fn a_repeat_without_an_opening_goes_back_to_the_start_or_the_last_repeat() {
        let mut b = bars(4);
        b[1].repeat_end = Some(2);
        b[3].repeat_end = Some(3);
        assert_eq!(order(&b), [1, 2, 1, 2, 3, 4, 3, 4, 3, 4]);
    }

    #[test]
    fn after_endings_a_new_repeat_counts_from_one() {
        // |: 1 |1. 2 :|2. 3 | 4 :|  The second closing repeat has no opening
        // one, so it goes back to the bar after the first section: bar 3.
        let mut b = bars(4);
        b[0].repeat_start = true;
        b[1].alternate_endings = 0b01;
        b[1].repeat_end = Some(2);
        b[2].alternate_endings = 0b10;
        b[3].repeat_end = Some(2);
        assert_eq!(order(&b), [1, 2, 1, 3, 4, 3, 4]);
    }

    #[test]
    fn alternate_endings() {
        // |: 1 2 |1. 3 :|2. 4 | 5
        let mut b = bars(5);
        b[0].repeat_start = true;
        b[2].alternate_endings = 0b01;
        b[2].repeat_end = Some(2);
        b[3].alternate_endings = 0b10;
        assert_eq!(order(&b), [1, 2, 3, 1, 2, 4, 5]);
    }

    #[test]
    fn three_endings_and_a_shared_one() {
        // |: 1 |1.2. 2 :|3. 3 |  repeated three times
        let mut b = bars(3);
        b[0].repeat_start = true;
        b[1].alternate_endings = 0b011;
        b[1].repeat_end = Some(3);
        b[2].alternate_endings = 0b100;
        assert_eq!(order(&b), [1, 2, 1, 2, 1, 3]);
    }

    #[test]
    fn dal_segno_al_coda() {
        // 1 | segno 2 | 3 to coda | 4 D.S. al coda | coda 5
        let mut b = bars(5);
        b[1].directions.push(Direction::Segno);
        b[2].directions.push(Direction::ToCoda);
        b[3].directions.push(Direction::DalSegnoAlCoda);
        b[4].directions.push(Direction::Coda);
        assert_eq!(order(&b), [1, 2, 3, 4, 2, 3, 5]);
    }

    #[test]
    fn da_capo_al_fine_with_a_repeat_played_once() {
        // |: 1 2 fine :| 3 D.C. al fine
        let mut b = bars(3);
        b[0].repeat_start = true;
        b[1].directions.push(Direction::Fine);
        b[1].repeat_end = Some(2);
        b[2].directions.push(Direction::DaCapoAlFine);
        assert_eq!(order(&b), [1, 2, 1, 2, 3, 1, 2]);
    }

    #[test]
    fn after_a_jump_endings_take_the_last() {
        // |: 1 |1. 2 :|2. 3 | 4 D.C.
        let mut b = bars(4);
        b[0].repeat_start = true;
        b[1].alternate_endings = 0b01;
        b[1].repeat_end = Some(2);
        b[2].alternate_endings = 0b10;
        b[3].directions.push(Direction::DaCapo);
        assert_eq!(order(&b), [1, 2, 1, 3, 4, 1, 3, 4]);
    }

    #[test]
    fn to_coda_and_fine_do_nothing_before_a_jump() {
        let mut b = bars(3);
        b[0].directions.push(Direction::ToCoda);
        b[1].directions.push(Direction::Fine);
        b[2].directions.push(Direction::Coda);
        assert_eq!(order(&b), [1, 2, 3]);
    }

    #[test]
    fn a_jump_with_nowhere_to_go_is_ignored_and_loops_end() {
        let mut b = bars(2);
        b[1].directions.push(Direction::DalSegno);
        assert_eq!(order(&b), [1, 2]);
        // A repeat of 255 times plays 255 times.
        let mut b = bars(1);
        b[0].repeat_start = true;
        b[0].repeat_end = Some(255);
        assert_eq!(playing_order(&b).len(), 255);
    }
}
