//! the four recovery steps: decrypt, carve, thumbnails, media
//!
//! every step opens the backup read only and writes only inside the workspace

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use anyhow::{anyhow, Context, Result};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};

use crate::backup;

const CORE_DATA_EPOCH: i64 = 978_307_200; // seconds between 1970 and 2001
const PHOTOS_DB: &str = "Media/PhotoData/Photos.sqlite";
const THUMB_PREFIX: &str = "Media/PhotoData/Thumbnails/";

/// shared run context from the cli
pub struct Ctx {
    pub backup: PathBuf,
    pub work: PathBuf,
    pub count_only: bool,
    pub force: bool,
    pub include_all: bool,
}

static PASSWORD: OnceLock<String> = OnceLock::new();

/// read the backup password once, from env or an unechoed prompt, then cache it
fn password() -> Result<String> {
    if let Some(p) = PASSWORD.get() {
        return Ok(p.clone());
    }
    let p = match std::env::var("IOS_BACKUP_PASSWORD") {
        Ok(v) if !v.is_empty() => v,
        _ => rpassword::prompt_password("backup password (not echoed): ")?,
    };
    let _ = PASSWORD.set(p.clone());
    Ok(p)
}

fn manifest_path(ctx: &Ctx) -> PathBuf {
    ctx.work.join("Manifest.decrypted.db")
}

fn photos_path(ctx: &Ctx) -> PathBuf {
    ctx.work.join("Photos.sqlite")
}

fn open_ro(path: &Path) -> Result<Connection> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("opening {}", path.display()))
}

fn table_columns(conn: &Connection, table: &str) -> Vec<String> {
    let sql = format!("PRAGMA table_info({table})");
    let mut out = Vec::new();
    if let Ok(mut stmt) = conn.prepare(&sql) {
        if let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(1)) {
            for r in rows.flatten() {
                out.push(r);
            }
        }
    }
    out
}

fn cell_to_string(v: ValueRef) -> String {
    match v {
        ValueRef::Null => String::new(),
        ValueRef::Integer(i) => i.to_string(),
        ValueRef::Real(f) => f.to_string(),
        ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned(),
        ValueRef::Blob(_) => "<blob>".to_string(),
    }
}

/// format core data seconds as a utc iso-ish timestamp
fn iso_from_core_data(core: f64) -> String {
    if core == 0.0 {
        return String::new();
    }
    let secs = core as i64 + CORE_DATA_EPOCH;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    // civil date from a unix day count (hinnant)
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    if m <= 2 {
        y += 1;
    }
    format!("{y:04}-{m:02}-{d:02} {h:02}:{mi:02}:{s:02}")
}

// ----------------------------------------------------------------------- decrypt

pub fn decrypt(ctx: &Ctx) -> Result<()> {
    std::fs::create_dir_all(&ctx.work)?;
    let pw = password()?;
    let (ok, n) = backup::decrypt_manifest_db(&ctx.backup, &pw, &manifest_path(ctx))?;
    if !ok {
        return Err(anyhow!("wrong password or Manifest.db did not decrypt to sqlite"));
    }
    println!("[ok] Manifest.db decrypted  ({n} class keys unlocked)");

    let (kb, _, _) = backup::load_keybag(&ctx.backup, &pw)?;
    let man = open_ro(&manifest_path(ctx))?;
    for rp in [
        PHOTOS_DB,
        &format!("{PHOTOS_DB}-wal"),
        &format!("{PHOTOS_DB}-shm"),
    ] {
        let mut stmt = man.prepare(
            "SELECT fileID, file FROM Files \
             WHERE domain='CameraRollDomain' AND relativePath=?1",
        )?;
        let row = stmt
            .query_row([rp], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?)))
            .ok();
        if let Some((fid, blob)) = row {
            let name = Path::new(rp).file_name().unwrap();
            let out = ctx.work.join(name);
            if backup::decrypt_backup_file(&ctx.backup, &kb, &fid, &blob, &out)? {
                let mb = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0) as f64 / 1e6;
                println!("[ok] decrypted {}  ({mb:.1} MB)", name.to_string_lossy());
            }
        }
    }
    if ctx.work.join("Photos.sqlite-wal").exists() {
        println!("[note] a -wal file is present; open Photos.sqlite once read-write to checkpoint");
    }
    println!("[done] decrypt");
    Ok(())
}

