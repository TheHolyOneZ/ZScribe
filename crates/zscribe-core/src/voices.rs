#[derive(Debug, Clone)]
pub struct Utterance {
    pub print: Vec<f32>,

    pub frames: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct VoiceOptions {
    pub threshold: f32,

    pub max_speakers: usize,

    pub min_frames: usize,

    pub max_seeds: usize,
}

impl Default for VoiceOptions {
    fn default() -> Self {
        Self {
            threshold: 0.28,
            max_speakers: 6,
            min_frames: 100,
            max_seeds: 240,
        }
    }
}

pub fn cluster(utterances: &[Utterance], options: &VoiceOptions) -> Vec<Option<usize>> {
    let mut seeds: Vec<usize> = utterances
        .iter()
        .enumerate()
        .filter(|(_, utterance)| {
            !utterance.print.is_empty() && utterance.frames >= options.min_frames
        })
        .map(|(index, _)| index)
        .collect();

    if seeds.len() > options.max_seeds {
        seeds.sort_by_key(|index| std::cmp::Reverse(utterances[*index].frames));
        seeds.truncate(options.max_seeds);
        seeds.sort_unstable();
    }

    if seeds.is_empty() {
        return vec![None; utterances.len()];
    }

    let mut groups: Vec<Vec<usize>> = seeds.iter().map(|index| vec![*index]).collect();

    while let Some((left, right, distance)) = closest(&groups, utterances) {
        let over_the_cap = groups.len() > options.max_speakers;
        if !over_the_cap && distance > options.threshold {
            break;
        }

        let merged = groups.remove(right);
        groups[left].extend(merged);
    }

    groups.sort_by_key(|group| group.iter().copied().min().unwrap_or(usize::MAX));

    let mut labels = vec![None; utterances.len()];
    for (speaker, group) in groups.iter().enumerate() {
        for index in group {
            labels[*index] = Some(speaker);
        }
    }

    let centroids: Vec<Vec<f32>> = groups
        .iter()
        .map(|group| centroid(group, utterances))
        .collect();

    for (index, utterance) in utterances.iter().enumerate() {
        if labels[index].is_some() || utterance.print.is_empty() {
            continue;
        }

        labels[index] = centroids
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                distance(&utterance.print, a).total_cmp(&distance(&utterance.print, b))
            })
            .map(|(speaker, _)| speaker);
    }

    labels
}

fn closest(groups: &[Vec<usize>], utterances: &[Utterance]) -> Option<(usize, usize, f32)> {
    if groups.len() < 2 {
        return None;
    }

    let mut best: Option<(usize, usize, f32)> = None;

    for left in 0..groups.len() {
        for right in (left + 1)..groups.len() {
            let mut total = 0.0f32;
            let mut pairs = 0usize;

            for a in &groups[left] {
                for b in &groups[right] {
                    total += distance(&utterances[*a].print, &utterances[*b].print);
                    pairs += 1;
                }
            }

            let average = if pairs == 0 {
                1.0
            } else {
                total / pairs as f32
            };
            if best.is_none_or(|(_, _, current)| average < current) {
                best = Some((left, right, average));
            }
        }
    }

    best
}

fn centroid(group: &[usize], utterances: &[Utterance]) -> Vec<f32> {
    let width = group
        .iter()
        .filter_map(|index| {
            let print = &utterances[*index].print;
            (!print.is_empty()).then_some(print.len())
        })
        .max()
        .unwrap_or(0);

    let mut mean = vec![0.0f32; width];
    let mut counted = 0usize;

    for index in group {
        let print = &utterances[*index].print;
        if print.len() != width {
            continue;
        }
        for (slot, value) in mean.iter_mut().zip(print) {
            *slot += value;
        }
        counted += 1;
    }

    if counted > 0 {
        for slot in &mut mean {
            *slot /= counted as f32;
        }
    }
    mean
}

pub fn distance(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 1.0;
    }

    let mut dot = 0.0f32;
    let mut left = 0.0f32;
    let mut right = 0.0f32;

    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        left += x * x;
        right += y * y;
    }

    let magnitude = left.sqrt() * right.sqrt();
    if magnitude == 0.0 {
        return 1.0;
    }

    1.0 - (dot / magnitude)
}

