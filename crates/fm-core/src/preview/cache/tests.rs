use std::cell::Cell;
use std::time::{Duration, Instant, SystemTime};

use fm_api::ErrorCode;
use fm_api::common::{AxisValue, FeatureSetting};
use fm_api::ids::FaceId;

use super::*;
use crate::preview::render_preview;
use crate::test_support::TempDir;

const ASAP: &str = "google-repo/asapsharp/AsapSharp[wdth,wght].ttf";

fn font() -> Vec<u8> {
    fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/fonts")
            .join(ASAP),
    )
    .unwrap()
}

fn req(text: &str) -> PreviewRequest {
    PreviewRequest {
        face: FaceId(1),
        text: text.to_string(),
        size_px: 48.0,
        variations: Vec::new(),
        features: Vec::new(),
        foreground: None,
        background: None,
    }
}

fn file_hash(n: u8) -> FileHash {
    FileHash::from([n; 32])
}

fn key(n: u8) -> CacheKey {
    CacheKey::new(&file_hash(n), 0, &req("Aa"))
}

/// A real thumbnail of `text`.
fn render(text: &str) -> PreviewImage {
    render_preview(&font(), 0, &req(text)).unwrap()
}

fn entry_path(cache: &PreviewCache, key: &CacheKey) -> PathBuf {
    cache.dir().join(key.file_name())
}

#[test]
fn a_hit_returns_the_rendered_thumbnail_without_rendering_again() {
    let dir = TempDir::new("pcache-hit");
    let cache = PreviewCache::open(dir.path(), DEFAULT_LIMIT_BYTES).unwrap();
    let renders = Cell::new(0);
    let draw = || {
        renders.set(renders.get() + 1);
        Ok(render("Hamburgefonstiv"))
    };
    let first = cache.get_or_render(&key(1), draw).unwrap();
    let second = cache.get_or_render(&key(1), draw).unwrap();
    assert_eq!(renders.get(), 1);
    assert_eq!(first, second);
    assert_eq!(first, render("Hamburgefonstiv"));
    assert!(entry_path(&cache, &key(1)).is_file());
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.total_bytes(), first.png.len() as u64);
}

#[test]
fn entries_live_on_disk_across_a_reopen() {
    let dir = TempDir::new("pcache-reopen");
    let image = render("Aa");
    PreviewCache::open(dir.path(), DEFAULT_LIMIT_BYTES)
        .unwrap()
        .put(&key(1), &image)
        .unwrap();
    let reopened = PreviewCache::open(dir.path(), DEFAULT_LIMIT_BYTES).unwrap();
    assert_eq!(reopened.len(), 1);
    assert_eq!(reopened.total_bytes(), image.png.len() as u64);
    assert_eq!(reopened.get(&key(1)), Some(image));
}

#[test]
fn the_key_is_the_face_hash_plus_the_settings_hash() {
    let base = req("Aa");
    let k = |file: u8, index: u32, r: &PreviewRequest| CacheKey::new(&file_hash(file), index, r);
    let same = k(1, 0, &base);
    // The face id is not the face: the same bytes share entries.
    assert_eq!(
        k(
            1,
            0,
            &PreviewRequest {
                face: FaceId(99),
                ..base.clone()
            }
        ),
        same
    );
    assert_eq!(same.face, k(1, 0, &req("other text")).face);
    assert_eq!(same.settings, k(2, 0, &base).settings);

    let mut others = vec![k(2, 0, &base), k(1, 1, &base)];
    let variants = [
        PreviewRequest {
            text: "Ab".into(),
            ..base.clone()
        },
        PreviewRequest {
            size_px: 48.5,
            ..base.clone()
        },
        PreviewRequest {
            variations: vec![AxisValue {
                tag: "wght".into(),
                value: 700.0,
            }],
            ..base.clone()
        },
        PreviewRequest {
            features: vec![FeatureSetting {
                tag: "liga".into(),
                on: false,
            }],
            ..base.clone()
        },
        PreviewRequest {
            foreground: Some("#1a1a1a".into()),
            ..base.clone()
        },
        PreviewRequest {
            background: Some("#1a1a1a".into()),
            ..base.clone()
        },
    ];
    others.extend(variants.iter().map(|r| k(1, 0, r)));
    for (i, other) in others.iter().enumerate() {
        assert_ne!(*other, same, "variant {i} must not share the key");
        for later in &others[i + 1..] {
            assert_ne!(other, later);
        }
    }
}

#[test]
fn file_names_round_trip_and_foreign_names_are_not_keys() {
    let k = key(7);
    assert_eq!(CacheKey::parse(&k.file_name()), Some(k));
    for name in [
        "notes.txt",
        "x.png",
        "0123-4567.png",
        &k.file_name().to_uppercase(),
        &k.file_name().replace(".png", ".jpg"),
    ] {
        assert_eq!(CacheKey::parse(name), None, "{name}");
    }
}

