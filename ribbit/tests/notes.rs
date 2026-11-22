use ribbit::notes::{LENGTH, parse};

// The pitch of each note in a tune, with None for a rest.
fn pitches(tune: &str) -> Vec<Option<u32>> {
    parse(tune).unwrap().iter().map(|note| note.hz).collect()
}

#[test]
fn pitches_and_octaves() {
    // A4 is 440 Hz, and each octave doubles it.
    assert_eq!(pitches("A4 A5 A2"), [Some(440), Some(880), Some(110)]);
    // The C major chord, rounded to whole hertz: C4 is 261.63.
    assert_eq!(pitches("C4 E4 G4 C5"), [Some(262), Some(330), Some(392), Some(523)]);
    // The ends of the range.
    assert_eq!(pitches("C1 B8"), [Some(33), Some(7902)]);
}

#[test]
fn sharps_and_flats() {
    assert_eq!(pitches("C#5 Bb3 F#3"), [Some(554), Some(233), Some(185)]);
    // A sharp or flat can cross into the next octave: Cb4 is B3, B#3 is C4.
    assert_eq!(pitches("Cb4 B#3"), [Some(247), Some(262)]);
    // Lower case works too, and R is a rest.
    assert_eq!(pitches("a4 bb4 R r"), [Some(440), Some(466), None, None]);
}

#[test]
fn lengths() {
    let tune = parse("C4 E4:500 R:100 G4:0 A4:10000").unwrap();
    let lengths: Vec<u64> = tune.iter().map(|note| note.ms).collect();
    assert_eq!(lengths, [LENGTH, 500, 100, 0, 10000]);
    assert_eq!(LENGTH, 125);
}

#[test]
fn mistakes() {
    // The first word that isn't a note comes back.
    for bad in ["H4", "C9", "C0", "C", "C4:", "C4:10001", "C4:-1", "C##4", "Cx4", "R:x", ":500", "4C"] {
        let tune = format!("A4 {} B4", bad);
        assert_eq!(parse(&tune).err(), Some(bad));
    }
    // Nothing at all is an empty tune.
    assert!(parse("").unwrap().is_empty());
    assert!(parse("   ").unwrap().is_empty());
}
