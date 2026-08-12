//! tcol lib: теговая коллекция файлов поверх eml-tag.
//!
//! Файлы остаются на месте; коллекция = tagdb-записи:
//!   X-Record-ID: f_<sha256 пути>
//!   X-Tag: game / wasm / ...
//!   X-Timestamp: mtime
//!   тело: {"path": <canonical>, "size": ..., "mtime": ..., "sha256": ...}

use emlbox::format::{find_blank, hash_bytes};
use emlbox::tagdb;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub fn record_id(path: &str) -> String {
    format!("f_{}", hash_bytes(path.as_bytes()))
}

fn body_of(p: &Path) -> Result<Value, String> {
    let data = std::fs::read(p).map_err(|e| e.to_string())?;
    let (end, sep) = find_blank(&data).ok_or("no blank line in record")?;
    serde_json::from_slice(&data[end + sep..]).map_err(|e| e.to_string())
}

fn mtime_secs(f: &Path) -> u64 {
    std::fs::metadata(f)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn add_file(db: &Path, f: &Path, tags: &[String]) -> Result<(), String> {
    let canon = f
        .canonicalize()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string();
    let data = std::fs::read(f).map_err(|e| e.to_string())?;
    let size = data.len() as u64;
    let sha = hash_bytes(&data);
    let mtime = mtime_secs(f);
    // merge: re-adding a file keeps old tags and adds new ones
    let id = record_id(&canon);
    let mut merged: Vec<String> = tags.to_vec();
    let q = format!("X-Record-ID == \"{id}\"");
    if let Ok((recs, _)) = tagdb::query(db, &q) {
        if let Some((_, headers)) = recs.first() {
            for (k, v) in headers {
                if k.eq_ignore_ascii_case("X-Tag") && !merged.contains(v) {
                    merged.push(v.clone());
                }
            }
        }
    }
    let body = serde_json::json!({"path": canon, "size": size, "mtime": mtime, "sha256": sha});
    tagdb::insert(db, &id, &merged, "local", mtime, &body).map_err(|e| e.to_string())?;
    Ok(())
}

/// Add a file (or, with recursive=true, a whole tree) to the collection.
pub fn add(db: &Path, path: &Path, tags: &[String], recursive: bool) -> Result<usize, String> {
    let mut n = 0usize;
    if path.is_dir() && recursive {
        let mut files = Vec::new();
        let mut stack = vec![path.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    files.push(p);
                }
            }
        }
        files.sort();
        for f in files {
            add_file(db, &f, tags)?;
            n += 1;
        }
    } else if path.is_file() {
        add_file(db, path, tags)?;
        n = 1;
    } else {
        return Err(format!("not a file or directory: {}", path.display()));
    }
    Ok(n)
}

/// Find records by X-Query. Returns (path, tags).
pub fn find(db: &Path, q: &str) -> Result<Vec<(String, Vec<String>)>, String> {
    let (records, _) = tagdb::query(db, q)?;
    let mut out = Vec::new();
    for (p, headers) in records {
        let tags: Vec<String> = headers
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case("X-Tag"))
            .map(|(_, v)| v.clone())
            .collect();
        let path = match body_of(&p) {
            Ok(b) => b.get("path").and_then(|v| v.as_str()).unwrap_or("?").to_string(),
            Err(_) => "?".to_string(),
        };
        out.push((path, tags));
    }
    Ok(out)
}

/// All records as (path, tags), sorted by path.
pub fn ls(db: &Path) -> Result<Vec<(String, Vec<String>)>, String> {
    find(db, "X-Record-ID != \"\"")
}

/// Tag counts across the collection (header-only).
pub fn tag_counts(db: &Path) -> Result<Vec<(String, usize)>, String> {
    let (records, _) = tagdb::scan(db)?;
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (_, h) in records {
        for (k, v) in h {
            if k.eq_ignore_ascii_case("X-Tag") {
                *counts.entry(v.clone()).or_default() += 1;
            }
        }
    }
    let mut out: Vec<_> = counts.into_iter().collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    Ok(out)
}