#[test]
fn the_least_recently_used_entry_goes_first() {
    let dir = TempDir::new("pcache-lru");
    let image = render("Aa");
    let size = image.png.len() as u64;
    let cache = PreviewCache::open(dir.path(), 2 * size).unwrap();
    cache.put(&key(1), &image).unwrap();
    cache.put(&key(2), &image).unwrap();
    assert!(cache.get(&key(1)).is_some()); // 2 is now the oldest
    cache.put(&key(3), &image).unwrap();

    assert_eq!(cache.len(), 2);
    assert!(cache.total_bytes() <= cache.limit());
    assert!(cache.get(&key(2)).is_none());
    assert!(!entry_path(&cache, &key(2)).exists());
    assert!(cache.get(&key(1)).is_some());
    assert!(cache.get(&key(3)).is_some());
}

#[test]
fn use_order_survives_a_restart() {
    let dir = TempDir::new("pcache-order");
    let image = render("Aa");
    let size = image.png.len() as u64;
    {
        let cache = PreviewCache::open(dir.path(), DEFAULT_LIMIT_BYTES).unwrap();
        for n in 1..=3 {
            cache.put(&key(n), &image).unwrap();
        }
        // Written in order 1, 2, 3; the file times say 3, 1, 2.
        let base = SystemTime::now() - Duration::from_secs(3600);
        for (n, minutes) in [(3, 1), (1, 2), (2, 3)] {
            File::options()
                .write(true)
                .open(entry_path(&cache, &key(n)))
                .unwrap()
                .set_modified(base + Duration::from_secs(60 * minutes))
                .unwrap();
        }
    }
    let cache = PreviewCache::open(dir.path(), 2 * size).unwrap();
    assert_eq!(cache.len(), 2);
    assert!(cache.get(&key(3)).is_none());
    assert!(!dir.path().join(key(3).file_name()).exists());
    assert!(cache.get(&key(1)).is_some());
    assert!(cache.get(&key(2)).is_some());
}

#[test]
fn a_hit_marks_the_file_as_recently_used() {
    let dir = TempDir::new("pcache-touch");
    let cache = PreviewCache::open(dir.path(), DEFAULT_LIMIT_BYTES).unwrap();
    cache.put(&key(1), &render("Aa")).unwrap();
    let path = entry_path(&cache, &key(1));
    let old = SystemTime::now() - Duration::from_secs(3600);
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(old)
        .unwrap();
    assert!(cache.get(&key(1)).is_some());
    let touched = fs::metadata(&path).unwrap().modified().unwrap();
    assert!(touched > old + Duration::from_secs(1800));
}

#[test]
fn the_limit_defaults_to_500_mb_and_can_change() {
    assert_eq!(DEFAULT_LIMIT_BYTES, 500 * 1024 * 1024);
    assert_eq!(limit_from_mb(500), DEFAULT_LIMIT_BYTES);
    assert_eq!(limit_from_mb(16_384), 16_384 * 1024 * 1024);

    let dir = TempDir::new("pcache-limit");
    let image = render("Aa");
    let size = image.png.len() as u64;
    let cache = PreviewCache::open(dir.path(), DEFAULT_LIMIT_BYTES).unwrap();
    for n in 1..=4 {
        cache.put(&key(n), &image).unwrap();
    }
    assert_eq!(cache.len(), 4);
    cache.set_limit(size);
    assert_eq!(cache.limit(), size);
    assert_eq!(cache.len(), 1);
    assert!(cache.get(&key(4)).is_some());
    let files = fs::read_dir(dir.path()).unwrap().count();
    assert_eq!(files, 1);
}

#[test]
fn opening_over_the_limit_trims_the_folder() {
    let dir = TempDir::new("pcache-trim");
    let image = render("Aa");
    let cache = PreviewCache::open(dir.path(), DEFAULT_LIMIT_BYTES).unwrap();
    for n in 1..=3 {
        cache.put(&key(n), &image).unwrap();
    }
    drop(cache);
    let cache = PreviewCache::open(dir.path(), 0).unwrap();
    assert!(cache.is_empty());
    assert_eq!(cache.total_bytes(), 0);
}

#[test]
fn an_image_larger_than_the_limit_is_not_stored() {
    let dir = TempDir::new("pcache-huge");
    let image = render("Aa");
    let cache = PreviewCache::open(dir.path(), image.png.len() as u64 - 1).unwrap();
    let got = cache.get_or_render(&key(1), || Ok(image.clone())).unwrap();
    assert_eq!(got, image);
    assert!(cache.is_empty());
    assert!(!entry_path(&cache, &key(1)).exists());
}

#[test]
fn a_damaged_entry_is_a_miss_and_is_removed() {
    let dir = TempDir::new("pcache-damaged");
    let cache = PreviewCache::open(dir.path(), DEFAULT_LIMIT_BYTES).unwrap();
    let image = render("Aa");
    cache.put(&key(1), &image).unwrap();
    let path = entry_path(&cache, &key(1));
    fs::write(&path, &image.png[..image.png.len() / 2]).unwrap();
    assert_eq!(cache.get(&key(1)), None);
    assert!(!path.exists());
    assert_eq!(cache.total_bytes(), 0);
    // The next look renders again and heals the entry.
    let got = cache.get_or_render(&key(1), || Ok(image.clone())).unwrap();
    assert_eq!(got, image);
    assert_eq!(cache.get(&key(1)), Some(image));
}

