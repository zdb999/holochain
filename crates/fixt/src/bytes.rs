use crate::prelude::*;
use rand::Rng;

const UNPREDICTABLE_MIN_LEN: usize = 0;
const UNPREDICTABLE_MAX_LEN: usize = 32;

pub type Bytes = Vec<u8>;
pub type BytesNotEmpty = Vec<u8>;

// Simply generate "bytes" which is a Vec<u8>
// likely the most interesting is the Unpredictable curve that throws out random bytes in a vec
// of random length between 0 and 32 bytes long
fixturator!(
    Bytes,
    vec![],
    {
        let mut rng = rand::thread_rng();
        let len = rng.gen_range(UNPREDICTABLE_MIN_LEN, UNPREDICTABLE_MAX_LEN);
        let mut u8_fixturator = U8Fixturator::new(Unpredictable);
        let mut bytes = vec![];
        for _ in 0..len {
            bytes.push(u8_fixturator.next().unwrap());
        }
        bytes
    },
    {
        let mut u8_fixturator = U8Fixturator::new_indexed(Predictable, self.0.index);
        let mut bytes = vec![];
        for _ in 0..32 {
            bytes.push(u8_fixturator.next().unwrap());
        }
        self.0.index += 1;
        bytes
    }
);

// Simply generate "bytes" which is a Vec<u8>
// likely the most interesting is the Unpredictable curve that throws out random bytes in a vec
// of random length between 1 and 32 bytes long
// This version of Bytes is never empty.
fixturator!(
    BytesNotEmpty,
    vec![0u8],
    {
        let mut rng = rand::thread_rng();
        let len = rng.gen_range(1, UNPREDICTABLE_MAX_LEN);
        let mut u8_fixturator = U8Fixturator::new(Unpredictable);
        let mut bytes = vec![];
        for _ in 0..len {
            bytes.push(u8_fixturator.next().unwrap());
        }
        bytes
    },
    {
        let mut u8_fixturator = U8Fixturator::new_indexed(Predictable, self.0.index);
        let mut bytes = vec![];
        for _ in 0..32 {
            bytes.push(u8_fixturator.next().unwrap());
        }
        self.0.index += 1;
        bytes
    }
);

/// A type alias for a Vec<u8> whose fixturator is expected to only return
/// a Vec of length 36
pub type ThirtySixBytes = Vec<u8>;

// Simply generate "bytes" which is a Vec<u8> of 36 bytes
fixturator!(
    ThirtySixBytes;
    curve Empty [0; 36].to_vec();
    curve Predictable {
        let mut u8_fixturator = U8Fixturator::new(Unpredictable);
        let mut bytes = vec![];
        for _ in 0..36 {
            bytes.push(u8_fixturator.next().unwrap());
        }
        bytes
    };
    curve Unpredictable {
        let mut u8_fixturator = U8Fixturator::new_indexed(Predictable, self.0.index);
        let mut bytes = vec![];
        for _ in 0..36 {
            bytes.push(u8_fixturator.next().unwrap());
        }
        bytes
    };
);

/// A type alias for a Vec<u8> whose fixturator is expected to only return
/// a Vec of length 36
pub type ThirtyTwoBytes = Vec<u8>;

// Simply generate "bytes" which is a Vec<u8> of 36 bytes
fixturator!(
    ThirtyTwoBytes;
    curve Empty [0; 32].to_vec();
    curve Unpredictable {
        let mut u8_fixturator = U8Fixturator::new(Unpredictable);
        let mut bytes = vec![];
        for _ in 0..32 {
            bytes.push(u8_fixturator.next().unwrap());
        }
        bytes
    };
    curve Predictable {
        let mut u8_fixturator = U8Fixturator::new_indexed(Predictable, self.0.index);
        let mut bytes = vec![];
        for _ in 0..32 {
            bytes.push(u8_fixturator.next().unwrap());
        }
        bytes
    }
);