// ------------------------------------------------------------------------- carve

pub fn carve(ctx: &Ctx) -> Result<()> {
    let src = photos_path(ctx);
    if !src.exists() {
        return Err(anyhow!("run 'decrypt' first"));
    }
    let rec = ctx.work.join("recovered.db");
    if rec.exists() {
        std::fs::remove_file(&rec)?;
    }

    println!("[..] running sqlite3 .recover (can take a minute on a large library)");
    let dump = Command::new("sqlite3")
        .arg(&src)
        .arg(".recover")
        .output()
        .context("spawning sqlite3 (is the cli installed?)")?;
    if !dump.status.success() {
        return Err(anyhow!("sqlite3 .recover failed"));
    }
    let mut child = Command::new("sqlite3")
        .arg(&rec)
        .stdin(Stdio::piped())
        .spawn()
        .context("spawning sqlite3 to load the recovered dump")?;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&dump.stdout)
        .context("feeding the recovered dump")?;
    child.wait()?;
    if !rec.exists() {
        return Err(anyhow!(".recover produced no database"));
    }

    let live = open_ro(&src)?;
    let recdb = open_ro(&rec)?;

    let mut live_uuids: HashSet<String> = HashSet::new();
    {
        let mut stmt = live.prepare("SELECT ZUUID FROM ZASSET")?;
        for u in stmt
            .query_map([], |r| r.get::<_, Option<String>>(0))?
            .flatten()
            .flatten()
        {
            live_uuids.insert(u);
        }
    }

    let cols = table_columns(&recdb, "ZASSET");
    if !cols.iter().any(|c| c == "ZUUID") {
        println!("[warn] recovered db has no ZASSET.ZUUID, nothing to diff");
        return Ok(());
    }
    let wanted: Vec<&str> = [
        "ZUUID",
        "ZFILENAME",
        "ZDIRECTORY",
        "ZDATECREATED",
        "ZTRASHEDDATE",
        "ZHIDDEN",
        "ZKIND",
        "ZCLOUDASSETGUID",
    ]
    .into_iter()
    .filter(|c| cols.iter().any(|x| x == c))
    .collect();
    let sel = wanted.join(", ");
    let uuid_idx = wanted.iter().position(|c| *c == "ZUUID").unwrap();
    let created_idx = wanted.iter().position(|c| *c == "ZDATECREATED");
    let trashed_idx = wanted.iter().position(|c| *c == "ZTRASHEDDATE");

    let mut stmt = recdb.prepare(&format!("SELECT {sel} FROM ZASSET"))?;
    let mut deleted_rows: Vec<Vec<String>> = Vec::new();
    let mut extra: Vec<(String, String)> = Vec::new();
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let uuid: Option<String> = row.get(uuid_idx)?;
        let uuid = match uuid {
            Some(u) if !live_uuids.contains(&u) => u,
            _ => continue,
        };
        let _ = uuid;
        let mut cells = Vec::with_capacity(wanted.len());
        for i in 0..wanted.len() {
            cells.push(cell_to_string(row.get_ref(i)?));
        }
        let created = created_idx
            .map(|i| iso_from_core_data(row.get::<_, f64>(i).unwrap_or(0.0)))
            .unwrap_or_default();
        let trashed = trashed_idx
            .map(|i| iso_from_core_data(row.get::<_, f64>(i).unwrap_or(0.0)))
            .unwrap_or_default();
        deleted_rows.push(cells);
        extra.push((created, trashed));
    }

    println!(
        "[ok] deleted ZASSET rows recovered from free pages: {}",
        deleted_rows.len()
    );

    if ctx.count_only {
        println!("[count-only] not writing deleted_assets.csv");
        println!("[done] carve");
        return Ok(());
    }

    let out_dir = ctx.work.join("recovered");
    std::fs::create_dir_all(&out_dir)?;
    let out_csv = out_dir.join("deleted_assets.csv");
    let mut w = csv::Writer::from_path(&out_csv)?;
    let mut header: Vec<String> = wanted.iter().map(|s| s.to_string()).collect();
    header.push("created_iso".to_string());
    header.push("trashed_iso".to_string());
    w.write_record(&header)?;
    for (cells, (created, trashed)) in deleted_rows.iter().zip(extra.iter()) {
        let mut rec_row = cells.clone();
        rec_row.push(created.clone());
        rec_row.push(trashed.clone());
        w.write_record(&rec_row)?;
    }
    w.flush()?;
    println!(
        "     written to {} (open it locally; it contains filenames + dates)",
        out_csv.display()
    );
    println!("[done] carve");
    Ok(())
}

