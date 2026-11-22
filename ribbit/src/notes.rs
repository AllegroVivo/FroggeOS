// The ability to write simple music using the integrated speaker.

use alloc::vec::Vec;

pub struct Note {
    pub hz: Option<u32>,    // None for rests
    pub ms: u64,
}

// The default length for a Note
pub const LENGTH: u64 = 125; // 125ms == 16th Note

// The twelve primary notes C4 -> B4.
const OCTAVE_4: [u32; 12] = [
    26163, 27718, 29366, 31113, 32963, 34923,
    36999, 39200, 41530, 44000, 46616, 49388,
];

// Reads and returns a whole block of music. Will early-return the first non-music token.
pub fn parse(tune: &str) -> Result<Vec<Note>, &str> {
    let mut notes = Vec::new();
    for word in tune.split_whitespace() {
        notes.push(note(word).ok_or(word)?);
    }
    Ok(notes)
}

fn note(word: &str) -> Option<Note> {
    let (name, ms) = match word.split_once(':') {
        Some((name, ms)) => (name, ms.parse().ok().filter(|&ms| ms <= 10_000)?),
        None => (word, LENGTH),
    };
    if name == "R" || name == "r" {
        return Some(Note{ hz: None, ms });
    }
    let (letter, rest) = name.split_at_checked(1)?;
    let semitone = match letter {
        "C" | "c" => 0,
        "D" | "d" => 2,
        "E" | "e" => 4,
        "F" | "f" => 5,
        "G" | "g" => 7,
        "A" | "a" => 9,
        "B" | "b" => 11,
        _ => return None,
    };
    let (shift, octave) = match rest.as_bytes() {
        [b'#', octave] => (1, *octave),
        [b'b', octave] => (-1, *octave),
        [octave] => (0, *octave),
        _ => return None,
    };
    if !(b'1'..=b'8').contains(&octave) {
        return None;
    }
    let n = (octave - b'0') as i32 * 12 + semitone + shift;
    let (octave, semitone) = (n / 12, (n % 12) as usize);
    let centi = if octave >= 4 {
        OCTAVE_4[semitone] << (octave - 4)
    } else {
        OCTAVE_4[semitone] >> (4 - octave)
    };
    Some(Note { hz: Some((centi + 50) / 100), ms })
}