/// Sum of tracked file sizes.
pub fn total_bytes(db: &Path) -> Result<u64, String> {
    let (records, _) = tagdb::scan(db)?;
    let mut total = 0u64;
    for (p, _) in records {
        if let Ok(b) = body_of(&p) {
            total += b.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
        }
    }
    Ok(total)
}

/// Remove a file from the collection (by canonical path).
pub fn remove(db: &Path, path: &Path) -> Result<bool, String> {
    let canon = path
        .canonicalize()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string();
    let p = db.join(format!("{}.eml", record_id(&canon)));
    if p.exists() {
        std::fs::remove_file(&p).map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Path -> record id lookup for tests.
pub fn id_of(db: &Path, path: &Path) -> Option<PathBuf> {
    let canon = path.canonicalize().ok()?.to_string_lossy().to_string();
    let p = db.join(format!("{}.eml", record_id(&canon)));
    p.exists().then_some(p)
}

/// (records, tracked bytes, corrupt skipped)
pub fn tagdb_stats(db: &Path) -> Result<(usize, u64, usize), String> {
    let (records, corrupt) = tagdb::scan(db)?;
    let bytes = total_bytes(db)?;
    Ok((records.len(), bytes, corrupt))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(tag: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("tcol_{}_{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("db");
        std::fs::create_dir_all(&db).unwrap();
        let f = dir.join("a.txt");
        std::fs::write(&f, b"hello").unwrap();
        (db, f)
    }

    #[test]
    fn add_find_remove_roundtrip() {
        let (db, f) = setup("rt");
        let n = add(&db, &f, &["game".into(), "wasm".into()], false).unwrap();
        assert_eq!(n, 1);

        let res = find(&db, "X-Tag == \"game\"").unwrap();
        assert_eq!(res.len(), 1);
        assert!(res[0].0.ends_with("a.txt"));
        assert_eq!(res[0].1, vec!["game", "wasm"]);

        let res = find(&db, "X-Tag == \"game\" AND X-Tag == \"wasm\"").unwrap();
        assert_eq!(res.len(), 1);

        let res = find(&db, "X-Tag == \"missing\"").unwrap();
        assert!(res.is_empty());

        assert_eq!(tag_counts(&db).unwrap(), vec![("game".to_string(), 1), ("wasm".to_string(), 1)]);

        assert!(remove(&db, &f).unwrap());
        assert!(find(&db, "X-Tag == \"game\"").unwrap().is_empty());
    }

    #[test]
    fn recursive_add_tree() {
        let dir = std::env::temp_dir().join(format!("tcol_rec_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("db");
        std::fs::create_dir_all(&db).unwrap();
        let tree = dir.join("tree");
        std::fs::create_dir_all(tree.join("sub")).unwrap();
        std::fs::write(tree.join("1.rs"), b"fn main(){}").unwrap();
        std::fs::write(tree.join("sub/2.py"), b"print(1)").unwrap();

        let n = add(&db, &tree, &["code".into()], true).unwrap();
        assert_eq!(n, 2);
        assert_eq!(find(&db, "X-Tag == \"code\"").unwrap().len(), 2);
        assert_eq!(total_bytes(&db).unwrap(), 19); // "fn main(){}" = 11 B + "print(1)" = 8 B
    }

    #[test]
    fn duplicate_add_merges_tags() {
        let (db, f) = setup("dup");
        add(&db, &f, &["a".into()], false).unwrap();
        add(&db, &f, &["b".into()], false).unwrap();
        let (records, _) = tagdb::scan(&db).unwrap();
        assert_eq!(records.len(), 1, "same file -> same record id");
        // tags are merged, not replaced
        assert_eq!(find(&db, "X-Tag == \"a\" AND X-Tag == \"b\"").unwrap().len(), 1);
        assert_eq!(tag_counts(&db).unwrap(), vec![("a".to_string(), 1), ("b".to_string(), 1)]);
    }
}
