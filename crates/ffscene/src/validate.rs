//! Scene validation performed before any graph is produced.

use std::collections::{BTreeMap, BTreeSet};

use crate::error::CompileError;
use crate::scene::{LayerId, Scene};

/// Validates a scene without producing a graph.
///
/// Validation is all-or-nothing: on error no partial graph exists. The checks
/// cover duplicate layer ids, unknown parents, parent cycles, transform bounds,
/// opacity, and blend modes (FR-002).
pub fn validate(scene: &Scene) -> Result<(), CompileError> {
    let canvas = scene.canvas;

    let mut ids: BTreeSet<&LayerId> = BTreeSet::new();
    for layer in &scene.layers {
        if !ids.insert(&layer.id) {
            return Err(CompileError::DuplicateLayer {
                layer: layer.id.to_string(),
            });
        }
    }

    for layer in &scene.layers {
        if !layer.opacity.is_finite() {
            return Err(CompileError::NonFiniteOpacity {
                layer: layer.id.to_string(),
            });
        }
        if !(0.0..=1.0).contains(&layer.opacity) {
            return Err(CompileError::OpacityOutOfRange {
                layer: layer.id.to_string(),
            });
        }
        if !layer.blend.is_supported() {
            return Err(CompileError::UnsupportedBlend {
                layer: layer.id.to_string(),
            });
        }
        if layer.transform.width == 0 || layer.transform.height == 0 {
            return Err(CompileError::TransformOutOfBounds {
                layer: layer.id.to_string(),
            });
        }
        let right = i64::from(layer.transform.x) + i64::from(layer.transform.width);
        let bottom = i64::from(layer.transform.y) + i64::from(layer.transform.height);
        if layer.transform.x < 0
            || layer.transform.y < 0
            || right > i64::from(canvas.width)
            || bottom > i64::from(canvas.height)
        {
            return Err(CompileError::TransformOutOfBounds {
                layer: layer.id.to_string(),
            });
        }
    }

    let parents: BTreeMap<&LayerId, Option<&LayerId>> = scene
        .layers
        .iter()
        .map(|layer| (&layer.id, layer.parent.as_ref()))
        .collect();

    for layer in &scene.layers {
        let Some(parent) = layer.parent.as_ref() else {
            continue;
        };
        if !parents.contains_key(parent) {
            return Err(CompileError::UnknownParent {
                layer: layer.id.to_string(),
                parent: parent.to_string(),
            });
        }
    }

    for layer in &scene.layers {
        let mut seen = BTreeSet::new();
        let mut current = Some(&layer.id);
        while let Some(id) = current {
            if !seen.insert(id) {
                return Err(CompileError::Cycle {
                    layer: layer.id.to_string(),
                });
            }
            current = parents.get(id).copied().flatten();
        }
    }

    Ok(())
}
