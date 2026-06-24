// src/cli_args.rs
use std::ffi::OsString;
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
    pub disable: PostFxMask,
    pub freeze_time: bool,
    pub show_bounds: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            test: false,
            scene: String::from("headlight_closeup"),
            time: None,
            camera: CameraSpec::BehindPlayer,
            seed: 0xC0FFEE,
            cars: 0,
            peds: 0,
            screenshot: None,
            disable: PostFxMask::none(),
            freeze_time: false,
            show_bounds: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CameraSpec {
    BehindPlayer,
    Free { pos: Vector3, yaw: f32, pitch: f32 },
}

pub use crate::postfx_mask::PostFxMask;

pub fn parse_args() -> Args {
    parse_args_with(std::env::args_os())
}

pub fn parse_args_with<I, T>(it: I) -> Args
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let mut args = Args::default();
    let mut cams: Vec<f32> = Vec::new();
    let mut it = it.into_iter();
    let _bin = it.next(); // skip argv[0]
    for raw in it {
        let s = raw.into();
        let s = match s.to_str() {
            Some(s) => s,
            None => continue,
        };
        if let Some(rest) = s.strip_prefix("--") {
            if let Some((k, v)) = rest.split_once('=') {
                match k {
                    "test" => args.test = true,
                    "scene" => args.scene = v.to_string(),
                    "time" => args.time = v.parse().ok(),
                    "seed" => args.seed = v.parse().unwrap_or(args.seed),
                    "cars" => args.cars = v.parse().unwrap_or(0),
                    "peds" => args.peds = v.parse().unwrap_or(0),
                    "camera" => {
                        if v == "behind_player" {
                            args.camera = CameraSpec::BehindPlayer;
                        } else {
                            cams = v.split(',')
                                .map(|t| t.trim().parse::<f32>().unwrap_or(0.0))
                                .collect();
                        }
                    }
                    "screenshot" => args.screenshot = Some(PathBuf::from(v)),
                    "disable" => args.disable = PostFxMask::from_csv(v),
                    _ => {} // unknown flags are ignored
                }
            } else {
                match rest {
                    "test" => args.test = true,
                    "freeze-time" => args.freeze_time = true,
                    "show-bounds" => args.show_bounds = true,
                    _ => {}
                }
            }
        }
    }
    match cams.len() {
        3 => args.camera = CameraSpec::Free {
            pos: Vector3 { x: cams[0], y: cams[1], z: cams[2] },
            yaw: 0.0, pitch: 0.0,
        },
        5 => args.camera = CameraSpec::Free {
            pos: Vector3 { x: cams[0], y: cams[1], z: cams[2] },
            yaw: cams[3], pitch: cams[4],
        },
        _ => {} // leave as default
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn with_args(args: &[&str], f: impl FnOnce(Args)) {
        let mut iter: Vec<OsString> = args.iter().map(Into::into).collect();
        iter.insert(0, OsString::from("gta7"));
        let parsed = parse_args_with(iter.into_iter());
        f(parsed);
    }

    #[test]
    fn test_flag_sets_test_mode() {
        with_args(&["--test"], |a| assert!(a.test));
    }

    #[test]
    fn scene_parses_string() {
        with_args(&["--test", "--scene=night_street"], |a| {
            assert_eq!(a.scene, "night_street");
        });
    }

    #[test]
    fn time_is_optional_and_parses_floats() {
        with_args(&["--test", "--time=6.5"], |a| {
            assert_eq!(a.time, Some(6.5));
        });
        with_args(&["--test"], |a| assert_eq!(a.time, None));
    }

    #[test]
    fn camera_xyz() {
        with_args(&["--test", "--camera=1.0,2.0,3.0"], |a| match a.camera {
            CameraSpec::Free { pos, yaw, pitch } => {
                assert_eq!(pos.x, 1.0);
                assert_eq!(pos.y, 2.0);
                assert_eq!(pos.z, 3.0);
                assert_eq!(yaw, 0.0);
                assert_eq!(pitch, 0.0);
            }
            _ => panic!("expected Free"),
        });
    }

    #[test]
    fn camera_xyz_yaw_pitch() {
        with_args(&["--test", "--camera=1,2,3,45,-15"], |a| match a.camera {
            CameraSpec::Free { pos, yaw, pitch } => {
                assert_eq!(pos.x, 1.0);
                assert_eq!(pos.y, 2.0);
                assert_eq!(pos.z, 3.0);
                assert_eq!(yaw, 45.0);
                assert_eq!(pitch, -15.0);
            }
            _ => panic!("expected Free"),
        });
    }

    #[test]
    fn camera_behind_player_keyword() {
        with_args(&["--test", "--camera=behind_player"], |a| {
            assert_eq!(a.camera, CameraSpec::BehindPlayer);
        });
    }

    #[test]
    fn seed_default_and_override() {
        with_args(&["--test"], |a| assert_eq!(a.seed, 0xC0FFEE));
        with_args(&["--test", "--seed=42"], |a| assert_eq!(a.seed, 42));
    }

    #[test]
    fn cars_peds_default_zero() {
        with_args(&["--test"], |a| {
            assert_eq!(a.cars, 0);
            assert_eq!(a.peds, 0);
        });
    }

    #[test]
    fn screenshot_path() {
        with_args(&["--test", "--screenshot=/tmp/x.png"], |a| {
            assert_eq!(a.screenshot.as_ref().unwrap().to_str(), Some("/tmp/x.png"));
        });
    }

    #[test]
    fn disable_csv_parses_to_mask() {
        with_args(&["--test", "--disable=bloom,crt"], |a| {
            assert!(a.disable.bloom);
            assert!(!a.disable.ssr);
            assert!(a.disable.crt);
            assert!(!a.disable.god_rays);
        });
    }

    #[test]
    fn freeze_time_and_show_bounds() {
        with_args(&["--test", "--freeze-time", "--show-bounds"], |a| {
            assert!(a.freeze_time);
            assert!(a.show_bounds);
        });
    }

    #[test]
    fn unknown_flag_is_ignored() {
        with_args(&["--test", "--garbage=42"], |a| {
            assert!(a.test);
            assert_eq!(a.time, None);
        });
    }
}
