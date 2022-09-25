#![feature(is_some_with)]

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server, StatusCode};
use std::convert::Infallible;
use std::error::Error;
use std::net::SocketAddr;

use std::{
    env, fs,
    panic::{catch_unwind, AssertUnwindSafe},
    path::{Path, PathBuf},
    sync::Arc,
};

use swc_core::{
    base::{config::Options, try_with_handler, Compiler},
    bundler::Resolve,
    common::{sync::Lazy, FileName, FilePathMapping, SourceMap},
};
use swc_ecma_visit::{
    noop_fold_type,
    swc_ecma_ast::{ExportAll, ImportDecl, Module, NamedExport, Script},
    Fold,
};

use swc_ecma_loader::{resolvers::node::NodeModulesResolver, TargetEnv};

struct Noop;
impl Fold for Noop {
    #[inline(always)]
    fn fold_module(&mut self, m: Module) -> Module {
        m
    }

    #[inline(always)]
    fn fold_script(&mut self, s: Script) -> Script {
        s
    }
}

pub fn noop() -> impl Fold {
    Noop
}

static COMPILER: Lazy<Arc<Compiler>> = Lazy::new(|| {
    let cm = Arc::new(SourceMap::new(FilePathMapping::empty()));

    Arc::new(Compiler::new(cm))
});

fn get_compiler() -> Arc<Compiler> {
    COMPILER.clone()
}

struct TransformImport<'a> {
    root: &'a String,
    path: &'a str,
}

impl<'a> Fold for TransformImport<'a> {
    noop_fold_type!();

    fn fold_import_decl(&mut self, n: ImportDecl) -> ImportDecl {
        if !n.src.value.starts_with("/")
            || !n.src.value.starts_with("./")
            || !n.src.value.starts_with("../")
        {
            let resolver = NodeModulesResolver::new(TargetEnv::Browser, Default::default(), false);
            let resolved_path = resolver
                .resolve(&FileName::Real(self.path.into()), n.src.value.as_ref())
                .unwrap()
                .to_string();
            let mut cloned = n.clone();
            cloned.src.value = resolved_path.strip_prefix(self.root).unwrap().into();
            return cloned;
        }
        return n;
    }

    fn fold_named_export(&mut self, n: NamedExport) -> NamedExport {
        if !n.src.as_ref().map_or(true, |s| {
            s.value.starts_with("/") || s.value.starts_with("./") || s.value.starts_with("../")
        }) {
            let resolver = NodeModulesResolver::new(TargetEnv::Browser, Default::default(), false);
            let resolved_path = resolver
                .resolve(
                    &FileName::Real(self.path.into()),
                    n.src.clone().unwrap().value.as_ref(),
                )
                .unwrap()
                .to_string();
            let mut cloned = n.clone();
            cloned.src = Some(Box::new(
                resolved_path.strip_prefix(self.root).unwrap().into(),
            ));
            return cloned;
        }
        return n;
    }

    fn fold_export_all(&mut self, n: ExportAll) -> ExportAll {
        if !n.src.value.starts_with("/")
            || !n.src.value.starts_with("./")
            || !n.src.value.starts_with("../")
        {
            let resolver = NodeModulesResolver::new(TargetEnv::Browser, Default::default(), false);
            let resolved_path = resolver
                .resolve(&FileName::Real(self.path.into()), n.src.value.as_ref())
                .unwrap()
                .to_string();
            let mut cloned = n.clone();
            cloned.src.value = resolved_path.strip_prefix(self.root).unwrap().into();
            return cloned;
        }
        return n;
    }
}

static ROOT: Lazy<Arc<PathBuf>> = Lazy::new(|| {
    Arc::new(
        fs::canonicalize(Path::new(&env::var("ROOT").unwrap_or("test/simple".into()))).unwrap(),
    )
});

static SWC_OPTS: Lazy<swc_core::base::HandlerOpts> = Lazy::new(|| swc_core::base::HandlerOpts {
    ..Default::default()
});

async fn hello_world(req: Request<Body>) -> Result<Response<String>, Infallible> {
    let path = Path::new(ROOT.clone().as_ref()).join(&req.uri().path()[1..]);

    let file = tokio::fs::read_to_string(&path).await;

    let mut builder = Response::builder().status(StatusCode::OK);

    match file {
        Ok(content) => {
            let ext = path.extension().unwrap();

            if ext == "html" {
                return Ok(builder
                    .header("Content-Type", "text/html")
                    .body(content)
                    .unwrap());
            }

            if ext == "js"
                || ext == "ts"
                || ext == "jsx"
                || ext == "tsx"
                || ext == "mjs"
                || ext == "cjs"
                || ext == "mts"
                || ext == "mtsx"
            {
                let c = get_compiler();

                let code = try_with_handler(c.cm.clone(), SWC_OPTS.clone(), |handler| {
                    let cloned_path = path.clone();
                    let transform_import = TransformImport {
                        root: &String::from(ROOT.to_str().unwrap()),
                        path: cloned_path.as_path().to_str().unwrap(),
                    };

                    let options = Options {
                        ..Default::default()
                    };

                    let result = catch_unwind(AssertUnwindSafe(|| {
                        c.process_js_with_custom_pass(
                            c.cm.new_source_file(FileName::Real(path), content),
                            None,
                            handler,
                            &options,
                            |_, _| noop(),
                            |_, _| transform_import,
                        )
                    }));
                    let code = result.unwrap().unwrap().code;
                    return Ok(code);
                })
                .unwrap();

                return Ok(builder
                    .header("Content-Type", "application/javascript")
                    .body(code)
                    .unwrap());
            }

            return Ok(builder
                .header("Content-Type", "text/plain")
                .body(content)
                .unwrap());
        }
        _ => Ok(builder.status(404).body("".into()).unwrap()),
    }
}

#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([127, 0, 0, 1], 8000));

    // A `Service` is needed for every connection, so this
    // creates one from our `hello_world` function.
    let make_svc = make_service_fn(|_conn| async {
        // service_fn converts our function into a `Service`
        Ok::<_, Infallible>(service_fn(hello_world))
    });

    let server = Server::bind(&addr).serve(make_svc);

    // Run this server for... forever!
    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}
