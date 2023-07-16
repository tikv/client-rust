use std::io::Write;
use std::ptr;

use crate::internal_err;
use crate::Result;

const ENC_GROUP_SIZE: usize = 8;
const ENC_MARKER: u8 = 0xff;
const ENC_ASC_PADDING: [u8; ENC_GROUP_SIZE] = [0; ENC_GROUP_SIZE];
const ENC_DESC_PADDING: [u8; ENC_GROUP_SIZE] = [!0; ENC_GROUP_SIZE];

/// Returns the maximum encoded bytes size.
///
/// Duplicate from components/tikv_util/src/codec/bytes.rs.
pub fn max_encoded_bytes_size(n: usize) -> usize {
    (n / ENC_GROUP_SIZE + 1) * (ENC_GROUP_SIZE + 1)
}

pub trait BytesEncoder: Write {
    /// Refer: <https://github.com/facebook/mysql-5.6/wiki/MyRocks-record-format#memcomparable-format>
    ///
    /// Duplicate from components/tikv_util/src/codec/bytes.rs.
    fn encode_bytes(&mut self, key: &[u8], desc: bool) -> Result<()> {
        let len = key.len();
        let mut index = 0;
        let mut buf = [0; ENC_GROUP_SIZE];
        while index <= len {
            let remain = len - index;
            let mut pad: usize = 0;
            if remain > ENC_GROUP_SIZE {
                self.write_all(adjust_bytes_order(
                    &key[index..index + ENC_GROUP_SIZE],
                    desc,
                    &mut buf,
                ))?;
            } else {
                pad = ENC_GROUP_SIZE - remain;
                self.write_all(adjust_bytes_order(&key[index..], desc, &mut buf))?;
                if desc {
                    self.write_all(&ENC_DESC_PADDING[..pad])?;
                } else {
                    self.write_all(&ENC_ASC_PADDING[..pad])?;
                }
            }
            self.write_all(adjust_bytes_order(
                &[ENC_MARKER - (pad as u8)],
                desc,
                &mut buf,
            ))?;
            index += ENC_GROUP_SIZE;
        }
        Ok(())
    }
}

impl<T: Write> BytesEncoder for T {}

fn adjust_bytes_order<'a>(bs: &'a [u8], desc: bool, buf: &'a mut [u8]) -> &'a [u8] {
    if desc {
        let mut buf_idx = 0;
        for &b in bs {
            buf[buf_idx] = !b;
            buf_idx += 1;
        }
        &buf[..buf_idx]
    } else {
        bs
    }
}

/// Decodes bytes which are encoded by `encode_bytes` before just in place without malloc.
///
/// Duplicate from components/tikv_util/src/codec/bytes.rs.
pub fn decode_bytes_in_place(data: &mut Vec<u8>, desc: bool) -> Result<()> {
    if data.is_empty() {
        return Ok(());
    }
    let mut write_offset = 0;
    let mut read_offset = 0;
    loop {
        let marker_offset = read_offset + ENC_GROUP_SIZE;
        if marker_offset >= data.len() {
            return Err(internal_err!("unexpected EOF, original key = {:?}", data));
        };

        unsafe {
            // it is semantically equivalent to C's memmove()
            // and the src and dest may overlap
            // if src == dest do nothing
            ptr::copy(
                data.as_ptr().add(read_offset),
                data.as_mut_ptr().add(write_offset),
                ENC_GROUP_SIZE,
            );
        }
        write_offset += ENC_GROUP_SIZE;
        // everytime make ENC_GROUP_SIZE + 1 elements as a decode unit
        read_offset += ENC_GROUP_SIZE + 1;

        // the last byte in decode unit is for marker which indicates pad size
        let marker = data[marker_offset];
        let pad_size = if desc {
            marker as usize
        } else {
            (ENC_MARKER - marker) as usize
        };

        if pad_size > 0 {
            if pad_size > ENC_GROUP_SIZE {
                return Err(internal_err!("invalid key padding"));
            }

            // check the padding pattern whether validate or not
            let padding_slice = if desc {
                &ENC_DESC_PADDING[..pad_size]
            } else {
                &ENC_ASC_PADDING[..pad_size]
            };
            if &data[write_offset - pad_size..write_offset] != padding_slice {
                return Err(internal_err!("invalid key padding"));
            }
            unsafe {
                data.set_len(write_offset - pad_size);
            }
            if desc {
                for k in data {
                    *k = !*k;
                }
            }
            return Ok(());
        }
    }
}

