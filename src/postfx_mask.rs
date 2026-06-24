//! Bitset of post-processing passes that can be disabled at runtime.
//! Lives in its own module so `render` doesn't depend on `cli_args`.

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PostFxMask {
    pub bloom: bool,
    pub ssr: bool,
    pub crt: bool,
    pub god_rays: bool,
    pub fog: bool,
    pub shadows: bool,
    pub dyn_sky: bool,
}

impl PostFxMask {
    pub fn none() -> Self { Self::default() }

    pub fn any(&self) -> bool {
        self.bloom || self.ssr || self.crt || self.god_rays
            || self.fog || self.shadows || self.dyn_sky
    }

    pub fn from_csv(s: &str) -> Self {
        let mut m = Self::none();
        for tok in s.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            match tok.to_ascii_lowercase().as_str() {
                "bloom" => m.bloom = true,
                "ssr" => m.ssr = true,
                "crt" => m.crt = true,
                "godrays" | "god_rays" => m.god_rays = true,
                "fog" => m.fog = true,
                "shadows" | "shadow" => m.shadows = true,
                "dyn" | "dyn_sky" | "sky" => m.dyn_sky = true,
                _ => {} // unknown tokens are ignored
            }
        }
        m
    }
}
