//! Driving a step machine in tests.

/// Feeds `inputs` in order through `step`, and returns the last state with every output in
/// order.
pub fn run_steps<State, In, Out>(
    from: State,
    inputs: impl IntoIterator<Item = In>,
    mut step: impl FnMut(State, In) -> (State, Vec<Out>),
) -> (State, Vec<Out>) {
    let mut state = from;
    let mut outs = Vec::new();
    for input in inputs {
        let (next, more) = step(state, input);
        state = next;
        outs.extend(more);
    }
    (state, outs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_run_in_order_and_their_outputs_are_kept_in_order() {
        let (state, outs) = run_steps(0, [1, 2, 3], |total, n| (total + n, vec![total, n]));
        assert_eq!(state, 6);
        assert_eq!(outs, vec![0, 1, 1, 2, 3, 3]);
    }
}
