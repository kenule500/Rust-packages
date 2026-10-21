//! Deterministic filter-graph and argument compilation.

use std::collections::{BTreeMap, BTreeSet};

use crate::error::CompileError;
use crate::output::Output;
use crate::scene::{BlendMode, Layer, LayerId, Scene, SourceId};
use crate::validate::validate;

/// Per-layer output labels produced by compilation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LayerLabelMap {
    labels: BTreeMap<LayerId, String>,
}

impl LayerLabelMap {
    /// Returns the label for a layer.
    #[must_use]
    pub fn get(&self, id: &LayerId) -> Option<&str> {
        self.labels.get(id).map(String::as_str)
    }

    /// Iterates in layer-id order.
    pub fn iter(&self) -> impl Iterator<Item = (&LayerId, &String)> {
        self.labels.iter()
    }

    /// Returns the number of mapped layers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.labels.len()
    }

    /// Returns whether no layers are mapped.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.labels.is_empty()
    }
}

/// The result of compiling a scene.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledGraph {
    /// The FFmpeg `filter_complex` expression.
    pub filter_complex: String,
    /// External source identifiers in input-index order.
    pub inputs: Vec<String>,
    /// Output labels, without brackets.
    pub outputs: Vec<String>,
    /// Layer-to-label mapping.
    pub layer_map: LayerLabelMap,
    /// Complete FFmpeg argument vector.
    pub args: Vec<String>,
}

/// Compiles a scene into a deterministic FFmpeg graph and argument vector.
///
/// The call is pure: it performs no I/O, resolves no DNS, opens no socket, and
/// spawns no process (FR-005).
pub fn compile(scene: &Scene, output: &Output) -> Result<CompiledGraph, CompileError> {
    validate(scene)?;
    output.validate()?;

    let mut layers: Vec<&Layer> = scene.layers.iter().filter(|layer| layer.visible).collect();
    layers.sort_by(|left, right| {
        (left.z, &left.id, &left.source).cmp(&(right.z, &right.id, &right.source))
    });

    let sources: BTreeSet<&SourceId> = scene.layers.iter().map(|layer| &layer.source).collect();
    let source_index: BTreeMap<&SourceId, usize> = sources
        .iter()
        .enumerate()
        .map(|(index, source)| (*source, index))
        .collect();

    let mut fragments = vec![format!(
        "color=c={}:s={}x{}:r={},format=rgba[base0]",
        scene.background.as_ffmpeg(),
        scene.canvas.width,
        scene.canvas.height,
        scene.canvas.fps
    )];
    let mut layer_map = LayerLabelMap::default();
    let mut base = String::from("base0");

    for (position, layer) in layers.iter().enumerate() {
        let index = source_index[&layer.source];
        let scaled = format!("l_{}", layer.id);
        let opacity = format_opacity(layer.opacity);
        fragments.push(format!(
            "[{index}:v]scale={}:{}:force_original_aspect_ratio=decrease,colorchannelmixer=aa={opacity},format=rgba[{scaled}]",
            layer.transform.width, layer.transform.height
        ));

        let next = format!("b{}", position + 1);
        if layer.blend == BlendMode::Normal {
            fragments.push(format!(
                "[{base}][{scaled}]overlay=x={}:y={}:format=auto[{next}]",
                layer.transform.x, layer.transform.y
            ));
        } else {
            let padded = format!("p_{}", layer.id);
            fragments.push(format!(
                "[{scaled}]scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:0:0:color=0x00000000,format=rgba[{padded}]",
                scene.canvas.width,
                scene.canvas.height,
                scene.canvas.width,
                scene.canvas.height
            ));
            fragments.push(format!(
                "[{base}][{padded}]blend=all_mode={}:all_opacity=1.0[{next}]",
                layer.blend.as_ffmpeg()
            ));
        }
        layer_map.labels.insert(layer.id.clone(), next.clone());
        base = next;
    }

    fragments.push(format!("[{base}]format=yuv420p,setpts=PTS-STARTPTS[out]"));
    let filter_complex = fragments.join(";");

    let mut args = vec![
        "-filter_complex".to_string(),
        filter_complex.clone(),
        "-map".to_string(),
        "[out]".to_string(),
    ];
    args.extend(output.args());

    Ok(CompiledGraph {
        filter_complex,
        inputs: sources.iter().map(ToString::to_string).collect(),
        outputs: vec!["out".to_string()],
        layer_map,
        args,
    })
}

fn format_opacity(opacity: f32) -> String {
    let mut text = format!("{opacity:.3}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    if text.is_empty() {
        text.push('0');
    }
    text
}
