//! Pictures of the drums, for people who cannot read the names yet, children above all. Each is
//! drawn from simple shapes, so it is sharp at any size and takes its drum family's colour. There
//! are no image files.
//!
//! Related drums get related pictures that still differ: toms shrink as they get higher, a closed
//! hi-hat has its cymbals together and an open one has them apart, a crash cymbal is tilted and
//! a ride has a bell.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

/// How row names show in the grid. A device setting, not part of any project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum RowLabels {
    /// The name, with a stripe in the drum family's colour.
    #[default]
    Names,
    /// A picture with the name.
    PicturesAndNames,
    /// A larger picture, without the name.
    Pictures,
}

impl RowLabels {
    /// The next choice, for a menu item that cycles through them.
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Names => Self::PicturesAndNames,
            Self::PicturesAndNames => Self::Pictures,
            Self::Pictures => Self::Names,
        }
    }

    pub(crate) fn shows_pictures(self) -> bool {
        self != Self::Names
    }

    /// How the setting is written to the device.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Names => "names",
            Self::PicturesAndNames => "pictures_and_names",
            Self::Pictures => "pictures",
        }
    }

    pub(crate) fn from_str(text: &str) -> Self {
        match text.trim() {
            "pictures_and_names" => Self::PicturesAndNames,
            "pictures" => Self::Pictures,
            _ => Self::Names,
        }
    }
}

/// What a picture shows.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Kick,
    Snare,
    SideStick,
    Clap,
    HiHatClosed,
    HiHatOpen,
    HiHatPedal,
    /// A tom, `size` 1.0 for the lowest down to about 0.6 for the highest.
    Tom {
        size: f32,
        floor: bool,
    },
    Crash,
    Ride,
    Tambourine,
    Cowbell,
    Maracas,
    Bongos,
    Conga,
    Timbale,
    Whistle,
    Guiro,
    Claves,
    WoodBlock,
    Triangle,
    /// Anything else: a plain drum.
    Drum,
}

fn kind(key: u8) -> Kind {
    match key {
        35 | 36 => Kind::Kick,
        38 | 40 => Kind::Snare,
        37 => Kind::SideStick,
        39 => Kind::Clap,
        42 => Kind::HiHatClosed,
        46 => Kind::HiHatOpen,
        44 => Kind::HiHatPedal,
        41 => Kind::Tom {
            size: 1.0,
            floor: true,
        },
        43 => Kind::Tom {
            size: 0.92,
            floor: true,
        },
        45 => Kind::Tom {
            size: 0.85,
            floor: false,
        },
        47 => Kind::Tom {
            size: 0.75,
            floor: false,
        },
        48 => Kind::Tom {
            size: 0.68,
            floor: false,
        },
        50 => Kind::Tom {
            size: 0.6,
            floor: false,
        },
        49 | 52 | 55 | 57 => Kind::Crash,
        51 | 53 | 59 => Kind::Ride,
        54 => Kind::Tambourine,
        56 | 67 | 68 => Kind::Cowbell,
        58 | 69 | 70 => Kind::Maracas,
        60 | 61 => Kind::Bongos,
        62..=64 | 78 | 79 => Kind::Conga,
        65 | 66 => Kind::Timbale,
        71 | 72 => Kind::Whistle,
        73 | 74 => Kind::Guiro,
        75 => Kind::Claves,
        76 | 77 => Kind::WoodBlock,
        80 | 81 => Kind::Triangle,
        _ => Kind::Drum,
    }
}

/// Draws the picture of drum `key` in the largest square that fits `rect`, filled with
/// `colour` and outlined with `ink`.
pub(crate) fn paint(painter: &Painter, rect: Rect, key: u8, colour: Color32, ink: Color32) {
    let side = rect.width().min(rect.height());
    let square = Rect::from_center_size(rect.center(), Vec2::splat(side));
    let pen = Pen {
        painter,
        square,
        colour,
        ink: Stroke::new((side / 22.0).max(1.0), ink),
        dark: colour.gamma_multiply(0.55),
    };
    pen.draw(kind(key));
}

/// Draws in a unit square: (0, 0) is the top left and (1, 1) the bottom right.
struct Pen<'a> {
    painter: &'a Painter,
    square: Rect,
    colour: Color32,
    ink: Stroke,
    /// A darker shade of the colour, for sides and shadows.
    dark: Color32,
}

