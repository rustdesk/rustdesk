#[cfg(test)]
mod fixed_big_int_test;

use std::fmt;

// FixedBigInt is the fix-sized multi-word integer.
pub(crate) struct FixedBigInt {
    bits: Vec<u64>,
    n: usize,
    msb_mask: u64,
}

impl fmt::Display for FixedBigInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = String::new();
        for i in (0..self.bits.len()).rev() {
            out += format!("{:016X}", self.bits[i]).as_str();
        }

        write!(f, "{out}")
    }
}

impl FixedBigInt {
    pub(crate) fn new(n: usize) -> Self {
        let mut chunk_size = n.div_ceil(64);
        if chunk_size == 0 {
            chunk_size = 1;
        }

        FixedBigInt {
            bits: vec![0; chunk_size],
            n,
            msb_mask: if n % 64 == 0 {
                u64::MAX
            } else {
                (1 << (64 - n % 64)) - 1
            },
        }
    }

    // lsh is the left shift operation.
    pub(crate) fn lsh(&mut self, n: usize) {
        if n == 0 {
            return;
        }
        let n_chunk = (n / 64) as isize;
        let n_n = n % 64;

        for i in (0..self.bits.len() as isize).rev() {
            let mut carry: u64 = 0;
            if i - n_chunk >= 0 {
                carry = if n_n >= 64 {
                    0
                } else {
                    self.bits[(i - n_chunk) as usize] << n_n
                };
                if i - n_chunk > 0 {
                    carry |= if n_n == 0 {
                        0
                    } else {
                        self.bits[(i - n_chunk - 1) as usize] >> (64 - n_n)
                    };
                }
            }
            self.bits[i as usize] = if n >= 64 {
                carry
            } else {
                (self.bits[i as usize] << n) | carry
            };
        }

        let last = self.bits.len() - 1;
        self.bits[last] &= self.msb_mask;
    }

    // bit returns i-th bit of the fixedBigInt.
    pub(crate) fn bit(&self, i: usize) -> usize {
        if i >= self.n {
            return 0;
        }
        let chunk = i / 64;
        let pos = i % 64;
        usize::from(self.bits[chunk] & (1 << pos) != 0)
    }

    // set_bit sets i-th bit to 1.
    pub(crate) fn set_bit(&mut self, i: usize) {
        if i >= self.n {
            return;
        }
        let chunk = i / 64;
        let pos = i % 64;
        self.bits[chunk] |= 1 << pos;
    }
}
