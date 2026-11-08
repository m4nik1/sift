//! Sift's palette. Cool slate surfaces so the data carries the colour: every value is inked by
//! its SQLite storage class (integers blue, reals violet, blobs amber, NULL faint), which is the
//! one thing in the window allowed to be colourful.

use rune::Color;
use rune_kit::{Kit, PaletteSeed};

use crate::db::Class;

fn c(s: &str) -> Color {
    rune::color::hex(s).expect("valid hex")
}

#[derive(Clone)]
pub struct Ink {
    pub integer: Color,
    pub real: Color,
    pub blob: Color,
    pub null: Color,
    pub text: Color,
    /// The density bars beside each table in the sidebar.
    pub bar: Color,
    pub bar_track: Color,
    pub drop: Color,
}

pub fn kit(dark: bool) -> Kit {
    let seed = if dark {
        PaletteSeed { bg: c("#191E25"), fg: c("#E3E8EE"), accent: c("#6EA8FE") }
    } else {
        PaletteSeed { bg: c("#FBFCFD"), fg: c("#17202B"), accent: c("#0F5FA6") }
    };
    let mut k = Kit::from_palette(seed);
    k.c.page = if dark { c("#12161C") } else { c("#E7EBF0") };
    k.radius.md = 8.0;
    k
}

pub fn ink(dark: bool, k: &Kit) -> Ink {
    if dark {
        Ink {
            integer: c("#86ADFF"),
            real: c("#BEA2FF"),
            blob: c("#E7A962"),
            null: k.c.text_faint,
            text: k.c.text_base,
            bar: c("#6EA8FE"),
            bar_track: k.alpha(k.c.text_base, 0.08),
            drop: k.alpha(c("#6EA8FE"), 0.14),
        }
    } else {
        Ink {
            integer: c("#2152BD"),
            real: c("#6E3FC6"),
            blob: c("#9A5200"),
            null: k.c.text_faint,
            text: k.c.text_base,
            bar: c("#0F5FA6"),
            bar_track: k.alpha(k.c.text_base, 0.08),
            drop: k.alpha(c("#0F5FA6"), 0.10),
        }
    }
}

impl Ink {
    pub fn class(&self, class: Class) -> Color {
        match class {
            Class::Null => self.null,
            Class::Integer => self.integer,
            Class::Real => self.real,
            Class::Text => self.text,
            Class::Blob => self.blob,
        }
    }
}
