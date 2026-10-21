//! Compile an in-memory scene and print the argument vector.
//!
//! The example touches no network, device, or process.

use ffscene::{
    compile, Background, Canvas, Layer, LayerId, Output, Rational, Scene, SourceId, Transform,
};

fn main() -> Result<(), ffscene::CompileError> {
    let canvas = Canvas::new(1920, 1080, Rational::new(30, 1)?)?;
    let scene = Scene::new(
        canvas,
        Background::rgba(8, 12, 20, 255),
        vec![
            Layer::new(
                LayerId::new("background")?,
                SourceId::new("camera-wide")?,
                Transform::new(0, 0, 1920, 1080),
                0,
            ),
            Layer::new(
                LayerId::new("host")?,
                SourceId::new("camera-host")?,
                Transform::new(1280, 620, 560, 420),
                10,
            ),
        ],
    );

    let graph = compile(
        &scene,
        &Output::File {
            path: "out.mkv".into(),
        },
    )?;
    println!("inputs: {:?}", graph.inputs);
    println!("filter_complex: {}", graph.filter_complex);
    for arg in &graph.args {
        println!("arg: {arg}");
    }
    Ok(())
}
