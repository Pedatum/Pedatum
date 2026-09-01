//! Resolving a stage's signature into a call.
//!
//! The query is the parameter list, so scheduling is: intersect the columns
//! the signature names, write down one row of dense indices per match, and
//! cross the boundary exactly once. The per-entity loop lives inside the
//! module, where it is a plain monomorphised Rust loop.

use crate::registry::{ParamDef, StageDef};
use crate::world::World;
use anyhow::{bail, Result};
use se_abi::{Column as AbiColumn, Input, ParamKind, StageCall};
use std::collections::HashSet;

/// Check a stage against the bundle contract before it is ever run.
pub fn validate(world: &World, s: &StageDef) -> Result<()> {
    let mut seen = HashSet::new();
    for p in &s.params {
        match p.kind {
            ParamKind::Component => {
                if !seen.insert(p.name.as_str()) {
                    bail!("stage `{}` names `{}` twice", s.name, p.name);
                }
                let Some(l) = world.layout(&p.name) else {
                    bail!("stage `{}` wants `{}`, which the bundle does not define", s.name, p.name)
                };
                if l.hash != p.hash {
                    bail!(
                        "stage `{}` was built against a different `{}` — rebuild it against the current bundle",
                        s.name,
                        p.name
                    );
                }
            }
            ParamKind::Resource => {
                if p.name != Input::NAME {
                    bail!("stage `{}` wants unknown resource `{}`", s.name, p.name);
                }
                if p.size as usize != std::mem::size_of::<Input>() {
                    bail!("stage `{}` was built against a different Input", s.name);
                }
            }
            ParamKind::Dt => {}
        }
    }
    Ok(())
}

/// Entities carrying every component the signature names, plus the dense index
/// into each column for each of them.
fn resolve(world: &World, comps: &[&ParamDef]) -> (Vec<u32>, u32) {
    if comps.is_empty() {
        // No components means no query: the stage runs once.
        return (Vec::new(), 1);
    }
    let n = comps.len();
    let cols: Vec<_> = comps
        .iter()
        .map(|p| world.column(&p.name).expect("validated"))
        .collect();

    // Drive from the smallest column; the rest are membership tests.
    let (drive, _) = cols
        .iter()
        .enumerate()
        .min_by_key(|(_, c)| c.len())
        .expect("non-empty");

    let mut rows = Vec::with_capacity(cols[drive].len() * n);
    let mut slots = vec![0u32; n];
    let mut count = 0u32;
    'entity: for &e in cols[drive].entities() {
        for (i, c) in cols.iter().enumerate() {
            match c.row_of(e) {
                Some(r) => slots[i] = r,
                None => continue 'entity,
            }
        }
        rows.extend_from_slice(&slots);
        count += 1;
    }
    (rows, count)
}

/// Run one stage over the whole world.
pub fn run(world: &mut World, s: &StageDef, input: &Input, dt: f32) -> Result<()> {
    let comps: Vec<&ParamDef> = s.components().collect();
    let (rows, n_rows) = resolve(world, &comps);
    if n_rows == 0 {
        return Ok(());
    }

    // Column bases, taken one at a time. `validate` proved the names are
    // distinct, so these mutable borrows never alias.
    let mut abi_cols: Vec<AbiColumn> = Vec::with_capacity(comps.len());
    for p in &comps {
        let c = world.column_mut(&p.name).expect("validated");
        abi_cols.push(AbiColumn { base: c.base_ptr(), stride: c.stride });
    }

    // The only resource is `Input`, and a stage only ever reads it.
    let res: Vec<*const u8> = s
        .params
        .iter()
        .filter(|p| p.kind == ParamKind::Resource)
        .map(|_| input as *const Input as *const u8)
        .collect();

    let call = StageCall {
        cols: abi_cols.as_ptr(),
        n_cols: abi_cols.len() as u32,
        rows: rows.as_ptr(),
        n_rows,
        res: res.as_ptr(),
        n_res: res.len() as u32,
        dt,
    };

    // SAFETY: the frame matches the spec the module registered, and every
    // pointer in it outlives the call.
    unsafe { (s.run)(&call) };
    Ok(())
}
