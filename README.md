# revenant

recover deleted, hidden and permanently-purged photos from an **encrypted iOS
backup**, locally and privately. revenant decrypts the backup in place (read
only), then digs out the imagery and metadata that survive after a user deletes
a photo: cached thumbnails, orphaned thumbnails of purged assets, and deleted
database records carved from free pages.

it is a single self-contained Rust binary. nothing is uploaded anywhere: every
step runs on your machine and writes only inside a workspace directory you
choose.

## what it can recover

An iOS photo library is described by `Photos.sqlite` and backed by originals,
cached renders and thumbnails. revenant targets the parts that outlive a
deletion:

- **recently deleted** assets (still in the 30 day window): db rows + thumbnails
- **hidden album** assets: db rows + thumbnails
- **permanently purged** assets (emptied from recently deleted): the db row is
  gone, but its **thumbnail often lingers** in the cache as an orphan, and its
  **record can frequently be carved** back from free pages in `Photos.sqlite`
- **present originals**: full-resolution files that are physically in the backup

### the iCloud caveat

If the device uses iCloud Photos with "Optimize iPhone Storage", most
full-resolution originals live in iCloud, not in the backup. For those assets
revenant recovers thumbnails and metadata, not the originals. To recover
originals too, first download them to the device ("Download and Keep
Originals"), make a fresh encrypted backup, then point revenant at it.

Note: nothing can recover media that Apple has already purged server-side
(permanently deleted and past the 30 day window). In that case the local
backup artifacts revenant extracts are the only remaining trace.

## requirements

- Rust (stable) to build
- the `sqlite3` command line tool on `PATH` (used for record carving)
- an **encrypted** iOS backup, and its backup password

The backup password is the standalone password set when "Encrypt local backup"
was first enabled in Finder or iTunes. It is not the device passcode and not the
Apple ID password. On the Mac that made the backup it is usually saved in the
login Keychain (item "iOS Backup").

## build

```bash
cargo build --release
# binary at target/release/revenant
```

## usage

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

### steps

| step | what it does |
|------|--------------|
| `decrypt` | decrypts `Manifest.db` and `Photos.sqlite` into the workspace. required first |
| `carve` | runs `sqlite3 .recover`, then reports rows present in free pages but absent from the live library (deleted assets) |
| `thumbnails` | matches thumbnails to the live library, then recovers the unmatched (orphan) ones plus the current hidden and recently-deleted thumbnails |
| `media` | decrypts the original media that is physically present in the backup |
| `all` | the four steps above, in order |

### flags (place before the step name)

| flag | affects | effect |
|------|---------|--------|
| `--backup <dir>` | all | path to the backup directory. required |
| `--work <dir>` | all | workspace directory (default `./workspace`) |
| `-c`, `--count-only` | all | report totals only, write nothing under `recovered/` |
| `--force` | `thumbnails` | export orphans even if the match rate looks unreliable |
| `--include-all` | `media` | also decrypt the CPL cloud-staged assets |

```bash
$REV --backup "$BACKUP" -c all          # preview totals, write nothing
$REV --backup "$BACKUP" --force thumbnails
$REV --backup "$BACKUP" --include-all media
```

The `thumbnails` step self-calibrates: it measures how well it can match
thumbnails to the live library and refuses to dump everything as "orphans" if
matching looks unreliable (more than 20 percent unmatched). Inspect the paths,
then re-run with `--force` once you are satisfied.

## output

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

## privacy and safety

- the backup is opened read only and is never modified
- nothing is sent off the machine
- the password is taken from `IOS_BACKUP_PASSWORD` or an unechoed prompt, never
  a command line argument, so it does not land in `ps` output or shell history
- the workspace holds decrypted copies of private data. remove it when done:
  `rm -rf workspace`

## how it works

- parses the backup keybag from `Manifest.plist`, derives the key with PBKDF2
  (SHA-256 then SHA-1) from the password, and unwraps the per protection class
  keys with AES key unwrap (RFC 3394)
- decrypts `Manifest.db` and payload files with AES-256-CBC and a zero IV, using
  the per file key stored in each file's NSKeyedArchiver metadata
- reads the decrypted SQLite databases to classify assets and locate thumbnails

## legal

Use revenant only on backups of devices and accounts you own or are authorized
to examine. You are responsible for how you use it.

## license

GNU Affero General Public License v3.0 (AGPLv3)
