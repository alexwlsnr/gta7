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
                _ => {} // unknown tokens are ignored (lenient for dev UX)
            }
        }
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_is_all_false_and_any_false() {
        let m = PostFxMask::none();
        assert!(!m.any());
        assert!(!m.bloom && !m.ssr && !m.crt && !m.god_rays);
        assert!(!m.fog && !m.shadows && !m.dyn_sky);
    }

    #[test]
    fn default_matches_none() {
        assert_eq!(PostFxMask::default(), PostFxMask::none());
    }

    #[test]
    fn any_returns_true_when_any_pass_set() {
        for m in [
            PostFxMask { bloom: true, ..PostFxMask::none() },
            PostFxMask { ssr: true, ..PostFxMask::none() },
            PostFxMask { crt: true, ..PostFxMask::none() },
            PostFxMask { god_rays: true, ..PostFxMask::none() },
            PostFxMask { fog: true, ..PostFxMask::none() },
            PostFxMask { shadows: true, ..PostFxMask::none() },
            PostFxMask { dyn_sky: true, ..PostFxMask::none() },
        ] {
            assert!(m.any());
        }
    }

    #[test]
    fn from_csv_empty_string_is_none() {
        let m = PostFxMask::from_csv("");
        assert_eq!(m, PostFxMask::none());
    }

    #[test]
    fn from_csv_recognises_all_seven_fields() {
        let m = PostFxMask::from_csv(
            "bloom,ssr,crt,godrays,fog,shadows,dyn",
        );
        assert!(m.bloom);
        assert!(m.ssr);
        assert!(m.crt);
        assert!(m.god_rays);
        assert!(m.fog);
        assert!(m.shadows);
        assert!(m.dyn_sky);
    }

    #[test]
    fn from_csv_is_case_insensitive() {
        let m = PostFxMask::from_csv("BLOOM,Crt,GoDrAyS");
        assert!(m.bloom);
        assert!(m.crt);
        assert!(m.god_rays);
    }

    #[test]
    fn from_csv_accepts_aliases() {
        // godrays <-> god_rays; shadows <-> shadow; dyn <-> dyn_sky <-> sky
        assert!(PostFxMask::from_csv("god_rays").god_rays);
        assert!(PostFxMask::from_csv("shadow").shadows);
        assert!(PostFxMask::from_csv("dyn_sky").dyn_sky);
        assert!(PostFxMask::from_csv("sky").dyn_sky);
    }

    #[test]
    fn from_csv_trims_whitespace_around_tokens() {
        let m = PostFxMask::from_csv("  bloom , crt  ");
        assert!(m.bloom);
        assert!(m.crt);
        assert!(!m.ssr);
    }

    #[test]
    fn from_csv_silently_drops_unknown_tokens() {
        // Spec promises lenient parsing for dev UX. Confirm unknown tokens don't crash
        // and don't set any pass.
        let m = PostFxMask::from_csv("bloom,wat,crt,xyz");
        assert!(m.bloom);
        assert!(m.crt);
        assert!(!m.ssr);
        assert!(!m.god_rays);
    }
}
