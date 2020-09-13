use crate::errors::Result;
use std::io::{Write};

const ENC_GROUP_SIZE: usize = 8;
const ENC_MARKER: u8 = b'\xff';
const ENC_ASC_PADDING: [u8; ENC_GROUP_SIZE] = [0; ENC_GROUP_SIZE];
const ENC_DESC_PADDING: [u8; ENC_GROUP_SIZE] = [!0; ENC_GROUP_SIZE];

/// Returns the maximum encoded bytes size.
pub fn max_encoded_bytes_size(n: usize) -> usize {
    (n / ENC_GROUP_SIZE + 1) * (ENC_GROUP_SIZE + 1)
}

pub trait BytesEncoder : Write {
    /// Refer: https://github.com/facebook/mysql-5.6/wiki/MyRocks-record-format#memcomparable-format
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

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::codec::{bytes, number};
    use std::cmp::Ordering;

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
        }
    }
}
