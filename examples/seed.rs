// Dev-only helper to populate a RocksDB directory with a few CFs and keys
// so you can exercise rocksdb-mcp end-to-end. Run: cargo run --example seed -- /tmp/mydb

use std::path::Path;

use rocksdb::{DB, Options};

fn main() {
    let path = std::env::args().nth(1).expect("usage: seed <db-path>");
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    let cfs = ["default", "meta"];
    let db = DB::open_cf(&opts, Path::new(&path), cfs).unwrap();

    let def = db.cf_handle("default").unwrap();
    db.put_cf(&def, b"greeting", b"hello world").unwrap();
    db.put_cf(&def, b"binary", [0xff, 0xfe, 0x00, 0x01])
        .unwrap();

    let meta = db.cf_handle("meta").unwrap();
    db.put_cf(&meta, b"version", b"42").unwrap();

    println!("seeded {path}");
}
