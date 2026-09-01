//! What the IDE draws. Plain data, filled by the host each frame.
//!
//! Deliberately free of engine types: the panels render a snapshot, so the
//! IDE cannot accidentally become a second place where world state lives.

/// What the IDE is doing. It opens in `Edit`: an engine editor shows you a
/// scene to work on, it does not start playing at you. `r` toggles.
///
/// In `Edit` the world exists — control's `start` has reconciled the roster —
/// but no tick runs, so nothing moves and the Inspector shows a still frame
/// you can actually read. In `Play` the clock runs and the keyboard belongs to
/// the game.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mode {
    #[default]
    Edit,
    Play,
}

impl Mode {
    pub fn is_play(self) -> bool {
        matches!(self, Mode::Play)
    }
    pub fn label(self) -> &'static str {
        match self {
            Mode::Edit => "■ EDIT",
            Mode::Play => "▶ PLAY",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Panel {
    Hierarchy,
    /// Where focus starts: the picture is the point.
    #[default]
    Viewport,
    Inspector,
    Project,
    Terminal,
}

pub const PANEL_ORDER: [Panel; 5] = [
    Panel::Hierarchy,
    Panel::Viewport,
    Panel::Inspector,
    Panel::Project,
    Panel::Terminal,
];

impl Panel {
    pub fn title(self) -> &'static str {
        match self {
            Panel::Hierarchy => "Hierarchy",
            Panel::Viewport => "Viewport",
            Panel::Inspector => "Inspector",
            Panel::Project => "Project",
            Panel::Terminal => "Terminal",
        }
    }
    pub fn next(self) -> Panel {
        let i = PANEL_ORDER.iter().position(|&p| p == self).unwrap_or(0);
        PANEL_ORDER[(i + 1) % PANEL_ORDER.len()]
    }
}

/// One entity, as the Hierarchy lists it.
pub struct Node {
    pub label: String,
    /// Component names it carries — what the Inspector expands.
    pub components: Vec<String>,
}

pub struct Ide {
    /// The deck. `n` moves through it; this is GameTok's swipe, on a keyboard.
    pub games: Vec<String>,
    pub current: usize,

    pub adapter: String,
    pub fps: f32,
    pub frame: u64,

    pub hierarchy: Vec<Node>,
    pub selected: usize,
    /// `(field, value)` rows for the selected entity.
    pub inspector: Vec<(String, String)>,
    /// The bundle's module tree, as the layout rule lays it out.
    pub project: Vec<String>,
    pub log: Vec<String>,

    pub focused: Panel,
    pub mode: Mode,
    /// How the viewport spends a character cell. `m` cycles it — but only in
    /// the editor: once a game is running, `m` is whatever that game says it
    /// is, and game1 says it changes the model.
    pub art: crate::textart::TextArtMode,
}

impl Default for Ide {
    fn default() -> Ide {
        Ide {
            games: Vec::new(),
            current: 0,
            adapter: String::new(),
            fps: 0.0,
            frame: 0,
            hierarchy: Vec::new(),
            selected: 0,
            inspector: Vec::new(),
            project: Vec::new(),
            log: Vec::new(),
            focused: Panel::default(),
            mode: Mode::default(),
            art: crate::textart::TextArtMode::Mixed,
        }
    }
}

impl Ide {
    /// Cycle the viewport glyph set, the way `m` did in the original.
    pub fn cycle_art(&mut self) -> &'static str {
        use crate::textart::TextArtMode::*;
        self.art = match self.art {
            Mixed => Quadrant,
            Quadrant => Braille,
            Braille => Mixed,
        };
        match self.art {
            Mixed => "mixed",
            Quadrant => "quadrant",
            Braille => "braille",
        }
    }

    pub fn game(&self) -> &str {
        self.games.get(self.current).map(|s| s.as_str()).unwrap_or("-")
    }

    /// `game 2/3: game2`, as the original menu bar put it.
    pub fn game_label(&self) -> String {
        format!(
            "game {}/{}: {}",
            self.current + 1,
            self.games.len().max(1),
            self.game()
        )
    }

    pub fn move_selection(&mut self, delta: i32) {
        if self.hierarchy.is_empty() {
            return;
        }
        let n = self.hierarchy.len() as i32;
        self.selected = (((self.selected as i32 + delta) % n + n) % n) as usize;
    }

    pub fn note(&mut self, s: impl Into<String>) {
        self.log.push(s.into());
        if self.log.len() > 200 {
            self.log.remove(0);
        }
    }
}
