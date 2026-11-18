//! `Core/ImageEffects/Drawings/DrawingEnums.cs`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawingPlacement {
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    MiddleCenter,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawingImageSizeMode {
    DontResize,
    AbsoluteSize,
    PercentageOfWatermark,
    PercentageOfCanvas,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawingShapeType {
    Rectangle,
    RoundedRectangle,
    Ellipse,
    Triangle,
    Diamond,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawingImageRotateFlipType {
    None = 0,
    Rotate90 = 1,
    Rotate180 = 2,
    Rotate270 = 3,
    FlipX = 4,
    Rotate90FlipX = 5,
    FlipY = 6,
    Rotate90FlipY = 7,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawingInterpolationMode {
    HighQualityBicubic,
    Bicubic,
    HighQualityBilinear,
    Bilinear,
    NearestNeighbor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawingCompositingMode {
    SourceOver,
    SourceCopy,
}
