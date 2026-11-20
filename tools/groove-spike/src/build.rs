//! Turns an analysed file into a TickTapDrum project, through the project's public API only.

use std::collections::BTreeSet;

use dm_project::{
    DrumFamily, MAX_BARS, MAX_PATTERNS, MAX_SONG, Project, STEPS_PER_BAR, drum_family, drum_name,
};

use crate::analysis::{Analysis, Hit};

/// Bars per pattern when a file is too long for one pattern.
const CHUNK_BARS: usize = 4;

pub struct Built {
    pub project: Project,
    pub rows: usize,
    pub patterns: usize,
    pub bars_per_pattern: usize,
    pub song: usize,
    /// Chunks that needed a pattern beyond Z, so their song entries were left out.
    pub chunks_without_pattern: usize,
    /// Song entries beyond the song's limit.
    pub song_cut: usize,
    pub tempo_clamped: Option<f32>,
}

fn family_order(key: u8) -> u8 {
    match drum_family(key) {
        DrumFamily::Kick => 0,
        DrumFamily::Snare => 1,
        DrumFamily::HiHat => 2,
        DrumFamily::Tom => 3,
        DrumFamily::Cymbal => 4,
        DrumFamily::Percussion => 5,
    }
}

pub fn project(a: &Analysis) -> Built {
    let mut project = Project::new();
    let mut keys: Vec<u8> = a
        .grid
        .iter()
        .flatten()
        .map(|h| h.0)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    keys.sort_by_key(|&k| (family_order(k), k));
    if keys.is_empty() {
        keys.push(36);
    }
    keys.truncate(dm_project::MAX_ROWS);
    for (row, &key) in keys.iter().enumerate() {
        if row < project.rows().len() {
            project.set_row_key(row, key).unwrap();
            project.rename_row(row, &drum_name(key)).unwrap();
        } else {
            project.add_row(key).unwrap();
        }
    }
    while project.rows().len() > keys.len() {
        project.remove_row(project.rows().len() - 1).unwrap();
    }
    let row_of = |key: u8| keys.iter().position(|&k| k == key);

    let wanted = a.bpm as f32;
    let tempo = wanted.clamp(dm_audio::MIN_BPM, dm_audio::MAX_BPM);
    project.tempo = tempo;
    project.set_swing(a.swing as f32);

    let chunk = if a.bars <= MAX_BARS {
        a.bars
    } else {
        CHUNK_BARS
    };
    let chunks: Vec<&[Vec<Hit>]> = a.grid.chunks(chunk).collect();
    let mut distinct: Vec<&[Vec<Hit>]> = Vec::new();
    let mut order = Vec::new();
    let mut chunks_without_pattern = 0;
    for c in &chunks {
        let index = match distinct.iter().position(|d| d == c) {
            Some(i) => Some(i),
            None if distinct.len() < MAX_PATTERNS => {
                distinct.push(c);
                Some(distinct.len() - 1)
            }
            None => None,
        };
        match index {
            Some(i) => order.push(i),
            None => chunks_without_pattern += 1,
        }
    }

    for (i, bars) in distinct.iter().enumerate() {
        if i > 0 {
            project.add_pattern().unwrap();
        }
        for _ in 1..bars.len() {
            project.add_bar(i).unwrap();
        }
        for (bar, hits) in bars.iter().enumerate() {
            for &(key, step, velocity) in hits {
                if let Some(row) = row_of(key) {
                    project
                        .set_step(i, row, bar * STEPS_PER_BAR + step, velocity)
                        .unwrap();
                }
            }
        }
    }

    let song_cut = order.len().saturating_sub(MAX_SONG);
    order.truncate(MAX_SONG);
    if order != [0] {
        for &i in &order {
            project.song_append(i).unwrap();
        }
        project.song_remove(0).unwrap();
    }

    Built {
        rows: project.rows().len(),
        patterns: distinct.len(),
        bars_per_pattern: chunk,
        song: project.song().len(),
        project,
        chunks_without_pattern,
        song_cut,
        tempo_clamped: (tempo != wanted).then_some(wanted),
    }
}

impl Built {
    pub fn report(&self) -> String {
        let mut s = format!(
            "  project: {} rows, {} patterns of up to {} bars, song of {} entries, tempo {} BPM, swing {:.2}",
            self.rows,
            self.patterns,
            self.bars_per_pattern,
            self.song,
            self.project.tempo,
            self.project.swing()
        );
        if self.chunks_without_pattern > 0 {
            s += &format!(
                "\n  {} chunks found no free pattern and were left out of the song",
                self.chunks_without_pattern
            );
        }
        if self.song_cut > 0 {
            s += &format!("\n  {} song entries cut at the limit", self.song_cut);
        }
        if let Some(t) = self.tempo_clamped {
            s += &format!("\n  tempo {t} BPM was outside the app's range");
        }
        s
    }
}
