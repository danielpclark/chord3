extern crate pdf;
extern crate regex;

use pdf::{Canvas, Pdf, FontSource};
use regex::Regex;
use std::fs::File;
use std::io::BufRead;
use std::io;
use std::vec::Vec;
use std::collections::{BTreeSet, BTreeMap};
use std::env;
use std::sync::Mutex;

mod chords;
use ::chords::get_known_chords;

fn chordbox<'a>(c: &mut Canvas<'a, File>, left: f32, top: f32,
                name: &str, strings: &Vec<i8>)
                -> io::Result<()> {
    let dx = 5.0;
    let dy = 7.0;
    let right = left + 5.0 * dx;
    let bottom = top - 4.4 * dy;
    try!(c.center_text(left + 2.0 * dx, top + dy,
                       FontSource::Helvetica_Oblique, 12.0, name));
    let barre = strings[0];
    let up =
        if barre < 2 {
            try!(c.set_line_width(1.0));
            try!(c.line(left-0.15, top+0.5, right+0.15, top+0.5));
            try!(c.stroke());
            0.0
        } else {
            try!(c.right_text(left - 0.4 * dx, top - 0.9 * dy,
                              FontSource::Helvetica, dy, &format!("{}", barre)));
            1.6
        };
    try!(c.set_line_width(0.3));
    for b in 0..5 {
        let y = top - b as f32 * dy;
        try!(c.line(left, y, right, y));
    }
    for s in 0..6 {
        let x = left + s as f32 * dx;
        try!(c.line(x, top+up, x, bottom));
    }
    try!(c.stroke());
    let radius = 1.4;
    let above = top + 2.0 + radius;
    for s in 0..6 {
        let x = left + s as f32 * dx;
        match strings[s+1] {
            -2 => (), // No-op for unknown chord
            -1 => {
                let (l, r) = (x-radius, x+radius);
                let (t, b) = (above-radius, above+radius);
                try!(c.line(l, t, r, b));
                try!(c.line(r, t, l, b));
                try!(c.stroke());
            }
            0 => {
                try!(c.circle(x, above, radius));
                try!(c.stroke());
            }
            y => {
                let y = top - (y as f32 - 0.5) * dy;
                try!(c.circle(x, y, radius+0.4));
                try!(c.fill());
            }
        }
    }
    Ok(())
}

enum ChordFileExpression {
    Title{s: String},
    SubTitle{s: String},
    Comment{s: String},
    ChordDef{name: String, def: Vec<i8>},
    Line{s: Vec<String>}
}

struct ChoproParser<R: io::Read> {
    source: Mutex<io::Lines<io::BufReader<R>>>
}

impl ChoproParser<File> {
    fn open(path: &str) -> io::Result<ChoproParser<File>> {
        let f = try!(File::open(path));
        Ok(ChoproParser::new(f))
    }
}
impl<R: io::Read> ChoproParser<R> {
    fn new(source: R) -> ChoproParser<R> {
        let reader = io::BufReader::new(source);
        ChoproParser {
            source: Mutex::new(reader.lines())
        }
    }

    // Internal: Return the next line that is not a comment
    fn nextline(&mut self) -> Option<String> {
        loop {
            match self.source.lock().unwrap().next() {
                Some(Ok(line)) => {
                    let comment_re = Regex::new(r"^\s*#").unwrap();
                    if !comment_re.is_match(&line) {
                        return Some(line)
                    }
                },
                Some(Err(e)) => {
                    println!("Failed to read source: {}", e);
                    return None
                },
                _ => {
                    return None
                }
            }
        }
    }
}

impl<R: io::Read> Iterator for ChoproParser<R> {
    type Item = ChordFileExpression;

    fn next(&mut self) -> Option<ChordFileExpression> {
        if let Some(line) = self.nextline() {
            let re = Regex::new(r"\{(?P<cmd>\w+)(?::?\s*(?P<arg>.*))?\}").unwrap();
            if let Some(caps) = re.captures(&line) {
                let arg = caps.name("arg").unwrap_or("").to_string();
                match caps.name("cmd").unwrap() {
                    "t" | "title" => Some(ChordFileExpression::Title{s: arg}),
                    "st" | "subtitle" => Some(ChordFileExpression::SubTitle{s:arg}),
                    "c" => Some(ChordFileExpression::Comment{s:arg}),
                    "define" => {
                        //println!("Parse chord def '{}'", arg);
                        let re = Regex::new(r"(?i)^([\S]+)\s+base-fret\s+([x0-5])\s+frets(?:\s+([x0-5]))(?:\s+([x0-5]))(?:\s+([x0-5]))(?:\s+([x0-5]))(?:\s+([x0-5]))(?:\s+([x0-5]))\s*$").unwrap();
                        if let Some(caps) = re.captures(&arg) {
                            let s = |n| {
                                //println!("String {} is {:?}", n,
                                //         caps.at(n as usize+2));
                                match caps.at(n as usize+2) {
                                    Some("x") | Some("X") | None => -1,
                                    Some(s) => s.parse::<i8>().unwrap(),
                                }
                            };
                            Some(ChordFileExpression::ChordDef {
                                name: caps.at(1).unwrap().to_string(),
                                def: vec!(s(0),
                                          s(1), s(2), s(3),
                                          s(4), s(5), s(6))
                            })
                        } else {
                            let whole = caps.at(0).unwrap();
                            println!("Warning: Bad chord definition {}", whole);
                            Some(ChordFileExpression::Comment{s:whole.to_string()})
                        }
                    },
                    x => {
                        println!("unknown expression {}", x);
                        Some(ChordFileExpression::Comment{s:caps.at(0).unwrap().to_string()})
                    }
                }
            } else {
                let mut s = vec!();
                let re = Regex::new(r"([^\[]*)(?:\[([^\]]*)\])?").unwrap();
                for caps in re.captures_iter(&line) {
                    s.push(caps.at(1).unwrap().to_string());
                    if let Some(chord) = caps.at(2) {
                        s.push(chord.to_string());
                    }
                }
                Some(ChordFileExpression::Line{s: s})
            }
        } else {
            None
        }
    }
}