impl Pen<'_> {
    fn p(&self, x: f32, y: f32) -> Pos2 {
        self.square.min + self.square.size() * Vec2::new(x, y)
    }

    fn len(&self, d: f32) -> f32 {
        self.square.width() * d
    }

    fn ellipse(&self, x: f32, y: f32, rx: f32, ry: f32, fill: Color32) {
        let (c, r) = (self.p(x, y), Vec2::new(self.len(rx), self.len(ry)));
        self.painter.add(Shape::ellipse_filled(c, r, fill));
        self.painter.add(Shape::ellipse_stroke(c, r, self.ink));
    }

    fn circle(&self, x: f32, y: f32, r: f32, fill: Color32) {
        self.ellipse(x, y, r, r, fill);
    }

    fn line(&self, points: &[(f32, f32)]) {
        let points = points.iter().map(|&(x, y)| self.p(x, y)).collect();
        self.painter.add(Shape::line(points, self.ink));
    }

    /// A thick line in the picture's colour, for things made of rods: claves, a triangle.
    fn rod(&self, points: &[(f32, f32)]) {
        let points: Vec<Pos2> = points.iter().map(|&(x, y)| self.p(x, y)).collect();
        let width = self.len(0.09);
        self.painter.add(Shape::line(
            points.clone(),
            Stroke::new(width + 2.0 * self.ink.width, self.ink.color),
        ));
        self.painter
            .add(Shape::line(points, Stroke::new(width, self.colour)));
    }

    fn polygon(&self, points: &[(f32, f32)], fill: Color32) {
        let points = points.iter().map(|&(x, y)| self.p(x, y)).collect();
        self.painter
            .add(Shape::convex_polygon(points, fill, self.ink));
    }

    /// A drum seen from the front and a little above: a body with an oval head on top.
    /// `x` is the centre, `top` and `bottom` the head's and the body's lowest edge, `rx` the
    /// half width and `ry` the head's half height.
    fn drum(&self, x: f32, top: f32, bottom: f32, rx: f32, ry: f32) {
        self.ellipse(x, bottom, rx, ry, self.dark);
        self.polygon(
            &[
                (x - rx, top),
                (x + rx, top),
                (x + rx, bottom),
                (x - rx, bottom),
            ],
            self.dark,
        );
        // The body's sides, drawn over the polygon's top edge.
        self.line(&[(x - rx, top), (x - rx, bottom)]);
        self.line(&[(x + rx, top), (x + rx, bottom)]);
        self.ellipse(x, top, rx, ry, self.colour);
    }

    /// A cymbal: a thin oval, tilted by `tilt` (rise from left to right, in unit lengths).
    fn cymbal(&self, x: f32, y: f32, rx: f32, tilt: f32) {
        let points: Vec<(f32, f32)> = (0..24)
            .map(|i| {
                let a = i as f32 / 24.0 * std::f32::consts::TAU;
                let (dx, dy) = (rx * a.cos(), 0.07 * a.sin());
                (x + dx, y + dy - tilt * dx / rx)
            })
            .collect();
        self.polygon(&points, self.colour);
    }

    fn draw(&self, kind: Kind) {
        match kind {
            Kind::Kick => {
                // The front head of a bass drum, with its logo ring and two short feet.
                self.line(&[(0.22, 0.86), (0.12, 0.97)]);
                self.line(&[(0.78, 0.86), (0.88, 0.97)]);
                self.circle(0.5, 0.52, 0.42, self.colour);
                self.circle(0.5, 0.52, 0.2, self.dark);
            }
            Kind::Snare => {
                self.drum(0.5, 0.42, 0.72, 0.4, 0.13);
                // Tension rods round the side.
                for x in [0.24, 0.42, 0.58, 0.76] {
                    self.line(&[(x, 0.5), (x, 0.74)]);
                }
            }
            Kind::SideStick => {
                self.drum(0.5, 0.5, 0.76, 0.4, 0.13);
                // A stick lying across the rim.
                self.line(&[(0.06, 0.28), (0.9, 0.56)]);
                self.circle(0.06, 0.28, 0.05, self.dark);
            }
            Kind::Clap => {
                // Two hands meeting, and lines for the sound.
                self.polygon(
                    &[
                        (0.18, 0.88),
                        (0.3, 0.3),
                        (0.42, 0.22),
                        (0.5, 0.3),
                        (0.46, 0.92),
                    ],
                    self.colour,
                );
                self.polygon(
                    &[
                        (0.82, 0.88),
                        (0.7, 0.3),
                        (0.58, 0.22),
                        (0.5, 0.3),
                        (0.54, 0.92),
                    ],
                    self.dark,
                );
                self.line(&[(0.5, 0.04), (0.5, 0.14)]);
                self.line(&[(0.26, 0.08), (0.32, 0.16)]);
                self.line(&[(0.74, 0.08), (0.68, 0.16)]);
            }
            Kind::HiHatClosed => {
                self.line(&[(0.5, 0.3), (0.5, 0.96)]);
                self.line(&[(0.3, 0.96), (0.7, 0.96)]);
                self.cymbal(0.5, 0.28, 0.44, 0.0);
                self.cymbal(0.5, 0.36, 0.44, 0.0);
            }
            Kind::HiHatOpen => {
                self.line(&[(0.5, 0.18), (0.5, 0.96)]);
                self.line(&[(0.3, 0.96), (0.7, 0.96)]);
                self.cymbal(0.5, 0.16, 0.44, 0.0);
                self.cymbal(0.5, 0.46, 0.44, 0.0);
            }
            Kind::HiHatPedal => {
                self.line(&[(0.5, 0.2), (0.5, 0.8)]);
                self.cymbal(0.5, 0.18, 0.44, 0.0);
                self.cymbal(0.5, 0.26, 0.44, 0.0);
                // The foot pedal.
                self.polygon(
                    &[(0.36, 0.8), (0.86, 0.72), (0.9, 0.78), (0.4, 0.92)],
                    self.dark,
                );
            }
            Kind::Tom { size, floor } => {
                let rx = 0.42 * size;
                let height = 0.36 * size;
                let bottom = if floor { 0.78 } else { 0.86 };
                if floor {
                    self.line(&[(0.5 - rx, 0.6), (0.5 - rx - 0.06, 0.97)]);
                    self.line(&[(0.5 + rx, 0.6), (0.5 + rx + 0.06, 0.97)]);
                }
                self.drum(0.5, bottom - height, bottom, rx, 0.14 * size);
            }
            Kind::Crash => {
                self.line(&[(0.5, 0.4), (0.5, 0.96)]);
                self.line(&[(0.3, 0.96), (0.7, 0.96)]);
                self.cymbal(0.5, 0.38, 0.46, 0.14);
            }
            Kind::Ride => {
                self.line(&[(0.5, 0.42), (0.5, 0.96)]);
                self.line(&[(0.3, 0.96), (0.7, 0.96)]);
                self.cymbal(0.5, 0.42, 0.48, 0.0);
                // The bell in the middle.
                self.ellipse(0.5, 0.36, 0.12, 0.08, self.dark);
            }
            Kind::Tambourine => {
                self.circle(0.5, 0.5, 0.38, self.colour);
                self.circle(0.5, 0.5, 0.26, self.dark);
                for i in 0..6 {
                    let a = i as f32 / 6.0 * std::f32::consts::TAU;
                    self.circle(
                        0.5 + 0.38 * a.cos(),
                        0.5 + 0.38 * a.sin(),
                        0.07,
                        self.colour,
                    );
                }
            }
            Kind::Cowbell => {
                self.polygon(
                    &[(0.36, 0.2), (0.64, 0.2), (0.82, 0.84), (0.18, 0.84)],
                    self.colour,
                );
                self.ellipse(0.5, 0.84, 0.32, 0.06, self.dark);
                // The handle on top.
                self.line(&[(0.44, 0.2), (0.44, 0.08), (0.56, 0.08), (0.56, 0.2)]);
            }
            Kind::Maracas => {
                self.line(&[(0.3, 0.56), (0.18, 0.96)]);
                self.line(&[(0.7, 0.56), (0.82, 0.96)]);
                self.ellipse(0.32, 0.34, 0.16, 0.24, self.colour);
                self.ellipse(0.68, 0.34, 0.16, 0.24, self.dark);
            }
            Kind::Bongos => {
                self.drum(0.28, 0.4, 0.8, 0.22, 0.09);
                self.drum(0.72, 0.48, 0.8, 0.18, 0.08);
            }
            Kind::Conga => {
                // A tall drum, wider in the middle.
                self.polygon(
                    &[
                        (0.32, 0.16),
                        (0.68, 0.16),
                        (0.76, 0.5),
                        (0.66, 0.94),
                        (0.34, 0.94),
                        (0.24, 0.5),
                    ],
                    self.dark,
                );
                self.ellipse(0.5, 0.16, 0.18, 0.07, self.colour);
            }
            Kind::Timbale => {
                self.drum(0.5, 0.48, 0.66, 0.44, 0.12);
                self.line(&[(0.5, 0.72), (0.5, 0.96)]);
            }
            Kind::Whistle => {
                self.polygon(
                    &[(0.08, 0.38), (0.6, 0.38), (0.6, 0.56), (0.08, 0.56)],
                    self.colour,
                );
                self.circle(0.66, 0.6, 0.22, self.colour);
                self.circle(0.66, 0.6, 0.08, self.dark);
            }
            Kind::Guiro => {
                self.ellipse(0.5, 0.5, 0.44, 0.24, self.colour);
                for x in [0.28, 0.38, 0.48, 0.58, 0.68] {
                    self.line(&[(x, 0.34), (x, 0.66)]);
                }
            }
            Kind::Claves => {
                self.rod(&[(0.14, 0.22), (0.86, 0.78)]);
                self.rod(&[(0.14, 0.78), (0.86, 0.22)]);
            }
            Kind::WoodBlock => {
                self.polygon(
                    &[(0.1, 0.32), (0.9, 0.32), (0.9, 0.72), (0.1, 0.72)],
                    self.colour,
                );
                self.polygon(
                    &[(0.2, 0.46), (0.8, 0.46), (0.8, 0.54), (0.2, 0.54)],
                    self.dark,
                );
            }
            Kind::Triangle => {
                self.rod(&[(0.5, 0.12), (0.88, 0.84), (0.12, 0.84), (0.42, 0.27)]);
                // The beater.
                self.line(&[(0.62, 0.6), (0.96, 0.4)]);
            }
            Kind::Drum => self.drum(0.5, 0.4, 0.8, 0.4, 0.13),
        }
    }
}