// -------------------------------------------------------------------- thumbnails

fn is_uuid(seg: &str) -> bool {
    let parts: Vec<&str> = seg.split('-').collect();
    if parts.len() != 5 {
        return false;
    }
    let lens = [8usize, 4, 4, 4, 12];
    for (p, l) in parts.iter().zip(lens.iter()) {
        if p.len() != *l || !p.chars().all(|c| c.is_ascii_hexdigit()) {
            return false;
        }
    }
    true
}

fn looks_like_image_name(stem: &str) -> bool {
    let up = stem.to_uppercase();
    ["IMG_", "FULLSIZERENDER", "DSC", "PXL_", "RPREPLAY", "VIDEO_"]
        .iter()
        .any(|p| up.starts_with(p))
}

/// derive the (uuid, (dirfolder, filestem)) keys a thumbnail path may carry
fn derive_keys(rp: &str) -> (Option<String>, Option<(String, String)>) {
    let segs: Vec<&str> = rp.split('/').collect();
    let mut uuid_key = None;
    for s in &segs {
        if is_uuid(s) {
            uuid_key = Some(s.to_uppercase());
            break;
        }
    }
    let mut path_key = None;
    for i in 0..segs.len().saturating_sub(1) {
        let a = segs[i];
        let b = segs[i + 1];
        let stem = b.rsplit_once('.').map(|(s, _)| s).unwrap_or(b);
        let apple = a.len() == 8 && a.ends_with("APPLE") && a[..3].chars().all(|c| c.is_ascii_digit());
        if looks_like_image_name(stem) || apple {
            path_key = Some((a.to_uppercase(), stem.to_uppercase()));
        }
    }
    (uuid_key, path_key)
}

