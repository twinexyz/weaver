use std::future::Future;
use std::pin::Pin;

use log::{error, info};

use crate::precompile_test;

pub type TestFuture = Pin<Box<dyn Future<Output = eyre::Result<()>> + 'static>>;

pub struct Tests {
    pub tests: Vec<Test>,
}

pub struct Test {
    pub name: String,
    pub description: String,
    pub test: Box<dyn Fn() -> TestFuture + Send + Sync>,
}

impl Default for Tests {
    fn default() -> Self { Self::new() }
}

impl Tests {
    pub fn new() -> Self {
        Self {
            tests: vec![precompile_test::register_precompile_block_production_test()],
        }
    }

    pub fn add_test(
        &mut self,
        name: &str,
        description: &str,
        test: Box<dyn Fn() -> TestFuture + Send + Sync>,
    ) {
        self.tests.push(Test {
            name: name.to_string(),
            description: description.to_string(),
            test,
        });
    }

    pub fn run(&self, regex_to_match: &str) -> eyre::Result<()> {
        let matching_regex = regex::Regex::new(regex_to_match)?;
        info!("Matching regex: {}", matching_regex);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        for test in &self.tests {
            if !matching_regex.is_match(&test.name) {
                continue;
            }
            info!("Running test: {}", test.name);
            match runtime.block_on((test.test)()) {
                Ok(_) => info!("Test passed"),
                Err(e) => error!("Test failed: {:?}", e),
            }
        }
        Ok(())
    }
}
