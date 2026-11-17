//! Scalars held at a narrower alignment than their own, for the AST enums that carry one by
//! value.

/// An `f64` aligned to 4 bytes, for a number held inside an AST enum.
///
/// A bare `f64` raises every enum that holds it, and every enum holding that one by value, to
/// 8-byte alignment, padding each to a multiple of 8 on every row of its store and at every
/// recursion level that holds one. Packed, a value sits directly after its tag, and on wasm32,
/// where a pointer is 4 bytes, the enclosing enums stay 4-aligned. The value goes in and comes
/// out by copy, bit for bit, so `-0.0` and NaN read back as written and `==` is `f64`'s.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C, packed(4))]
pub struct PackedF64(f64);

const _: () = assert!(size_of::<PackedF64>() == 8 && align_of::<PackedF64>() == 4);

impl PackedF64 {
    /// Wrap a value.
    #[inline]
    pub const fn new(value: f64) -> Self {
        Self(value)
    }

    /// The wrapped value.
    #[inline]
    pub const fn get(self) -> f64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::PackedF64;

    #[test]
    fn packed_f64_keeps_the_bits_and_the_comparison() {
        for value in [0.0, -0.0, 1.5, f64::NAN, -f64::NAN, f64::INFINITY] {
            assert_eq!(PackedF64::new(value).get().to_bits(), value.to_bits());
        }
        assert_eq!(PackedF64::new(0.0), PackedF64::new(-0.0));
        assert_ne!(PackedF64::new(f64::NAN), PackedF64::new(f64::NAN));
    }
}
