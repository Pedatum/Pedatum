//! Reads homunc's output to learn what a game declared.
//!
//! A game's `.hom` files define components (`pub struct`) and systems
//! (`pub fn <name>_system`). A system's parameter list is its query, so the
//! generated glue can be derived from the signature alone.

/// Types the engine's Homun shim defines. They are not game components.
pub const SHIM_TYPES: &[&str] = &["Vec2", "Vec3", "Transform", "Collider"];

#[derive(Debug, Clone, PartialEq)]
pub enum Param {
    /// The node's transform, in the module's own script-side type.
    Transform,
    /// A game component. `by_ref` distinguishes `&mut X` from a by-value `X`.
    Component { ty: String, by_ref: bool },
    /// The tick delta.
    Dt,
}

#[derive(Debug, Clone)]
pub struct System {
    pub name: String,
    pub params: Vec<Param>,
}

#[derive(Debug, Clone, Default)]
pub struct Module {
    /// Module name, from the `.hom` file stem.
    pub name: String,
    /// Component types: structs this module declares that something actually
    /// attaches to an entity. A struct used only as nested data — the element
    /// type of a list field, say — is not a component.
    pub components: Vec<String>,
    pub systems: Vec<System>,
    /// Collision pairs the module asks for, from `overlapping("A", "B")`.
    pub overlap_pairs: Vec<(String, String)>,
    /// Every struct the module declares, component or not.
    pub declared: Vec<String>,
}

