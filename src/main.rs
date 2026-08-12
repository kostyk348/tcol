//! tcol CLI — теговая коллекция файлов (плоский теговый диск).
//!
//!   tcol init <db>
//!   tcol add <db> <path> [--tag T]... [--recursive]
//!   tcol rm <db> <path>
//!   tcol find <db> "X-Query"
//!   tcol ls <db>
//!   tcol tags <db>
//!   tcol stats <db>

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let code = match args.get(1).map(|s| s.as_str()) {
        Some("init") => {
            match args.get(2) {
                Some(db) => match std::fs::create_dir_all(db) {
                    Ok(()) => {
                        println!("init {}", db);
                        0
                    }
                    Err(e) => err(&e.to_string()),
                },
                None => err("init: need db dir"),
            }
        }
        Some("add") => cmd_add(&args[2..]),
        Some("rm") => cmd_rm(&args[2..]),
        Some("find") => cmd_find(&args[2..]),
        Some("ls") => cmd_ls(&args[2..]),
        Some("tags") => cmd_tags(&args[2..]),
        Some("stats") => cmd_stats(&args[2..]),
        _ => {
            eprintln!("tcol: см. usage в шапке исходника");
            2
        }
    };
    std::process::exit(code);
}

fn cmd_add(a: &[String]) -> i32 {
    let db = match a.get(0).map(PathBuf::from) {
        Some(p) => p,
        None => return err("add: need db"),
    };
    let path = match a.get(1).map(PathBuf::from) {
        Some(p) => p,
        None => return err("add: need path"),
    };
    let mut tags: Vec<String> = Vec::new();
    let mut recursive = false;
    let mut i = 2;
    while i < a.len() {
        if a[i] == "--tag" {
            if let Some(t) = a.get(i + 1) {
                tags.push(t.clone());
                i += 2;
                continue;
            }
        }
        if a[i] == "--recursive" {
            recursive = true;
        }
        i += 1;
    }
    match tcol::add(&db, &path, &tags, recursive) {
        Ok(n) => {
            println!("added {n} file(s) with tags {:?}", tags);
            0
        }
        Err(e) => err(&e),
    }
}

fn cmd_rm(a: &[String]) -> i32 {
    let db = match a.get(0).map(PathBuf::from) {
        Some(p) => p,
        None => return err("rm: need db"),
    };
    let path = match a.get(1).map(PathBuf::from) {
        Some(p) => p,
        None => return err("rm: need path"),
    };
    match tcol::remove(&db, &path) {
        Ok(true) => {
            println!("removed {}", path.display());
            0
        }
        Ok(false) => {
            println!("not in collection: {}", path.display());
            0
        }
        Err(e) => err(&e),
    }
}

fn cmd_find(a: &[String]) -> i32 {
    let db = match a.get(0).map(PathBuf::from) {
        Some(p) => p,
        None => return err("find: need db"),
    };
    let q = match a.get(1) {
        Some(q) => q.clone(),
        None => return err("find: need X-Query"),
    };
    match tcol::find(&db, &q) {
        Ok(res) => {
            for (path, tags) in &res {
                println!("  {}  [{}]", path, tags.join(","));
            }
            println!("{} match(es)", res.len());
            0
        }
        Err(e) => err(&e),
    }
}

fn cmd_ls(a: &[String]) -> i32 {
    let db = match a.get(0).map(PathBuf::from) {
        Some(p) => p,
        None => return err("ls: need db"),
    };
    match tcol::ls(&db) {
        Ok(res) => {
            for (path, tags) in &res {
                println!("  {}  [{}]", path, tags.join(","));
            }
            println!("{} record(s)", res.len());
            0
        }
        Err(e) => err(&e),
    }
}

fn cmd_tags(a: &[String]) -> i32 {
    let db = match a.get(0).map(PathBuf::from) {
        Some(p) => p,
        None => return err("tags: need db"),
    };
    match tcol::tag_counts(&db) {
        Ok(counts) => {
            for (tag, n) in &counts {
                println!("  {:>4}  {}", n, tag);
            }
            0
        }
        Err(e) => err(&e),
    }
}

fn cmd_stats(a: &[String]) -> i32 {
    let db = match a.get(0).map(PathBuf::from) {
        Some(p) => p,
        None => return err("stats: need db"),
    };
    match tcol::tagdb_stats(&db) {
        Ok((files, bytes, corrupt)) => {
            println!("records: {files}, tracked bytes: {bytes}, corrupt: {corrupt}");
            0
        }
        Err(e) => err(&e),
    }
}

fn err(e: &str) -> i32 {
    eprintln!("tcol: {e}");
    1
}
