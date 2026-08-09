//! UI-independent state for text boxes drawn over the game viewport.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverlayAnchor {
    TopLeft,
    TopRight,
    #[default]
    BottomLeft,
    BottomRight,
    Center,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextBoxOverlay {
    pub id: String,
    pub title: Option<String>,
    pub lines: Vec<String>,
    pub anchor: OverlayAnchor,
    /// Requested width in terminal cells, including the border.
    pub width: u16,
    /// Optional maximum height in terminal cells, including the border.
    pub max_height: Option<u16>,
}

impl TextBoxOverlay {
    pub fn new(id: impl Into<String>, lines: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            id: id.into(),
            title: None,
            lines: lines.into_iter().map(Into::into).collect(),
            anchor: OverlayAnchor::default(),
            width: 38,
            max_height: None,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn anchor(mut self, anchor: OverlayAnchor) -> Self {
        self.anchor = anchor;
        self
    }

    pub fn width(mut self, width: u16) -> Self {
        self.width = width;
        self
    }

    pub fn max_height(mut self, max_height: u16) -> Self {
        self.max_height = Some(max_height);
        self
    }
}

#[derive(Debug, Default)]
pub struct OverlayState {
    boxes: Vec<TextBoxOverlay>,
}

impl OverlayState {
    /// Insert a new text box or replace the existing box with the same ID.
    /// Replacement keeps its position in the layer stack.
    pub fn show(&mut self, text_box: TextBoxOverlay) {
        if let Some(existing) = self.boxes.iter_mut().find(|item| item.id == text_box.id) {
            *existing = text_box;
        } else {
            self.boxes.push(text_box);
        }
    }

    pub fn dismiss(&mut self, id: &str) -> bool {
        let old_len = self.boxes.len();
        self.boxes.retain(|item| item.id != id);
        self.boxes.len() != old_len
    }

    pub fn clear(&mut self) {
        self.boxes.clear();
    }

    pub fn get(&self, id: &str) -> Option<&TextBoxOverlay> {
        self.boxes.iter().find(|item| item.id == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &TextBoxOverlay> {
        self.boxes.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.boxes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_replaces_a_box_without_duplicating_it() {
        let mut overlays = OverlayState::default();
        overlays.show(TextBoxOverlay::new("messages", ["first"]));
        overlays.show(TextBoxOverlay::new("messages", ["updated"]).width(50));

        assert_eq!(overlays.iter().count(), 1);
        let text_box = overlays.get("messages").unwrap();
        assert_eq!(text_box.lines, ["updated"]);
        assert_eq!(text_box.width, 50);
    }

    #[test]
    fn dismiss_reports_whether_a_box_existed() {
        let mut overlays = OverlayState::default();
        overlays.show(TextBoxOverlay::new("messages", ["hello"]));

        assert!(overlays.dismiss("messages"));
        assert!(!overlays.dismiss("messages"));
        assert!(overlays.is_empty());
    }
}
