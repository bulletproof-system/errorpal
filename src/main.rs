
use errorpal::checker::{Tester, Info};
use std::path::PathBuf;

const TEST_SOURCE_FOLDER: &str = r"D:\LTT\repository\Compiler\coderry\tests\ir_tests";
const CWD: &str = r"D:\LTT\repository\Compiler\errorpal\cwd";
const EXECUTABLE: &str = r"D:\LTT\repository\Compiler\coderry\build\src\coderry.exe";

fn main() -> () {
    let test_source_folder = PathBuf::from(TEST_SOURCE_FOLDER);
    let cwd = PathBuf::from(CWD);
    let executable = PathBuf::from(EXECUTABLE);
    let tester = Tester::new(test_source_folder, cwd, executable).unwrap();
    let (total, receiver) = tester.start();
    let mut now = 0;
    while let Ok(result) = receiver.recv() {
        println!("{:?}\n", result);
    }
}