//! Which processes belong to a session: its program and everything below it.

use std::collections::BTreeSet;

/// `roots` and every process below them, from `(pid, parent pid)` pairs, in pid order. A root
/// that is no longer in the table is still included.
pub fn descendants(roots: &[u32], parents: &[(u32, u32)]) -> Vec<u32> {
    let mut found: BTreeSet<u32> = roots.iter().copied().collect();
    loop {
        let before = found.len();
        for &(pid, parent) in parents {
            if found.contains(&parent) {
                found.insert(pid);
            }
        }
        if found.len() == before {
            return found.into_iter().collect();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tree_is_followed_down_whatever_the_table_order() {
        // 10 -> 11 -> 12 -> 13, an unrelated 20 -> 21, and 99 already gone.
        let table = [(13, 12), (21, 20), (12, 11), (11, 10), (20, 1)];
        let cases: [(&[u32], Vec<u32>); 3] = [
            (&[10], vec![10, 11, 12, 13]),
            (&[10, 20], vec![10, 11, 12, 13, 20, 21]),
            (&[99], vec![99]),
        ];
        for (roots, want) in cases {
            assert_eq!(descendants(roots, &table), want, "{roots:?}");
        }
    }
}
