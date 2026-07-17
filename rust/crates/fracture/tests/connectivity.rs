//! Demonstrates the payoff of a Bevy-free crate: the fracture graph is
//! unit-testable with zero engine/renderer setup.

use fracture::{connected_components, count_components, Bond};

fn bond(a: usize, b: usize) -> Bond {
    Bond {
        a,
        b,
        ..Default::default()
    }
}

#[test]
fn a_broken_bond_splits_a_chain_in_two() {
    // 3 cells in a line: 0 — 1 — 2. Break the 1—2 bond.
    let bonds = [bond(0, 1), bond(1, 2)];
    let broken = [false, true];
    let pulverized = [false, false, false];

    let (labels, count) = connected_components(3, &bonds, &broken, &pulverized);
    assert_eq!(count, 2);
    assert_eq!(labels[0], labels[1]);
    assert_ne!(labels[1], labels[2]);
}

#[test]
fn a_pulverized_cell_is_its_own_nothing() {
    let bonds = [bond(0, 1)];
    let broken = [false];
    let pulverized = [false, true]; // cell 1 vaporised
    assert_eq!(count_components(2, &bonds, &broken, &pulverized), 1);
}
