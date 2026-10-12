// Reproduce the MSVC v143 std::default_random_engine (MT19937) and
// std::shuffle used by the supplied Windows build. Keep the algorithm here
// so other platforms can retain Windows document-command behavior.
pub(crate) fn shuffle<T>(items: &mut [T], seed: u64) {
    let mut rng = Mt19937::new(seed as u32);
    for target in 1..items.len() {
        let offset = rng.below(target as u64 + 1) as usize;
        items.swap(target, offset);
    }
}

struct Mt19937 {
    state: [u32; 624],
    position: usize,
}

impl Mt19937 {
    fn new(seed: u32) -> Self {
        let mut state = [0; 624];
        state[0] = seed;
        for index in 1..624 {
            let previous = state[index - 1];
            state[index] = 1812433253u32
                .wrapping_mul(previous ^ (previous >> 30))
                .wrapping_add(index as u32);
        }
        Self {
            state,
            position: 624,
        }
    }

    fn next(&mut self) -> u32 {
        if self.position == 624 {
            for index in 0..624 {
                let combined =
                    (self.state[index] & 0x80000000) | (self.state[(index + 1) % 624] & 0x7fffffff);
                self.state[index] = self.state[(index + 397) % 624]
                    ^ (combined >> 1)
                    ^ if combined & 1 != 0 { 0x9908b0df } else { 0 };
            }
            self.position = 0;
        }
        let mut value = self.state[self.position];
        self.position += 1;
        value ^= value >> 11;
        value ^= (value << 7) & 0x9d2c5680;
        value ^= (value << 15) & 0xefc60000;
        value ^ (value >> 18)
    }

    fn below(&mut self, bound: u64) -> u64 {
        // MSVC's adapter concatenates 32-bit outputs when necessary and
        // rejects the incomplete bucket at the top of the generated range.
        loop {
            let mut value = 0u64;
            let mut mask = 0u64;
            while mask < bound - 1 {
                value = (value << 32) | u64::from(self.next());
                mask = (mask << 32) | u64::from(u32::MAX);
            }
            if value / bound < mask / bound || mask % bound == bound - 1 {
                return value % bound;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn bounded_draws_match_msvc_rejection_and_wide_ranges() {
        let mut previous_seed = None;
        let mut rng = super::Mt19937::new(0);
        let mut cases = 0;
        for line in include_str!("../tests/fixtures/random_bounds.tsv").lines() {
            let mut fields = line.split('\t');
            let seed: u32 = fields.next().unwrap().parse().unwrap();
            let bound: u64 = fields.next().unwrap().parse().unwrap();
            if previous_seed != Some(seed) {
                rng = super::Mt19937::new(seed);
                previous_seed = Some(seed);
            }
            for expected in fields.next().unwrap().split(',') {
                assert_eq!(
                    rng.below(bound),
                    expected.parse::<u64>().unwrap(),
                    "seed {seed}, bound {bound}"
                );
            }
            cases += 1;
        }
        assert_eq!(cases, 18);
    }

    #[test]
    fn matches_msvc_v143_fixture_permutations() {
        let fixture = include_str!("../tests/fixtures/random_sort.tsv");
        let mut cases = 0;
        for line in fixture.lines() {
            let mut fields = line.split('\t');
            let seed: u64 = fields.next().unwrap().parse().unwrap();
            let count: usize = fields.next().unwrap().parse().unwrap();
            let expected: Vec<usize> = fields
                .next()
                .unwrap()
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.parse().unwrap())
                .collect();
            let mut actual: Vec<_> = (0..count).collect();
            super::shuffle(&mut actual, seed);
            assert_eq!(actual, expected, "seed {seed}, count {count}");
            let mut wrapped: Vec<_> = (0..count).collect();
            super::shuffle(&mut wrapped, seed + (1u64 << 32));
            assert_eq!(wrapped, expected, "source truncates seed to unsigned");
            cases += 1;
        }
        assert_eq!(cases, 40);
    }
}