fn main() {
    let mut file = File::create("foo.pdf").unwrap();
    let mut document = Pdf::new(&mut file).unwrap();
    document.set_title("Songbook");
    document.set_producer(concat!("chord3 version ",
                                  env!("CARGO_PKG_VERSION"),
                                  "\nhttps://github.com/kaj/chord3"));
    let args = env::args();
    let args = args.skip(1);
    if args.len() > 0 {
        for name in args {
            if let Err(e) = render_song(&mut document, name.clone()) {
                println!("Failed to handle {}: {}", name, e);
            }
        }
    } else {
        println!("Usage: {} [chordfile]...", env::args().nth(0).unwrap());
    }
    document.finish().unwrap();
}

fn render_song<'a>(document: &mut Pdf<'a, File>, songfilename: String)
                   -> io::Result<()> {
    let source = try!(ChoproParser::open(&songfilename));
    let (width, height) = (596.0, 842.0);
    let known_chords = get_known_chords();
    let mut local_chords : BTreeMap<String, Vec<i8>> = BTreeMap::new();
    document.render_page(width, height, |c| {
        let mut y = height - 30.0;
        let left = 50.0;
        let times_bold = c.get_font(FontSource::Times_Bold);
        let times_italic = c.get_font(FontSource::Times_Italic);
        let times = c.get_font(FontSource::Times_Roman);
        let chordfont = c.get_font(FontSource::Helvetica_Oblique);
        let mut used_chords : BTreeSet<String> = BTreeSet::new();
        for token in source {
            //let token = ChordFileExpression::parse(&line.unwrap()).unwrap();
            try!(match token {
                ChordFileExpression::Title{s} => c.text(|t| {
                    y = y - 20.0;
                    try!(t.set_font(&times_bold, 18.0));
                    try!(t.pos(left, y));
                    t.show(&s)
                }),
                ChordFileExpression::SubTitle{s} => c.text(|t| {
                    y = y - 18.0;
                    try!(t.set_font(&times_italic, 16.0));
                    try!(t.pos(left, y));
                    t.show(&s)
                }),
                ChordFileExpression::Comment{s} => c.text(|t| {
                    y = y - 14.0;
                    try!(t.set_font(&times_italic, 14.0));
                    try!(t.pos(left, y));
                    t.show(&s)
                }),
                ChordFileExpression::ChordDef{name, def} => {
                    local_chords.insert(name, def);
                    Ok(())
                },
                ChordFileExpression::Line{s} => c.text(|t| {
                    let text_size = 14.0;
                    let chord_size = 10.0;
                    y = y - 1.2 * ( if s.len() > 1 {text_size + chord_size}
                                    else { text_size } );
                    try!(t.set_font(&times, text_size));
                    try!(t.pos(left, y));
                    let mut last_chord_width = 0.0;
                    for (i, part) in s.iter().enumerate() {
                        if i % 2 == 1 {
                            used_chords.insert(part.to_string());
                            try!(t.gsave());
                            try!(t.set_rise(text_size));
                            try!(t.set_font(&chordfont, chord_size));
                            let chord_width =
                                chordfont.get_width_raw(&part) as i32;
                            try!(t.show_j(&part, chord_width));
                            last_chord_width =
                                (chord_width + 400) as f32 * chord_size / 1000.0;
                            try!(t.grestore());
                        } else {
                            let part = { if part.len() > 0 { part.to_string() }
                                         else { " ".to_string() } };
                            let text_width = times.get_width(text_size, &part);
                            if last_chord_width > text_width && i+1 < s.len() {
                                let extra = last_chord_width - text_width;
                                let n_space = part.chars()
                                    .filter(|&c| {c == ' '})
                                    .count();
                                if n_space > 0 {
                                    try!(t.set_word_spacing(
                                        extra / n_space as f32));
                                } else {
                                    try!(t.set_char_spacing(
                                        extra / part.len() as f32));
                                }
                            }
                            try!(t.show(&part));
                            if last_chord_width > text_width {
                                try!(t.set_char_spacing(0.0));
                                try!(t.set_word_spacing(0.0));
                            }
                        }
                    }
                    Ok(())
                })
            })
        }
        // Remove non-chords that are displayed like chords above the text.
        used_chords.remove("%");
        used_chords.remove("");
        let mut x = width - used_chords.len() as f32 * 40.0;
        for chord in used_chords.iter() {
            if let Some(chorddef) = local_chords.get(chord) {
                try!(chordbox(c, x, 80.0, chord, chorddef));
            } else if let Some(chorddef) = known_chords.get(chord) {
                try!(chordbox(c, x, 80.0, chord, chorddef));
            } else {
                println!("Warning: Unknown chord '{}'.", chord);
                try!(chordbox(c, x, 80.0, chord, &vec!(0,-2,-2,-2,-2,-2,-2,-2)));
            }
            x = x + 40.0;
        }
        Ok(())
    })
}
