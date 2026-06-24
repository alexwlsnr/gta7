// src/cli_args.rs
use std::path::PathBuf;
use raylib::ffi::Vector3;

#[derive(Debug, Clone, PartialEq)]
pub struct Args {
    pub test: bool,
    pub scene: String,
    pub time: Option<f32>,
    pub camera: CameraSpec,
    pub seed: u64,
    pub cars: u32,
    pub peds: u32,
    pub screenshot: Option<PathBuf>,
    pub disable: PostFxMaskStub,
    pub freeze_time: bool,
    pub show_bounds: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CameraSpec {
    BehindPlayer,
    Free { pos: Vector3, yaw: f32, pitch: f32 },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PostFxMaskStub;

pub fn parse_args() -> Args {
    Args {
        test: false,
        scene: String::from("headlight_closeup"),
        time: None,
        camera: CameraSpec::BehindPlayer,
        seed: 0xC0FFEE,
        cars: 0,
        peds: 0,
        screenshot: None,
        disable: PostFxMaskStub,
        freeze_time: false,
        show_bounds: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_when_no_args() {
        let args = parse_args();
        assert!(!args.test);
        assert_eq!(args.scene, "headlight_closeup");
        assert_eq!(args.seed, 0xC0FFEE);
        assert_eq!(args.cars, 0);
    }
}
