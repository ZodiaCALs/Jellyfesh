//! Property tests: random sequences of queue operations keep the queue
//! consistent, and `peek_next` always agrees with `next`.

use jf_session::{Advance, Queue, RepeatMode, TrackRef};
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Op {
    SetItems(usize, Option<usize>),
    Append(usize),
    InsertNext(usize),
    Remove(usize),
    Move(usize, usize),
    Jump(usize),
    Next(bool),
    Previous,
    Shuffle(bool),
    Repeat(u8),
    Clear,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0usize..25, proptest::option::of(0usize..30)).prop_map(|(n, s)| Op::SetItems(n, s)),
        (0usize..4).prop_map(Op::Append),
        (0usize..4).prop_map(Op::InsertNext),
        any::<usize>().prop_map(Op::Remove),
        (any::<usize>(), any::<usize>()).prop_map(|(a, b)| Op::Move(a, b)),
        any::<usize>().prop_map(Op::Jump),
        any::<bool>().prop_map(Op::Next),
        Just(Op::Previous),
        any::<bool>().prop_map(Op::Shuffle),
        (0u8..3).prop_map(Op::Repeat),
        Just(Op::Clear),
    ]
}

fn tracks(counter: &mut i64, n: usize) -> Vec<TrackRef> {
    (0..n)
        .map(|_| {
            *counter += 1;
            TrackRef::Library(*counter)
        })
        .collect()
}

fn apply(q: &mut Queue, op: &Op, counter: &mut i64) {
    let len = q.len();
    let uid_at = |q: &Queue, i: usize| q.item_at(i % len).map(|it| it.uid);
    match *op {
        Op::SetItems(n, start) => {
            let start = start.filter(|s| *s < n);
            q.set_items(tracks(counter, n), start).unwrap();
        }
        Op::Append(n) => {
            q.append(tracks(counter, n));
        }
        Op::InsertNext(n) => {
            q.insert_next(tracks(counter, n));
        }
        Op::Remove(i) if len > 0 => {
            let uid = uid_at(q, i).unwrap();
            let was = q.current_uid() == Some(uid);
            assert_eq!(q.remove(uid).unwrap().was_current, was);
        }
        Op::Move(i, to) if len > 0 => {
            let uid = uid_at(q, i).unwrap();
            let cur = q.current_uid();
            q.move_item(uid, to % len).unwrap();
            assert_eq!(q.current_uid(), cur, "a move never changes the current entry");
            assert_eq!(q.index_of(uid), Some(to % len));
        }
        Op::Jump(i) if len > 0 => {
            let uid = uid_at(q, i).unwrap();
            q.jump_to(uid).unwrap();
            assert_eq!(q.current_uid(), Some(uid));
        }
        Op::Next(auto) => {
            q.next(if auto { Advance::Auto } else { Advance::Manual });
        }
        Op::Previous => {
            q.previous();
        }
        Op::Shuffle(on) => {
            let cur = q.current_uid();
            q.set_shuffle(on);
            assert_eq!(q.current_uid(), cur, "toggling shuffle keeps the current entry");
        }
        Op::Repeat(m) => q.set_repeat(match m {
            0 => RepeatMode::Off,
            1 => RepeatMode::All,
            _ => RepeatMode::One,
        }),
        Op::Clear => q.clear(),
        _ => {}
    }
}

fn check_invariants(q: &Queue) {
    let state = q.to_state();
    let mut canonical: Vec<u64> = state.items.iter().map(|i| i.uid).collect();
    let mut order = state.order.clone();
    if !state.shuffle {
        assert_eq!(
            order, canonical,
            "unshuffled playback order is the canonical order"
        );
    }
    canonical.sort_unstable();
    order.sort_unstable();
    assert_eq!(order, canonical, "playback order is a permutation of the entries");
    canonical.dedup();
    assert_eq!(canonical.len(), state.items.len(), "uids are unique");
    if let Some(p) = q.position() {
        assert!(p < q.len());
        assert_eq!(q.current(), q.item_at(p));
    } else {
        assert_eq!(q.current(), None);
    }
    for how in [Advance::Auto, Advance::Manual] {
        let mut probe = q.clone();
        assert_eq!(
            q.peek_next(how),
            probe.next(how),
            "peek_next agrees with next ({how:?})"
        );
    }
    let restored = Queue::from_state(state.clone()).unwrap();
    assert_eq!(restored.to_state(), state, "state round-trips");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn random_operations_keep_the_queue_consistent(seed in any::<u64>(), ops in proptest::collection::vec(op(), 1..60)) {
        let mut q = Queue::with_seed(seed);
        let mut counter = 0;
        for op in &ops {
            apply(&mut q, op, &mut counter);
            check_invariants(&q);
        }
    }

    #[test]
    fn without_repeat_every_entry_plays_exactly_once(seed in any::<u64>(), n in 0usize..40, shuffle in any::<bool>()) {
        let mut q = Queue::with_seed(seed);
        q.set_shuffle(shuffle);
        q.set_items((0..n as i64).map(TrackRef::Library).collect(), None).unwrap();
        let mut played = Vec::new();
        while let Some(item) = q.next(Advance::Auto) {
            played.push(item.uid);
            prop_assert!(played.len() <= n);
        }
        played.sort_unstable();
        played.dedup();
        prop_assert_eq!(played.len(), n);
    }

    #[test]
    fn previous_undoes_next_inside_the_queue(seed in any::<u64>(), n in 2usize..30, start in 0usize..29, shuffle in any::<bool>()) {
        let start = start % (n - 1);
        let mut q = Queue::with_seed(seed);
        q.set_items((0..n as i64).map(TrackRef::Library).collect(), Some(start)).unwrap();
        q.set_shuffle(shuffle);
        let before = q.current();
        if q.position().unwrap() + 1 < q.len() {
            q.next(Advance::Manual).unwrap();
            prop_assert_eq!(q.previous(), before);
        }
    }

    #[test]
    fn repeat_all_cycles_play_every_entry_once(seed in any::<u64>(), n in 1usize..20, cycles in 1usize..5) {
        let mut q = Queue::with_seed(seed);
        q.set_items((0..n as i64).map(TrackRef::Library).collect(), None).unwrap();
        q.set_shuffle(true);
        q.set_repeat(RepeatMode::All);
        let mut last = None;
        for _ in 0..cycles {
            let mut cycle = Vec::new();
            for _ in 0..n {
                let item = q.next(Advance::Auto).unwrap();
                if n > 1 {
                    prop_assert_ne!(Some(item.uid), last);
                }
                last = Some(item.uid);
                cycle.push(item.uid);
            }
            cycle.sort_unstable();
            cycle.dedup();
            prop_assert_eq!(cycle.len(), n);
        }
    }
}