pub fn thumbnails(ctx: &Ctx) -> Result<()> {
    if !manifest_path(ctx).exists() || !photos_path(ctx).exists() {
        return Err(anyhow!("run 'decrypt' first"));
    }
    let man = open_ro(&manifest_path(ctx))?;
    let ps = open_ro(&photos_path(ctx))?;

    let cols = table_columns(&ps, "ZASSET");
    let has = |c: &str| cols.iter().any(|x| x == c);
    let dir_col = if has("ZDIRECTORY") { "ZDIRECTORY" } else { "NULL" };
    let fn_col = if has("ZFILENAME") { "ZFILENAME" } else { "NULL" };

    let mut live_uuid: HashSet<String> = HashSet::new();
    let mut live_pathkey: HashSet<(String, String)> = HashSet::new();
    let mut hidden_uuid: HashSet<String> = HashSet::new();
    let mut trashed_uuid: HashSet<String> = HashSet::new();
    {
        let sql = format!(
            "SELECT ZUUID, {dir_col}, {fn_col}, ZHIDDEN, ZTRASHEDSTATE FROM ZASSET"
        );
        let mut stmt = ps.prepare(&sql)?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let uuid: Option<String> = row.get(0)?;
            let direc: Option<String> = row.get(1)?;
            let fname: Option<String> = row.get(2)?;
            let hidden: i64 = row.get(3).unwrap_or(0);
            let trashed: i64 = row.get(4).unwrap_or(0);
            if let Some(u) = &uuid {
                let up = u.to_uppercase();
                live_uuid.insert(up.clone());
                if hidden != 0 {
                    hidden_uuid.insert(up.clone());
                }
                if trashed != 0 {
                    trashed_uuid.insert(up);
                }
            }
            if let (Some(d), Some(f)) = (direc, fname) {
                let folder = d.rsplit('/').next().unwrap_or(&d).to_uppercase();
                let stem = f.rsplit_once('.').map(|(s, _)| s).unwrap_or(&f).to_uppercase();
                live_pathkey.insert((folder, stem));
            }
        }
    }

    let thumbs: Vec<(String, String, Vec<u8>)> = {
        let mut stmt = man.prepare(
            "SELECT fileID, relativePath, file FROM Files \
             WHERE domain='CameraRollDomain' AND relativePath LIKE ?1 AND flags=1",
        )?;
        let like = format!("{THUMB_PREFIX}%");
        let collected: Vec<(String, String, Vec<u8>)> = stmt
            .query_map([like], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Vec<u8>>(2)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();
        collected
    };

    // self-calibrate: which keying matches the live library better
    let mut uuid_hit = 0;
    let mut path_hit = 0;
    for (_, rp, _) in &thumbs {
        let (uk, pk) = derive_keys(rp);
        if uk.as_ref().map(|k| live_uuid.contains(k)).unwrap_or(false) {
            uuid_hit += 1;
        }
        if pk.as_ref().map(|k| live_pathkey.contains(k)).unwrap_or(false) {
            path_hit += 1;
        }
    }
    let total = thumbs.len();
    let use_uuid = uuid_hit >= path_hit;
    println!(
        "[info] thumbnails={total}  uuid-matches={uuid_hit}  path-matches={path_hit}  -> using {} keying",
        if use_uuid { "uuid" } else { "path" }
    );

    let is_orphan = |rp: &str| -> bool {
        let (uk, pk) = derive_keys(rp);
        let hit = if use_uuid {
            uk.as_ref().map(|k| live_uuid.contains(k)).unwrap_or(false)
        } else {
            pk.as_ref().map(|k| live_pathkey.contains(k)).unwrap_or(false)
        };
        !hit
    };

    let orphans: Vec<&(String, String, Vec<u8>)> =
        thumbs.iter().filter(|(_, rp, _)| is_orphan(rp)).collect();
    let frac = if total > 0 {
        orphans.len() as f64 / total as f64
    } else {
        0.0
    };

    let c_hidden = thumbs
        .iter()
        .filter(|(_, rp, _)| {
            derive_keys(rp)
                .0
                .map(|k| hidden_uuid.contains(&k))
                .unwrap_or(false)
        })
        .count();
    let c_trash = thumbs
        .iter()
        .filter(|(_, rp, _)| {
            derive_keys(rp)
                .0
                .map(|k| trashed_uuid.contains(&k))
                .unwrap_or(false)
        })
        .count();
    println!(
        "[info] orphan={}  hidden-album={c_hidden}  recently-deleted={c_trash} thumbnails",
        orphans.len()
    );

    if frac > 0.20 && !ctx.force {
        println!("[stop] more than 20% unmatched: keying probably did not resolve cleanly.");
        println!("       inspect a few paths yourself, then re-run as:  revenant --force thumbnails");
        println!("       (--force goes before the step name, like -c)");
        return Ok(());
    }

    if ctx.count_only {
        println!("[count-only] not exporting any images");
        println!("[note] orphan set is the best evidence of permanently purged media");
        println!("[done] thumbnails");
        return Ok(());
    }

    let (kb, _, _) = backup::load_keybag(&ctx.backup, &password()?)?;
    let base = ctx.work.join("recovered");
    let out_orphan = base.join("orphan_thumbnails");
    let out_hidden = base.join("hidden_thumbnails");
    let out_trash = base.join("recently_deleted_thumbnails");

    let export = |fid: &str, rp: &str, blob: &[u8], folder: &Path| -> bool {
        let leaf = rp.rsplit('/').next().unwrap_or(rp);
        let name = format!("{fid}__{leaf}");
        backup::decrypt_backup_file(&ctx.backup, &kb, fid, blob, &folder.join(name)).unwrap_or(false)
    };

    let mut n_orphan = 0;
    for (fid, rp, blob) in &orphans {
        if export(fid, rp, blob, &out_orphan) {
            n_orphan += 1;
        }
    }
    let mut n_hidden = 0;
    let mut n_trash = 0;
    for (fid, rp, blob) in &thumbs {
        let uk = derive_keys(rp).0;
        if let Some(k) = &uk {
            if hidden_uuid.contains(k) && export(fid, rp, blob, &out_hidden) {
                n_hidden += 1;
            }
            if trashed_uuid.contains(k) && export(fid, rp, blob, &out_trash) {
                n_trash += 1;
            }
        }
    }

    println!("[ok] orphan thumbnails recovered       : {n_orphan} -> {}", out_orphan.display());
    println!("[ok] hidden-album thumbnails exported   : {n_hidden} -> {}", out_hidden.display());
    println!("[ok] recently-deleted thumbnails export : {n_trash} -> {}", out_trash.display());
    println!("[note] orphan set is the best evidence of permanently purged media");
    println!("[done] thumbnails");
    Ok(())
}

