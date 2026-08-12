//! A narrow Dialog-style transition trace for the final CNOT phase network.
//!
//! Each symbol is an elementary invertible matrix over GF(2), embedded on two
//! of the q14--q16 parity-basis rows.  Replaying the ordered trace exposes the
//! requested ZZ parities; replaying the remaining inverse transitions restores
//! the identity basis.  This is a transfer of the paper's linear replay idea,
//! not an EEA Dialog and not a Clifford-tableau representation.

#[derive(Clone, Copy)]
struct CnotTransition {
    control: usize,
    target: usize,
}

impl CnotTransition {
    const fn new(control: usize, target: usize) -> Self {
        Self { control, target }
    }

    fn apply(self, rows: &mut [u8; 3]) {
        rows[self.target] ^= rows[self.control];
    }
}

const TRACE: [CnotTransition; 6] = [
    CnotTransition::new(0, 1),
    CnotTransition::new(0, 1),
    CnotTransition::new(1, 2),
    CnotTransition::new(1, 0),
    CnotTransition::new(1, 2),
    CnotTransition::new(1, 0),
];

pub(super) fn emit(
    lines: &mut Vec<String>,
    left_edge_angle: &str,
    right_edge_angle: &str,
    mixer_angle: &str,
) {
    let mut rows = [0b001_u8, 0b010_u8, 0b100_u8];

    for (index, transition) in TRACE.into_iter().enumerate() {
        if index == 4 {
            assert_eq!(rows[0], 0b011, "left ZZ parity must be implicit row 0");
            assert_eq!(rows[2], 0b110, "right ZZ parity must be implicit row 2");
            lines.push(format!("rz({left_edge_angle}) q[14];"));
            lines.push(format!("rz({right_edge_angle}) q[16];"));
        }

        lines.push(format!(
            "cx q[{}], q[{}];",
            14 + transition.control,
            14 + transition.target
        ));
        transition.apply(&mut rows);
    }

    assert_eq!(rows, [0b001, 0b010, 0b100], "Dialog replay must close");

    // The X-basis mixer is outside the GF(2) linear Dialog.
    lines.push("h q[15];".to_string());
    lines.push(format!("rz({mixer_angle}) q[15];"));
    lines.push("h q[15];".to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_exposes_requested_parities_and_closes() {
        let mut rows = [0b001_u8, 0b010_u8, 0b100_u8];
        for (index, transition) in TRACE.into_iter().enumerate() {
            if index == 4 {
                assert_eq!(rows[0], 0b011);
                assert_eq!(rows[2], 0b110);
            }
            transition.apply(&mut rows);
        }
        assert_eq!(rows, [0b001, 0b010, 0b100]);
    }
}