/// Split a parameter list on commas that are not nested.
fn split_params(list: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for ch in list.chars() {
        match ch {
            '<' | '(' | '[' => {
                depth += 1;
                cur.push(ch);
            }
            '>' | ')' | ']' => {
                depth -= 1;
                cur.push(ch);
            }
            ',' if depth == 0 => {
                out.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// Classify one `name: type` parameter.
fn classify(param: &str) -> Option<Param> {
    let (_, ty) = param.split_once(':')?;
    let ty = ty.trim();
    let (by_ref, ty) = match ty.strip_prefix("&mut ") {
        Some(rest) => (true, rest.trim()),
        None => (false, ty),
    };
    Some(match ty {
        "f32" | "f64" => Param::Dt,
        "Transform" => Param::Transform,
        other => Param::Component {
            ty: other.to_string(),
            by_ref,
        },
    })
}

/// Find the two string literals in `overlapping("A", "B")`.
///
/// The generated call is `overlapping("A".to_string(), "B".to_string())`, so the
/// first `)` belongs to `.to_string()` — take the first two literals instead of
/// trying to find the closing paren.
fn overlap_pair(line: &str) -> Option<(String, String)> {
    let args = line.split_once("overlapping(")?.1;
    let mut lits = args.split('"').skip(1).step_by(2);
    Some((lits.next()?.to_string(), lits.next()?.to_string()))
}

/// Parse one module's generated Rust.
pub fn parse_module(name: &str, generated: &str) -> Module {
    let mut m = Module {
        name: name.to_string(),
        ..Default::default()
    };
    for line in generated.lines() {
        let t = line.trim_start();

        if let Some(rest) = t.strip_prefix("pub struct ") {
            let ty: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !ty.is_empty() && !SHIM_TYPES.contains(&ty.as_str()) {
                m.declared.push(ty);
            }
            continue;
        }

        if let Some(rest) = t.strip_prefix("pub fn ") {
            if let Some((fname, tail)) = rest.split_once('(') {
                if fname.ends_with("_system") {
                    let list = tail.rsplit_once(')').map(|(a, _)| a).unwrap_or(tail);
                    let params = split_params(list).iter().filter_map(|p| classify(p)).collect();
                    m.systems.push(System {
                        name: fname.to_string(),
                        params,
                    });
                }
            }
            continue;
        }

        if t.contains("overlapping(") {
            if let Some(pair) = overlap_pair(t) {
                if !m.overlap_pairs.contains(&pair) {
                    m.overlap_pairs.push(pair);
                }
            }
        }
    }

    // A component is a declared struct that a system queries, or that a
    // collision check names. Anything else is plain data.
    let mut used: Vec<String> = Vec::new();
    for sys in &m.systems {
        for p in &sys.params {
            if let Param::Component { ty, .. } = p {
                if !used.contains(ty) {
                    used.push(ty.clone());
                }
            }
        }
    }
    for (a, b) in &m.overlap_pairs {
        for name in [a, b] {
            if !used.contains(name) {
                used.push(name.clone());
            }
        }
    }
    m.components = m
        .declared
        .iter()
        .filter(|d| used.contains(d))
        .cloned()
        .collect();
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAYER: &str = r#"
pub struct Vec3 { pub x: f32, pub y: f32, pub z: f32 }
pub struct Transform { pub translation: Vec3, pub scale: Vec3 }
pub struct Collider { pub size: Vec2 }
pub struct PlayerControlled { pub gravity: f32, pub vy: f32 }
pub fn player_system(t: &mut Transform, p: &mut PlayerControlled, mut dt: f32) {
    if action("jump".to_string()) { }
}
"#;

    #[test]
    fn shim_types_are_not_components() {
        let m = parse_module("player", PLAYER);
        assert_eq!(m.components, vec!["PlayerControlled"]);
        assert!(!m.declared.iter().any(|d| SHIM_TYPES.contains(&d.as_str())));
    }

    #[test]
    fn a_system_signature_becomes_a_query() {
        let m = parse_module("player", PLAYER);
        assert_eq!(m.systems.len(), 1);
        let s = &m.systems[0];
        assert_eq!(s.name, "player_system");
        assert_eq!(
            s.params,
            vec![
                Param::Transform,
                Param::Component { ty: "PlayerControlled".into(), by_ref: true },
                Param::Dt,
            ]
        );
    }

    #[test]
    fn by_value_component_is_distinguished_from_by_ref() {
        let src = "pub struct ScrollX { pub speed: f32 }\n\
                   pub fn scroll_system(t: &mut Transform, mut s: ScrollX, mut dt: f32) { }";
        let m = parse_module("scroller", src);
        assert_eq!(
            m.systems[0].params[1],
            Param::Component { ty: "ScrollX".into(), by_ref: false }
        );
    }

    #[test]
    fn a_system_without_a_transform_has_none() {
        let src = "pub struct Run { pub crashes: i32 }\n\
                   pub fn obstacle_system(run: &mut Run) { }";
        let m = parse_module("obstacle", src);
        assert_eq!(
            m.systems[0].params,
            vec![Param::Component { ty: "Run".into(), by_ref: true }]
        );
    }

    #[test]
    fn overlap_pairs_are_discovered() {
        let src = r#"pub fn obstacle_system(run: &mut Run) {
            let hits = overlapping("PlayerControlled".to_string(), "Obstacle".to_string());
        }"#;
        let m = parse_module("obstacle", src);
        assert_eq!(
            m.overlap_pairs,
            vec![("PlayerControlled".to_string(), "Obstacle".to_string())]
        );
    }

    #[test]
    fn non_system_functions_are_ignored() {
        let src = "pub fn current_line(d: Dialogue) -> Line { }\n\
                   pub fn dialogue_system(d: &mut Dialogue) { }";
        let m = parse_module("dialogue", src);
        assert_eq!(m.systems.len(), 1);
        assert_eq!(m.systems[0].name, "dialogue_system");
    }

    /// A struct used only as nested data is not a component.
    #[test]
    fn nested_data_structs_are_not_components() {
        let src = "pub struct Line { pub speaker: String }\n\
                   pub struct Dialogue { pub index: i32 }\n\
                   pub fn dialogue_system(d: &mut Dialogue) { }";
        let m = parse_module("dialogue", src);
        assert_eq!(m.components, vec!["Dialogue"]);
        assert!(m.declared.contains(&"Line".to_string()));
    }

    /// A component no system takes is still a component when a collision check
    /// names it.
    #[test]
    fn a_component_named_only_by_overlapping_is_kept() {
        let src = r#"pub struct Obstacle { }
            pub struct Run { pub crashes: i32 }
            pub fn obstacle_system(run: &mut Run) {
                let hits = overlapping("PlayerControlled".to_string(), "Obstacle".to_string());
            }"#;
        let m = parse_module("obstacle", src);
        assert!(m.components.contains(&"Obstacle".to_string()));
        assert!(m.components.contains(&"Run".to_string()));
    }

    #[test]
    fn duplicate_overlap_pairs_collapse() {
        let src = r#"
            let a = overlapping("A".to_string(), "B".to_string());
            let b = overlapping("A".to_string(), "B".to_string());
        "#;
        assert_eq!(parse_module("m", src).overlap_pairs.len(), 1);
    }
}
