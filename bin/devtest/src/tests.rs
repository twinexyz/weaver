use log::{error, info};

use crate::precompile_test;

pub struct Tests {
    pub tests: Vec<Test>,
}

pub struct Test {
    pub name: String,
    pub description: String,
    pub test: Box<dyn Fn() -> eyre::Result<()>>,
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
        test: Box<dyn Fn() -> eyre::Result<()>>,
    ) {
        self.tests.push(Test {
            name: name.to_string(),
            description: description.to_string(),
            test,
        });
    }

    pub fn run(&self, regex_to_match: &str) {
        let matching_regex = regex::Regex::new(regex_to_match).unwrap();
        for test in &self.tests {
            if !matching_regex.is_match(&test.name) {
                continue;
            }
            info!("Running test: {}", test.name);
            match (test.test)() {
                Ok(_) => info!("Test passed"),
                Err(e) => error!("Test failed: {}", e),
            }
        }
    }
}
