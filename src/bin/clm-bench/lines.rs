pub const DEFAULT_PASSES: usize = 1000;
pub const WORST: usize = 5;

pub fn spoken_column(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.split('\t').next())
        .map(|s| s.trim_end_matches('\r').trim())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn per_line(samples: &[u32], lines: usize, p: f64) -> Vec<u32> {
    (0..lines)
        .map(|i| {
            let mut v: Vec<u32> = samples.iter().skip(i).step_by(lines).copied().collect();
            v.sort_unstable();
            crate::replay::percentile(&v, p)
        })
        .collect()
}

pub fn worst(per_line: &[u32], k: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..per_line.len()).collect();
    idx.sort_by(|&a, &b| per_line[b].cmp(&per_line[a]));
    idx.truncate(k);
    idx
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AllocSummary {
    pub calls: usize,
    pub total: u64,
    pub max: u64,
    pub lines_allocating: usize,
}

pub fn alloc_summary(per_line: &[u64]) -> AllocSummary {
    AllocSummary {
        calls: per_line.len(),
        total: per_line.iter().sum(),
        max: per_line.iter().copied().max().unwrap_or(0),
        lines_allocating: per_line.iter().filter(|&&n| n > 0).count(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spoken_column_takes_the_first_tab_field_of_real_lines() {
        let text = "# comment\nfive pm\t5 p.m.\ttime\r\n\n  \nno tab line\n\twritten only\n";
        assert_eq!(spoken_column(text), ["five pm", "no tab line"]);
    }

    #[test]
    fn per_line_percentile_reads_each_line_across_passes() {
        let samples = [10, 1, 20, 2, 30, 3, 40, 4];
        assert_eq!(per_line(&samples, 2, 1.0), [40, 4]);
        assert_eq!(per_line(&samples, 2, 0.5), [20, 2]);
        assert_eq!(per_line(&samples, 2, 0.0), [10, 1]);
    }

    #[test]
    fn worst_lists_indexes_slowest_first_and_stable_on_ties() {
        assert_eq!(worst(&[5, 9, 1, 9, 7], 3), [1, 3, 4]);
        assert_eq!(worst(&[5, 9], 5), [1, 0]);
        assert!(worst(&[], 5).is_empty());
    }

    #[test]
    fn alloc_summary_counts_lines_that_allocate() {
        let s = alloc_summary(&[0, 2, 0, 5, 1]);
        assert_eq!(
            s,
            AllocSummary {
                calls: 5,
                total: 8,
                max: 5,
                lines_allocating: 3
            }
        );
        assert_eq!(alloc_summary(&[]), AllocSummary::default());
    }
}
