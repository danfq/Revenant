# Revenant

Recover deleted, hidden and permanently-purged photos from an **encrypted iOS
backup**, locally and privately. Revenant decrypts the backup in place (read
only), then digs out the imagery and metadata that survive after a user deletes
a photo: cached thumbnails, orphaned thumbnails of purged assets, and deleted
database records carved from free pages.

It is a single self-contained Rust binary. Nothing is uploaded anywhere: every
step runs on your machine and writes only inside a workspace directory you
choose.

## What it can recover

An iOS photo library is described by `Photos.sqlite` and backed by originals,
cached renders and thumbnails. Revenant targets the parts that outlive a
deletion:

- **Recently Deleted** assets (still in the 30 day window): database rows and thumbnails
- **Hidden album** assets: database rows and thumbnails
- **Permanently purged** assets (emptied from Recently Deleted): the database row
  is gone, but its **thumbnail often lingers** in the cache as an orphan, and its
  **record can frequently be carved** back from free pages in `Photos.sqlite`
- **Present originals**: full-resolution files that are physically in the backup

### The iCloud caveat

If the device uses iCloud Photos with "Optimize iPhone Storage", most
full-resolution originals live in iCloud, not in the backup. For those assets
Revenant recovers thumbnails and metadata, not the originals. To recover
originals too, first download them to the device ("Download and Keep
Originals"), make a fresh encrypted backup, then point Revenant at it.

Note that nothing can recover media Apple has already purged server-side
(permanently deleted and past the 30 day window). In that case the local backup
artifacts Revenant extracts are the only remaining trace.

## Requirements

- Rust (stable) to build
- The `sqlite3` command line tool on `PATH` (used for record carving)
- An **encrypted** iOS backup, and its backup password

The backup password is the standalone password set when "Encrypt local backup"
was first enabled in Finder or iTunes. It is not the device passcode and not the
Apple ID password. On the Mac that made the backup it is usually saved in the
login Keychain (item "iOS Backup").

## Build

```bash
cargo build --release
# binary at target/release/revenant
```

## Usage

Provide the backup directory with `--backup`. The password is read from the
`IOS_BACKUP_PASSWORD` environment variable, or prompted for without echo.

```bash
export REV=./target/release/revenant
BACKUP=/path/to/backup/<device-udid>

# private password entry, kept out of shell history:
read -rs IOS_BACKUP_PASSWORD; export IOS_BACKUP_PASSWORD

# run the whole pipeline
$REV --backup "$BACKUP" all

# or step by step
$REV --backup "$BACKUP" decrypt      # decrypt Manifest.db + Photos.sqlite
$REV --backup "$BACKUP" carve        # carve deleted db rows -> recovered/deleted_assets.csv
$REV --backup "$BACKUP" thumbnails   # recover orphan + hidden + recently-deleted thumbnails
$REV --backup "$BACKUP" media        # decrypt originals that are present

unset IOS_BACKUP_PASSWORD
```

### Steps

| Step         | What it does                                                                                                                               |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------ |
| `decrypt`    | Decrypts `Manifest.db` and `Photos.sqlite` into the workspace. Required first.                                                             |
| `carve`      | Runs `sqlite3 .recover`, then reports rows present in free pages but absent from the live library (deleted assets).                        |
| `thumbnails` | Matches thumbnails to the live library, then recovers the unmatched (orphan) ones plus the current Hidden and Recently Deleted thumbnails. |
| `media`      | Decrypts the original media that is physically present in the backup.                                                                      |
| `all`        | The four steps above, in order.                                                                                                            |

### Flags (place before the step name)

| Flag                 | Affects      | Effect                                                  |
| -------------------- | ------------ | ------------------------------------------------------- |
| `--backup <dir>`     | All          | Path to the backup directory. Required.                 |
| `--work <dir>`       | All          | Workspace directory (default `./workspace`).            |
| `-c`, `--count-only` | All          | Report totals only, write nothing under `recovered/`.   |
| `--force`            | `thumbnails` | Export orphans even if the match rate looks unreliable. |
| `--include-all`      | `media`      | Also decrypt the CPL cloud-staged assets.               |

```bash
$REV --backup "$BACKUP" -c all          # preview totals, write nothing
$REV --backup "$BACKUP" --force thumbnails
$REV --backup "$BACKUP" --include-all media
```

The `thumbnails` step self-calibrates: it measures how well it can match
thumbnails to the live library and refuses to dump everything as "orphans" if
matching looks unreliable (more than 20 percent unmatched). Inspect the paths,
then re-run with `--force` once you are satisfied.

## Output

Everything lands under `<work>/`:

```
workspace/
  Manifest.decrypted.db                 decrypted file index
  Photos.sqlite                         decrypted photo library
  recovered.db                          carved copy produced by sqlite3 .recover
  recovered/
    deleted_assets.csv                  metadata of rows found only in free pages
    orphan_thumbnails/                  thumbnails with no live asset = purged media
    hidden_thumbnails/                  thumbnails of the current Hidden album
    recently_deleted_thumbnails/        thumbnails of the current Recently Deleted album
    media/
      originals_DCIM/                   full-res originals present in the backup
      cached_renders_UBF/               cached full-size JPEG renders
      edit_mutations/                   edit and adjustment data
```

Browse `orphan_thumbnails/` in a gallery view to eyeball recovered deleted
images.

## Privacy and safety

- The backup is opened read only and is never modified.
- Nothing is sent off the machine.
- The password is taken from `IOS_BACKUP_PASSWORD` or an unechoed prompt, never a
  command line argument, so it does not land in `ps` output or shell history.
- The workspace holds decrypted copies of private data. Remove it when done:
  `rm -rf workspace`.

## How it works

- Parses the backup keybag from `Manifest.plist`, derives the key with PBKDF2
  (SHA-256 then SHA-1) from the password, and unwraps the per protection class
  keys with AES Key Unwrap (RFC 3394).
- Decrypts `Manifest.db` and payload files with AES-256-CBC and a zero IV, using
  the per file key stored in each file's NSKeyedArchiver metadata.
- Reads the decrypted SQLite databases to classify assets and locate thumbnails.

## Legal

Use Revenant only on backups of devices and accounts you own or are authorized
to examine. You are responsible for how you use it.

## License

GNU Affero General Public License v3.0 (AGPLv3)
