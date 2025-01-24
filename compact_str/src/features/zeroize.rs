//! Implements the [`zeroize::Zeroize`] trait for [`CompactString`]

use crate::CompactString;
use zeroize::Zeroize;

#[cfg_attr(docsrs, doc(cfg(feature = "zeroize")))]
impl Zeroize for CompactString {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::String;
    use test_strategy::proptest;

    use super::*;
    use crate::tests::rand_unicode;

    #[test]
    fn smoketest_zeroize() {
        let mut short = CompactString::from("hello");
        short.zeroize();
        assert_eq!(short, "\0\0\0\0\0");

        let mut long = CompactString::from("I am a long string that will be on the heap");
        long.zeroize();
        assert_eq!(long, "\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0");
        assert!(long.is_heap_allocated());
    }

    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_zeroize(#[strategy(rand_unicode())] s: String) {
        let mut compact = CompactString::new(s.clone());
        let mut control = s.clone();

        compact.zeroize();
        control.zeroize();

        assert_eq!(compact, control);
    }
}
