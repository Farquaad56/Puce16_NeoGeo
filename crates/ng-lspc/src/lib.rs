#![forbid(unsafe_code)]
//! ng-lspc : LSPC (vidéo). Voir tasks/T400.
#[rustfmt::skip]
pub mod dac_table;
#[rustfmt::skip]
pub mod zoom_table;

#[cfg(test)]
mod smoke {
    #[test]
    fn it_builds() {}
}
