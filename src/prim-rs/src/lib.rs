pub mod ir;
pub mod jit;
pub mod runtime;
pub mod syntax;

pub use runtime::{Config, Runtime, Stats, Value};
pub use syntax::{Error, Span};

pub fn prepare(source: &str, config: Config) -> syntax::Result<Runtime> {
    prepare_named("<source>", source, config)
}

pub fn prepare_named(name: &str, source: &str, config: Config) -> syntax::Result<Runtime> {
    let tokens = syntax::lex(source)?;
    let ast = syntax::parse(tokens)?;
    let mut program = ir::compile(&ast)?;
    program.add_source(name, source, 0);
    Ok(Runtime::new(program, config))
}

mod terminal;

mod liveness;
pub mod session;
pub use session::Session;

pub mod source;
