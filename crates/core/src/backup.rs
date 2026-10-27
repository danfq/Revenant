//! read-side helpers for an encrypted iOS backup
//!
//! loads the keybag, decrypts Manifest.db and decrypts individual payload
//! files given their Manifest.db metadata blob

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use plist::Value;

use crate::crypto;
use crate::keybag::Keybag;

const ZERO_IV: [u8; 16] = [0u8; 16];

/// load Manifest.plist and parse its keybag, unlocked with the password
pub fn load_keybag(backup: &Path, password: &str) -> Result<(Keybag, Value, usize)> {
    let plist_path = backup.join("Manifest.plist");
    let manifest = Value::from_file(&plist_path)
        .with_context(|| format!("reading {}", plist_path.display()))?;
    let dict = manifest
        .as_dictionary()
        .ok_or_else(|| anyhow!("Manifest.plist is not a dictionary"))?;
    let bag = dict
        .get("BackupKeyBag")
        .and_then(Value::as_data)
        .ok_or_else(|| anyhow!("Manifest.plist has no BackupKeyBag (is the backup encrypted?)"))?;
    let mut kb = Keybag::parse(bag);
    let n = kb.unlock(password.as_bytes());
    Ok((kb, manifest, n))
}

/// decrypt Manifest.db into out_path
///
/// returns (looks_like_sqlite, class_keys_unlocked)
pub fn decrypt_manifest_db(backup: &Path, password: &str, out_path: &Path) -> Result<(bool, usize)> {
    let (kb, manifest, n) = load_keybag(backup, password)?;
    let dict = manifest.as_dictionary().unwrap();
    let mkey = dict
        .get("ManifestKey")
        .and_then(Value::as_data)
        .ok_or_else(|| anyhow!("Manifest.plist has no ManifestKey"))?;

    // the manifest key class prefix is little endian, the rest is the wrapped key
    let clas = u32::from_le_bytes(mkey[0..4].try_into()?);
    let wrapped = &mkey[4..];
    let key = kb
        .unwrap_for_class(clas, wrapped)
        .ok_or_else(|| anyhow!("could not unwrap the manifest key (wrong password?)"))?;

    let data = fs::read(backup.join("Manifest.db")).context("reading Manifest.db")?;
    let dec = crypto::aes256_cbc_decrypt(&key, &ZERO_IV, &data);
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(out_path, &dec)?;
    let ok = dec.len() >= 15 && &dec[..15] == b"SQLite format 3";
    Ok((ok, n))
}

/// resolve a value that may be an NSKeyedArchiver uid reference
fn resolve<'a>(objects: &'a [Value], v: Option<&'a Value>) -> Option<&'a Value> {
    match v {
        Some(Value::Uid(uid)) => objects.get(uid.get() as usize),
        other => other,
    }
}

fn as_u64(v: Option<&Value>) -> u64 {
    match v {
        Some(Value::Integer(i)) => i.as_unsigned().unwrap_or(0),
        _ => 0,
    }
}

/// (protection_class, wrapped_key, plaintext_size) parsed from a Files.file blob
pub fn file_crypto_params(blob: &[u8]) -> Option<(u32, Vec<u8>, usize)> {
    let v = Value::from_reader(std::io::Cursor::new(blob)).ok()?;
    let top = v.as_dictionary()?;
    let objects = top.get("$objects")?.as_array()?;
    let root_uid = top.get("$top")?.as_dictionary()?.get("root")?.as_uid()?;
    let root = objects.get(root_uid.get() as usize)?.as_dictionary()?;

    if !root.contains_key("EncryptionKey") {
        return None;
    }
    let size = as_u64(resolve(objects, root.get("Size"))) as usize;
    let pclass = as_u64(resolve(objects, root.get("ProtectionClass"))) as u32;
    let enc = resolve(objects, root.get("EncryptionKey"))?.as_dictionary()?;
    let data = enc.get("NS.data")?.as_data()?;
    if data.len() <= 4 {
        return None;
    }
    Some((pclass, data[4..].to_vec(), size))
}

/// locate a payload file inside the backup from its sha1 file id
pub fn payload_path(backup: &Path, file_id: &str) -> PathBuf {
    backup.join(&file_id[..2]).join(file_id)
}

/// decrypt a single backup payload file into out_path
///
/// returns false when the file is not encrypted or its key cannot be unwrapped
pub fn decrypt_backup_file(
    backup: &Path,
    kb: &Keybag,
    file_id: &str,
    blob: &[u8],
    out_path: &Path,
) -> Result<bool> {
    let (pclass, wrapped, size) = match file_crypto_params(blob) {
        Some(p) => p,
        None => return Ok(false),
    };
    let key = match kb.unwrap_for_class(pclass, &wrapped) {
        Some(k) => k,
        None => return Ok(false),
    };
    let data = fs::read(payload_path(backup, file_id))?;
    let mut dec = crypto::aes256_cbc_decrypt(&key, &ZERO_IV, &data);
    if size <= dec.len() {
        dec.truncate(size);
    }
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(out_path, &dec)?;
    Ok(true)
}
