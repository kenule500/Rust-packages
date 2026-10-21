//! Scene model: canvas, layers, transforms, and identifiers.

use std::fmt;

use crate::error::CompileError;

/// A rational frame rate such as `30000/1001`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rational {
    /// Numerator.
    pub numerator: u32,
    /// Denominator; must be non-zero.
    pub denominator: u32,
}

impl Rational {
    /// Creates a reduced rational, rejecting a zero numerator or denominator.
    pub fn new(numerator: u32, denominator: u32) -> Result<Self, CompileError> {
        if numerator == 0 || denominator == 0 {
            return Err(CompileError::InvalidRational {
                numerator,
                denominator,
            });
        }
        let divisor = gcd(numerator, denominator);
        Ok(Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        })
    }

    /// Returns the rational as `numerator/denominator`.
    #[must_use]
    pub fn as_fraction(self) -> String {
        format!("{}/{}", self.numerator, self.denominator)
    }
}

impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_fraction())
    }
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}

/// A validated layer identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayerId(String);

impl LayerId {
    /// Validates and creates a layer identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, CompileError> {
        let value = value.into();
        if value.is_empty() {
            return Err(CompileError::EmptyLayerId);
        }
        if !is_safe_identifier(&value) {
            return Err(CompileError::InvalidLayerId { value });
        }
        Ok(Self(value))
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A validated external source identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceId(String);

impl SourceId {
    /// Validates and creates a source identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, CompileError> {
        let value = value.into();
        if value.is_empty() {
            return Err(CompileError::EmptySourceId);
        }
        if !is_safe_identifier(&value) {
            return Err(CompileError::InvalidSourceId { value });
        }
        Ok(Self(value))
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub(crate) fn is_safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && value
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
}

/// Canvas dimensions and frame rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Canvas {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Frame rate.
    pub fps: Rational,
}

impl Canvas {
    /// Creates a canvas, rejecting zero dimensions.
    pub fn new(width: u32, height: u32, fps: Rational) -> Result<Self, CompileError> {
        if width == 0 || height == 0 {
            return Err(CompileError::EmptyCanvas);
        }
        Ok(Self { width, height, fps })
    }
}

/// A solid background colour in RGBA order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Background {
    /// Red channel.
    pub red: u8,
    /// Green channel.
    pub green: u8,
    /// Blue channel.
    pub blue: u8,
    /// Alpha channel.
    pub alpha: u8,
}

impl Background {
    /// Creates a background from RGBA channels.
    #[must_use]
    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    /// Returns the FFmpeg colour literal (`0xRRGGBBAA`).
    #[must_use]
    pub fn as_ffmpeg(self) -> String {
        format!(
            "0x{:02X}{:02X}{:02X}{:02X}",
            self.red, self.green, self.blue, self.alpha
        )
    }
}

impl Default for Background {
    fn default() -> Self {
        Self::rgba(0, 0, 0, 255)
    }
}

/// Placement and size of a layer inside the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transform {
    /// Horizontal offset in pixels.
    pub x: i32,
    /// Vertical offset in pixels.
    pub y: i32,
    /// Display width in pixels.
    pub width: u32,
    /// Display height in pixels.
    pub height: u32,
}

impl Transform {
    /// Creates a transform.
    #[must_use]
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Compositing mode for a layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    /// Standard source-over compositing.
    Normal,
    /// Multiply blend.
    Multiply,
    /// Screen blend.
    Screen,
    /// Overlay blend.
    Overlay,
    /// Additive blend.
    Add,
    /// A mode the compiler refuses to guess at.
    Unsupported,
}

impl BlendMode {
    /// Returns the FFmpeg `blend` mode name.
    #[must_use]
    pub const fn as_ffmpeg(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Multiply => "multiply",
            Self::Screen => "screen",
            Self::Overlay => "overlay",
            Self::Add => "addition",
            Self::Unsupported => "unsupported",
        }
    }

    /// Returns whether the compiler supports the mode.
    #[must_use]
    pub const fn is_supported(self) -> bool {
        !matches!(self, Self::Unsupported)
    }
}

/// One visual layer.
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    /// Layer identifier, unique inside a scene.
    pub id: LayerId,
    /// External source that provides the layer image.
    pub source: SourceId,
    /// Placement and size.
    pub transform: Transform,
    /// Stacking order; larger values draw later.
    pub z: i32,
    /// Whether the layer participates in the graph.
    pub visible: bool,
    /// Compositing mode.
    pub blend: BlendMode,
    /// Opacity in `0.0..=1.0`.
    pub opacity: f32,
    /// Optional parent layer used for grouping.
    pub parent: Option<LayerId>,
}

impl Layer {
    /// Creates a visible, fully opaque layer.
    #[must_use]
    pub fn new(id: LayerId, source: SourceId, transform: Transform, z: i32) -> Self {
        Self {
            id,
            source,
            transform,
            z,
            visible: true,
            blend: BlendMode::Normal,
            opacity: 1.0,
            parent: None,
        }
    }
}

/// A complete scene description.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    /// Canvas dimensions and frame rate.
    pub canvas: Canvas,
    /// Background colour.
    pub background: Background,
    /// Layers in declaration order; compilation normalises the order.
    pub layers: Vec<Layer>,
}

impl Scene {
    /// Creates a scene.
    #[must_use]
    pub fn new(canvas: Canvas, background: Background, layers: Vec<Layer>) -> Self {
        Self {
            canvas,
            background,
            layers,
        }
    }
}