#[cfg(test)]
pub mod test {
    use super::*;

    fn encode_bytes(bs: &[u8]) -> Vec<u8> {
        encode_order_bytes(bs, false)
    }

    fn encode_bytes_desc(bs: &[u8]) -> Vec<u8> {
        encode_order_bytes(bs, true)
    }

    fn encode_order_bytes(bs: &[u8], desc: bool) -> Vec<u8> {
        let cap = max_encoded_bytes_size(bs.len());
        let mut encoded = Vec::with_capacity(cap);
        encoded.encode_bytes(bs, desc).unwrap();
        encoded
    }

    #[test]
    fn test_enc_dec_bytes() {
        let pairs = vec![
            (
                vec![],
                vec![0, 0, 0, 0, 0, 0, 0, 0, 247],
                vec![255, 255, 255, 255, 255, 255, 255, 255, 8],
            ),
            (
                vec![0],
                vec![0, 0, 0, 0, 0, 0, 0, 0, 248],
                vec![255, 255, 255, 255, 255, 255, 255, 255, 7],
            ),
            (
                vec![1, 2, 3],
                vec![1, 2, 3, 0, 0, 0, 0, 0, 250],
                vec![254, 253, 252, 255, 255, 255, 255, 255, 5],
            ),
            (
                vec![1, 2, 3, 0],
                vec![1, 2, 3, 0, 0, 0, 0, 0, 251],
                vec![254, 253, 252, 255, 255, 255, 255, 255, 4],
            ),
            (
                vec![1, 2, 3, 4, 5, 6, 7],
                vec![1, 2, 3, 4, 5, 6, 7, 0, 254],
                vec![254, 253, 252, 251, 250, 249, 248, 255, 1],
            ),
            (
                vec![0, 0, 0, 0, 0, 0, 0, 0],
                vec![0, 0, 0, 0, 0, 0, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0, 0, 247],
                vec![
                    255, 255, 255, 255, 255, 255, 255, 255, 0, 255, 255, 255, 255, 255, 255, 255,
                    255, 8,
                ],
            ),
            (
                vec![1, 2, 3, 4, 5, 6, 7, 8],
                vec![1, 2, 3, 4, 5, 6, 7, 8, 255, 0, 0, 0, 0, 0, 0, 0, 0, 247],
                vec![
                    254, 253, 252, 251, 250, 249, 248, 247, 0, 255, 255, 255, 255, 255, 255, 255,
                    255, 8,
                ],
            ),
            (
                vec![1, 2, 3, 4, 5, 6, 7, 8, 9],
                vec![1, 2, 3, 4, 5, 6, 7, 8, 255, 9, 0, 0, 0, 0, 0, 0, 0, 248],
                vec![
                    254, 253, 252, 251, 250, 249, 248, 247, 0, 246, 255, 255, 255, 255, 255, 255,
                    255, 7,
                ],
            ),
        ];

        for (source, mut asc, mut desc) in pairs {
            assert_eq!(encode_bytes(&source), asc);
            assert_eq!(encode_bytes_desc(&source), desc);
            decode_bytes_in_place(&mut asc, false).unwrap();
            assert_eq!(source, asc);
            decode_bytes_in_place(&mut desc, true).unwrap();
            assert_eq!(source, desc);
        }
    }
}
