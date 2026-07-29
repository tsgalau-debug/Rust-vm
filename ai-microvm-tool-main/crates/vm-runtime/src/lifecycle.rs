//! Aturan siklus hidup VM. Murni & deterministik — seperti tabel transisi state proses.
use shared_types::VmState;

pub fn can_transition(from: VmState, to: VmState) -> bool {
    use VmState::*;
    matches!(
        (from, to),
        (Building, Warming)
            | (Warming, Ready | Faulted)
            | (Ready, Leased | Snapshotting | Paused | Destroyed)
            | (Leased, Running)
            | (Running, Scrubbing | Faulted | Paused)
            | (Scrubbing, Ready | Destroyed)
            | (Snapshotting, Ready | Faulted)
            | (Faulted, Warming | Destroyed)
            | (Paused, Running | Ready | Destroyed)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use VmState::*;

    #[test]
    fn valid_transitions() {
        for (a, b) in [
            (Building, Warming), (Warming, Ready), (Ready, Leased), (Leased, Running),
            (Running, Scrubbing), (Scrubbing, Ready), (Running, Paused), (Paused, Running),
            (Faulted, Warming), (Ready, Destroyed),
        ] {
            assert!(can_transition(a, b), "{a:?}->{b:?} seharusnya valid");
        }
    }

    #[test]
    fn invalid_transitions() {
        for (a, b) in [
            (Destroyed, Running), (Ready, Running), (Building, Ready),
            (Running, Ready), (Leased, Ready),
        ] {
            assert!(!can_transition(a, b), "{a:?}->{b:?} seharusnya dilarang");
        }
    }
}