#[test]
fn a_file_removed_behind_the_cache_is_a_miss() {
    let dir = TempDir::new("pcache-gone");
    let cache = PreviewCache::open(dir.path(), DEFAULT_LIMIT_BYTES).unwrap();
    cache.put(&key(1), &render("Aa")).unwrap();
    fs::remove_file(entry_path(&cache, &key(1))).unwrap();
    assert_eq!(cache.get(&key(1)), None);
    assert!(cache.is_empty());
}

#[test]
fn open_removes_half_written_files_and_keeps_foreign_ones() {
    let dir = TempDir::new("pcache-open");
    let tmp = dir.path().join(format!("{}.123.0.tmp", key(1).file_name()));
    let foreign = dir.path().join("readme.txt");
    fs::write(&tmp, b"half").unwrap();
    fs::write(&foreign, b"not ours").unwrap();
    fs::create_dir(dir.path().join(key(2).file_name())).unwrap();
    let cache = PreviewCache::open(dir.path(), DEFAULT_LIMIT_BYTES).unwrap();
    assert!(cache.is_empty());
    assert!(!tmp.exists());
    assert!(foreign.is_file());
    cache.clear();
    assert!(foreign.is_file());
}

#[test]
fn open_creates_the_folder_and_cache_dir_is_under_the_app_cache() {
    let dir = TempDir::new("pcache-create");
    let nested = dir.path().join("a").join("b");
    let cache = PreviewCache::open(&nested, DEFAULT_LIMIT_BYTES).unwrap();
    assert!(nested.is_dir());
    assert_eq!(cache.dir(), nested);

    let paths = AppPaths {
        data: dir.path().join("data"),
        config: dir.path().join("config"),
        cache: dir.path().join("cache"),
        logs: dir.path().join("logs"),
        library: dir.path().join("Fonts"),
    };
    assert_eq!(cache_dir(&paths), dir.path().join("cache").join(CACHE_DIR));
}

#[test]
fn a_render_error_is_returned_and_nothing_is_cached() {
    let dir = TempDir::new("pcache-error");
    let cache = PreviewCache::open(dir.path(), DEFAULT_LIMIT_BYTES).unwrap();
    let err = cache
        .get_or_render(&key(1), || render_preview(b"not a font", 0, &req("Aa")))
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::Unsupported);
    assert!(cache.is_empty());
}

#[test]
fn a_cache_that_cannot_be_written_still_returns_the_render() {
    let dir = TempDir::new("pcache-readonly");
    let folder = dir.path().join("previews");
    let cache = PreviewCache::open(&folder, DEFAULT_LIMIT_BYTES).unwrap();
    fs::remove_dir(&folder).unwrap();
    let image = render("Aa");
    assert!(cache.put(&key(1), &image).is_err());
    assert_eq!(
        cache.get_or_render(&key(1), || Ok(image.clone())).unwrap(),
        image
    );
    assert!(cache.is_empty());
}

#[test]
fn clear_removes_every_entry() {
    let dir = TempDir::new("pcache-clear");
    let cache = PreviewCache::open(dir.path(), DEFAULT_LIMIT_BYTES).unwrap();
    for n in 1..=3 {
        cache.put(&key(n), &render("Aa")).unwrap();
    }
    cache.clear();
    assert!(cache.is_empty());
    assert_eq!(cache.total_bytes(), 0);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn the_cache_is_shared_between_threads() {
    let dir = TempDir::new("pcache-threads");
    let cache = PreviewCache::open(dir.path(), DEFAULT_LIMIT_BYTES).unwrap();
    let image = render("Aa");
    std::thread::scope(|s| {
        for n in 0..8 {
            let (cache, image) = (&cache, &image);
            s.spawn(move || {
                for _ in 0..10 {
                    cache.get_or_render(&key(n), || Ok(image.clone())).unwrap();
                }
            });
        }
    });
    assert_eq!(cache.len(), 8);
    assert_eq!(cache.total_bytes(), 8 * image.png.len() as u64);
}

/// Criterion 2: a cache hit returns in under 2 ms. The median of many hits,
/// so one slow disk access on a shared CI runner does not decide it; the
/// bench (`fm-bench preview-cache`) reports the p95 on the reference library.
#[test]
fn a_hit_returns_in_under_2_ms() {
    let dir = TempDir::new("pcache-speed");
    let cache = PreviewCache::open(dir.path(), DEFAULT_LIMIT_BYTES).unwrap();
    let image = render("Hamburgefonstiv");
    cache.put(&key(1), &image).unwrap();
    assert!(cache.get(&key(1)).is_some()); // warm the OS file cache
    let mut samples: Vec<Duration> = (0..200)
        .map(|_| {
            let begin = Instant::now();
            let hit = cache.get(&key(1));
            let took = begin.elapsed();
            assert!(hit.is_some());
            took
        })
        .collect();
    samples.sort();
    let median = samples[samples.len() / 2];
    assert!(
        median < Duration::from_millis(2),
        "median hit {median:?} for a {} byte thumbnail",
        image.png.len()
    );
}
