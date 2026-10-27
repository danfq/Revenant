//! backup keybag parsing and unlock
//!
//! the keybag is a tlv blob inside Manifest.plist. unlocking derives a key
//! from the backup password and unwraps the per protection class keys

use std::collections::HashMap;

use crate::crypto;

const WRAP_PASSCODE: u32 = 2;

#[derive(Default)]
struct ClassKey {
    clas: u32,
    wrap: u32,
    wpky: Vec<u8>,
    key: Option<Vec<u8>>,
}

/// a parsed, optionally unlocked backup keybag
pub struct Keybag {
    salt: Vec<u8>,
    iter: u32,
    dpsl: Option<Vec<u8>>,
    dpic: Option<u32>,
    class_keys: HashMap<u32, ClassKey>,
}

fn be_u32(v: &[u8]) -> u32 {
    let mut b = [0u8; 4];
    b.copy_from_slice(&v[..4]);
    u32::from_be_bytes(b)
}

impl Keybag {
    /// parse a raw keybag tlv blob
    pub fn parse(data: &[u8]) -> Self {
        let mut kb = Keybag {
            salt: Vec::new(),
            iter: 0,
            dpsl: None,
            dpic: None,
            class_keys: HashMap::new(),
        };
        let mut cur: Option<ClassKey> = None;
        let mut saw_top_uuid = false;
        let mut saw_top_wrap = false;

        let mut i = 0usize;
        while i + 8 <= data.len() {
            let tag = &data[i..i + 4];
            let len = be_u32(&data[i + 4..i + 8]) as usize;
            if i + 8 + len > data.len() {
                break;
            }
            let val = &data[i + 8..i + 8 + len];
            match tag {
                b"TYPE" => {}
                b"UUID" if !saw_top_uuid => saw_top_uuid = true,
                b"WRAP" if !saw_top_wrap => saw_top_wrap = true,
                b"UUID" => {
                    if let Some(c) = cur.take() {
                        kb.class_keys.insert(c.clas, c);
                    }
                    cur = Some(ClassKey::default());
                }
                b"CLAS" => {
                    if let Some(c) = cur.as_mut() {
                        c.clas = be_u32(val);
                    }
                }
                b"WRAP" => {
                    if let Some(c) = cur.as_mut() {
                        c.wrap = be_u32(val);
                    }
                }
                b"WPKY" => {
                    if let Some(c) = cur.as_mut() {
                        c.wpky = val.to_vec();
                    }
                }
                b"SALT" => kb.salt = val.to_vec(),
                b"ITER" => kb.iter = be_u32(val),
                b"DPSL" => kb.dpsl = Some(val.to_vec()),
                b"DPIC" => kb.dpic = Some(be_u32(val)),
                _ => {}
            }
            i += 8 + len;
        }
        if let Some(c) = cur.take() {
            kb.class_keys.insert(c.clas, c);
        }
        kb
    }

    /// derive keys from the backup password and unwrap the class keys
    ///
    /// returns how many class keys were unlocked (zero means wrong password)
    pub fn unlock(&mut self, passcode: &[u8]) -> usize {
        let derived = match (&self.dpsl, self.dpic) {
            (Some(dpsl), Some(dpic)) => crypto::pbkdf2_sha256(passcode, dpsl, dpic, 32),
            _ => passcode.to_vec(),
        };
        let key = crypto::pbkdf2_sha1(&derived, &self.salt, self.iter, 32);

        let mut unlocked = 0;
        for ck in self.class_keys.values_mut() {
            if ck.wrap & WRAP_PASSCODE != 0 && !ck.wpky.is_empty() {
                if let Some(k) = crypto::aes256_unwrap(&key, &ck.wpky) {
                    ck.key = Some(k);
                    unlocked += 1;
                }
            }
        }
        unlocked
    }

    /// unwrap a wrapped key using the unlocked key for its protection class
    pub fn unwrap_for_class(&self, protection_class: u32, wrapped: &[u8]) -> Option<Vec<u8>> {
        let ck = self.class_keys.get(&protection_class)?;
        let class_key = ck.key.as_ref()?;
        crypto::aes256_unwrap(class_key, wrapped)
    }
}