// ------------------------------------------------------------------------- media

pub fn media(ctx: &Ctx) -> Result<()> {
    if !manifest_path(ctx).exists() {
        return Err(anyhow!("run 'decrypt' first"));
    }
    let man = open_ro(&manifest_path(ctx))?;

    let mut groups: Vec<(&str, &str)> = vec![
        ("originals_DCIM", "Media/DCIM/%"),
        ("cached_renders_UBF", "Media/PhotoData/UBF/%"),
        ("edit_mutations", "Media/PhotoData/Mutations/%"),
    ];
    if ctx.include_all {
        groups.push(("cpl_assets", "Media/PhotoData/CPLAssets/%"));
    }

    let base = ctx.work.join("recovered").join("media");
    let kb = if ctx.count_only {
        None
    } else {
        Some(backup::load_keybag(&ctx.backup, &password()?)?.0)
    };

    for (label, like) in groups {
        let rows: Vec<(String, String, Vec<u8>)> = {
            let mut stmt = man.prepare(
                "SELECT fileID, relativePath, file FROM Files \
                 WHERE domain='CameraRollDomain' AND relativePath LIKE ?1 AND flags=1",
            )?;
            let collected: Vec<(String, String, Vec<u8>)> = stmt
                .query_map([like], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Vec<u8>>(2)?,
                    ))
                })?
                .filter_map(|r| r.ok())
                .collect();
            collected
        };

        if ctx.count_only {
            println!("[count] {label:<20} {} files present", rows.len());
            continue;
        }

        let kb = kb.as_ref().unwrap();
        let mut ok = 0;
        for (fid, rp, blob) in &rows {
            let leaf = rp.rsplit('/').next().unwrap_or(rp);
            let name = if leaf.is_empty() { fid.as_str() } else { leaf };
            let out = base.join(label).join(name);
            if backup::decrypt_backup_file(&ctx.backup, kb, fid, blob, &out)? {
                ok += 1;
            }
        }
        println!("[ok] {label:<20} {ok}/{} files -> {}", rows.len(), base.join(label).display());
    }
    println!("[done] media");
    Ok(())
}