pub fn default_name(speaker: usize) -> String {
    format!("Speaker {}", speaker + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn like(direction: &[f32], wobble: f32, seed: usize) -> Utterance {
        let print = direction
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let jitter = ((seed * 31 + index * 17) % 13) as f32 / 13.0 - 0.5;
                value + jitter * wobble
            })
            .collect();

        Utterance { print, frames: 200 }
    }

    const ANNA: [f32; 6] = [1.0, 0.2, -0.4, 0.9, 0.1, -0.2];
    const MAX: [f32; 6] = [-0.3, 0.9, 0.7, -0.5, 0.8, 0.4];

    #[test]
    fn one_person_talking_is_one_speaker() {
        let utterances: Vec<Utterance> = (0..8).map(|seed| like(&ANNA, 0.2, seed)).collect();
        let labels = cluster(&utterances, &VoiceOptions::default());

        assert!(labels.iter().all(|label| *label == Some(0)), "{labels:?}");
    }

    #[test]
    fn two_people_taking_turns_are_two_speakers() {
        let utterances: Vec<Utterance> = (0..10)
            .map(|seed| {
                if seed % 2 == 0 {
                    like(&ANNA, 0.15, seed)
                } else {
                    like(&MAX, 0.15, seed)
                }
            })
            .collect();

        let labels = cluster(&utterances, &VoiceOptions::default());

        let anna = labels[0];
        let max = labels[1];
        assert_ne!(anna, max, "the two voices were merged: {labels:?}");

        for (index, label) in labels.iter().enumerate() {
            let expected = if index % 2 == 0 { anna } else { max };
            assert_eq!(*label, expected, "line {index} went to the wrong speaker");
        }
    }

    #[test]
    fn speakers_are_numbered_by_who_spoke_first() {
        let utterances = vec![
            like(&MAX, 0.1, 1),
            like(&ANNA, 0.1, 2),
            like(&MAX, 0.1, 3),
            like(&ANNA, 0.1, 4),
        ];

        let labels = cluster(&utterances, &VoiceOptions::default());
        assert_eq!(labels[0], Some(0));
        assert_eq!(labels[1], Some(1));
        assert_eq!(labels[2], Some(0));
        assert_eq!(labels[3], Some(1));
    }

    #[test]
    fn a_two_word_interjection_never_founds_a_third_person() {
        let mut utterances: Vec<Utterance> = (0..6)
            .map(|seed| {
                if seed % 2 == 0 {
                    like(&ANNA, 0.15, seed)
                } else {
                    like(&MAX, 0.15, seed)
                }
            })
            .collect();

        utterances.push(Utterance {
            print: vec![0.5, 0.5, 0.5, 0.5, 0.5, 0.5],
            frames: 12,
        });

        let labels = cluster(&utterances, &VoiceOptions::default());
        let speakers: std::collections::HashSet<_> = labels.iter().flatten().collect();

        assert_eq!(
            speakers.len(),
            2,
            "an interjection invented a speaker: {labels:?}"
        );
        assert!(labels[6].is_some(), "and it still got attributed");
    }

    #[test]
    fn a_stretch_with_nothing_measurable_is_left_unattributed() {
        let utterances = vec![
            like(&ANNA, 0.1, 1),
            Utterance {
                print: Vec::new(),
                frames: 0,
            },
            like(&ANNA, 0.1, 3),
        ];

        let labels = cluster(&utterances, &VoiceOptions::default());
        assert_eq!(labels[1], None, "silence was given a speaker");
        assert_eq!(labels[0], Some(0));
        assert_eq!(labels[2], Some(0));
    }

    #[test]
    fn the_cap_holds_however_noisy_the_recording_is() {
        let utterances: Vec<Utterance> = (0..12)
            .map(|seed| {
                let print: Vec<f32> = (0..6)
                    .map(|index| if index == seed % 6 { 1.0 } else { 0.0 })
                    .collect();
                Utterance { print, frames: 200 }
            })
            .collect();

        let options = VoiceOptions {
            max_speakers: 3,
            ..VoiceOptions::default()
        };
        let labels = cluster(&utterances, &options);
        let speakers: std::collections::HashSet<_> = labels.iter().flatten().collect();

        assert!(speakers.len() <= 3, "got {} speakers", speakers.len());
    }

    #[test]
    fn nothing_to_go_on_means_nobody_is_named() {
        assert!(cluster(&[], &VoiceOptions::default()).is_empty());

        let quiet = vec![Utterance {
            print: Vec::new(),
            frames: 0,
        }];
        assert_eq!(cluster(&quiet, &VoiceOptions::default()), vec![None]);
    }

    #[test]
    fn distance_is_about_direction_and_not_loudness() {
        assert!(distance(&[1.0, 0.0], &[3.0, 0.0]).abs() < 1e-6);
        assert!((distance(&[1.0, 0.0], &[0.0, 1.0]) - 1.0).abs() < 1e-6);

        assert_eq!(distance(&[1.0], &[1.0, 2.0]), 1.0);
        assert_eq!(distance(&[], &[]), 1.0);
    }

    #[test]
    fn speakers_are_named_from_one() {
        assert_eq!(default_name(0), "Speaker 1");
        assert_eq!(default_name(3), "Speaker 4");
    }
}
