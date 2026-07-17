//! Geometry + physics of a fracture — port of `FractureSimulator.cs`.
//!
//! These are pure functions over plain data. A couple of the graph helpers are
//! implemented to set the porting pattern; the energy/fling-heavy ones are
//! `todo!()` stubs with the exact C# entry point named.

use crate::contract::{FractureInput, FragmentSpec};
use crate::kernel::CrackFront;
use crate::process::LivePiece;
use crate::types::{Bond, FracturableBody};

/// Precompute per-bond spin multipliers and per-cell adjacency for a hit.
/// Port of `FractureSimulator.PrepareGraph`.
pub fn prepare_graph(body: &FracturableBody, spin_omega: f32) -> (Vec<f32>, Vec<Vec<usize>>) {
    let adj = build_adjacency(body.cells.len(), &body.bonds);
    let spin_mul = compute_spin_mul(body, spin_omega);
    (spin_mul, adj)
}

/// Per-cell list of bond indices. Port of `BuildAdjacency`.
fn build_adjacency(n: usize, bonds: &[Bond]) -> Vec<Vec<usize>> {
    let mut adj = vec![Vec::new(); n];
    for (i, b) in bonds.iter().enumerate() {
        adj[b.a].push(i);
        adj[b.b].push(i);
    }
    adj
}

fn compute_spin_mul(_body: &FracturableBody, _omega: f32) -> Vec<f32> {
    todo!("port FractureSimulator.ComputeSpinMul")
}

/// Label each cell with its connected component over surviving (non-broken,
/// non-pulverized) bonds. Returns `(labels, component_count)`.
/// Port of `ConnectedComponents` — a flood fill; ports verbatim.
pub fn connected_components(
    n: usize,
    bonds: &[Bond],
    broken: &[bool],
    pulverized: &[bool],
) -> (Vec<i32>, usize) {
    let mut label = vec![-1i32; n];
    let mut count = 0usize;
    let adj = {
        let mut a = vec![Vec::new(); n];
        for (i, b) in bonds.iter().enumerate() {
            if !broken[i] {
                a[b.a].push(b.b);
                a[b.b].push(b.a);
            }
        }
        a
    };
    let mut stack = Vec::new();
    for start in 0..n {
        if label[start] != -1 || pulverized[start] {
            continue;
        }
        label[start] = count as i32;
        stack.push(start);
        while let Some(c) = stack.pop() {
            for &nb in &adj[c] {
                if label[nb] == -1 && !pulverized[nb] {
                    label[nb] = count as i32;
                    stack.push(nb);
                }
            }
        }
        count += 1;
    }
    (label, count)
}

/// Number of resulting fragments without materialising labels. Port of `CountComponents`.
pub fn count_components(n: usize, bonds: &[Bond], broken: &[bool], pulverized: &[bool]) -> usize {
    connected_components(n, bonds, broken, pulverized).1
}

/// Build the final fragment bodies (re-centre cells, derive mass/inertia/motion).
/// Port of `BuildResult` + `BuildComponentSpec` + `DerivedMotion` + `InertiaAbout`.
pub fn build_result(
    _body: &FracturableBody,
    _input: &FractureInput,
    _broken: &[bool],
    _pulverized: &[bool],
    _fling_e: &[f32],
) -> Vec<FragmentSpec> {
    todo!("port FractureSimulator.BuildResult")
}

/// Advance a live multi-frame fracture one iteration, emitting any pieces that
/// disconnected this step. Port of `FractureSimulator.SplitLive` + `PartitionFront`.
pub fn split_live(
    _body: &FracturableBody,
    _input: &FractureInput,
    _fronts: &mut Vec<CrackFront>,
    /* shared broken/pulverized/fling state */
) -> Vec<LivePiece> {
    todo!("port FractureSimulator.SplitLive")
}
