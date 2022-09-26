use std::{path::Path, sync::Arc};

use anyhow::Context;
use std::time::Instant;
use swc::{self, config::Options, try_with_handler, HandlerOpts};
use swc_common::source_map::SourceMap;

fn main() {
    let cm = Arc::<SourceMap>::default();

    let mut ts = Vec::<u128>::new();
    let fm = cm
        .load_file(Path::new(
            "test/rxjs/node_modules/rxjs/src/internal/Observable.ts",
        ))
        .expect("failed to load file");
    for x in 1..100 {
        let c = swc::Compiler::new(cm.clone());
        let t1 = Instant::now();

        let output = try_with_handler(
            cm.clone(),
            HandlerOpts {
                ..Default::default()
            },
            |handler| {
                c.process_js_file(
                    fm.clone(),
                    handler,
                    &Options {
                        ..Default::default()
                    },
                )
                .context("failed to process file")
            },
        )
        .unwrap();

        let t2 = Instant::now();
        // println!("{}", output.code);
        ts.push(t2.duration_since(t1).as_micros());
        println!("{}", ts.iter().sum::<u128>() / x);
        println!("{:#?}", t2.duration_since(t1));
    }
}
