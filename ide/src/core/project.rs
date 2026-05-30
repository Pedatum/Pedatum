use std::path::{Path, PathBuf};

#[allow(dead_code)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub depth: usize,
    pub expanded: bool,
}

pub struct ProjectState {
    pub entries: Vec<FileEntry>,
    pub selected: usize,
}

impl ProjectState {
    pub fn scan(root: &Path) -> Self {
        let mut entries = Vec::new();
        if root.is_dir() {
            scan_dir(root, root, 0, &mut entries);
        }
        Self {
            entries,
            selected: 0,
        }
    }

    pub fn visible_entries(&self) -> Vec<usize> {
        let mut visible = Vec::new();
        let mut skip_below: Option<usize> = None;
        for (i, entry) in self.entries.iter().enumerate() {
            if let Some(depth) = skip_below {
                if entry.depth > depth {
                    continue;
                }
                skip_below = None;
            }
            visible.push(i);
            if entry.is_dir && !entry.expanded {
                skip_below = Some(entry.depth);
            }
        }
        visible
    }

    pub fn move_up(&mut self) {
        let visible = self.visible_entries();
        if let Some(pos) = visible.iter().position(|&i| i == self.selected) {
            if pos > 0 {
                self.selected = visible[pos - 1];
            }
        }
    }

    pub fn move_down(&mut self) {
        let visible = self.visible_entries();
        if let Some(pos) = visible.iter().position(|&i| i == self.selected) {
            if pos + 1 < visible.len() {
                self.selected = visible[pos + 1];
            }
        }
    }

    pub fn toggle_expand(&mut self) {
        if let Some(entry) = self.entries.get_mut(self.selected) {
            if entry.is_dir {
                entry.expanded = !entry.expanded;
            }
        }
    }
}

fn scan_dir(root: &Path, dir: &Path, depth: usize, entries: &mut Vec<FileEntry>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut children: Vec<_> = read
        .filter_map(|e| e.ok())
        .filter(|e| {
            !e.file_name()
                .to_str()
                .map_or(false, |n| n.starts_with('.'))
        })
        .collect();
    children.sort_by(|a, b| {
        let a_dir = a.path().is_dir();
        let b_dir = b.path().is_dir();
        b_dir.cmp(&a_dir).then_with(|| a.file_name().cmp(&b.file_name()))
    });
    for child in children {
        let path = child.path();
        let is_dir = path.is_dir();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let rel_path = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        entries.push(FileEntry {
            name,
            path: rel_path,
            is_dir,
            depth,
            expanded: true,
        });
        if is_dir {
            scan_dir(root, &path, depth + 1, entries);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_tree(tmp: &Path) {
        fs::create_dir_all(tmp.join("assets/textures")).unwrap();
        fs::write(tmp.join("assets/scene.ron"), "()").unwrap();
        fs::write(tmp.join("assets/textures/grass.png"), "").unwrap();
        fs::write(tmp.join("readme.txt"), "hi").unwrap();
    }

    #[test]
    fn scan_sorts_dirs_first() {
        let tmp = tempfile::tempdir().unwrap();
        make_tree(tmp.path());
        let state = ProjectState::scan(tmp.path());
        let names: Vec<&str> = state.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names[0], "assets");
        assert!(state.entries[0].is_dir);
    }

    #[test]
    fn scan_sets_depth() {
        let tmp = tempfile::tempdir().unwrap();
        make_tree(tmp.path());
        let state = ProjectState::scan(tmp.path());
        let tex = state.entries.iter().find(|e| e.name == "textures").unwrap();
        assert_eq!(tex.depth, 1);
        let grass = state.entries.iter().find(|e| e.name == "grass.png").unwrap();
        assert_eq!(grass.depth, 2);
    }

    #[test]
    fn navigation_respects_collapsed() {
        let tmp = tempfile::tempdir().unwrap();
        make_tree(tmp.path());
        let mut state = ProjectState::scan(tmp.path());
        state.toggle_expand(); // collapse "assets"
        let visible = state.visible_entries();
        let visible_names: Vec<&str> = visible
            .iter()
            .map(|&i| state.entries[i].name.as_str())
            .collect();
        assert!(!visible_names.contains(&"scene.ron"));
        assert!(!visible_names.contains(&"textures"));
        assert!(visible_names.contains(&"readme.txt"));
    }

    #[test]
    fn move_down_skips_collapsed_children() {
        let tmp = tempfile::tempdir().unwrap();
        make_tree(tmp.path());
        let mut state = ProjectState::scan(tmp.path());
        state.toggle_expand(); // collapse "assets" (selected=0)
        state.move_down();
        assert_eq!(state.entries[state.selected].name, "readme.txt");
    }

    #[test]
    fn toggle_expand_noop_on_file() {
        let tmp = tempfile::tempdir().unwrap();
        make_tree(tmp.path());
        let mut state = ProjectState::scan(tmp.path());
        let file_idx = state
            .entries
            .iter()
            .position(|e| e.name == "readme.txt")
            .unwrap();
        state.selected = file_idx;
        state.toggle_expand();
        // no panic, no change
    }

    #[test]
    fn empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let state = ProjectState::scan(tmp.path());
        assert!(state.entries.is_empty());
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn nonexistent_root() {
        let state = ProjectState::scan(Path::new("/nonexistent/path"));
        assert!(state.entries.is_empty());
    }
}
