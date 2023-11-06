use std::{ffi::OsString, path::{Path, PathBuf}, fs, process::Command, io};
use regex::Regex;
use crate::error::Error;

struct TestPoint {
	input: Option<PathBuf>,
	testfile: PathBuf,
	output: Option<PathBuf>,
}

impl TestPoint {
	pub fn copy(&self, dest_folder: &Path) -> Result<(), Error> {
		if let Some(input) = &self.input {
			fs::copy(input, dest_folder.join("input.txt"))?;
		}
		fs::copy(&self.testfile, dest_folder.join("testfile.txt"))?;
		if let Some(output) = &self.output {
			fs::copy(output, dest_folder.join("output.txt"))?;
		}
		Ok(())
	}
}

struct Tester {
	test_source_folder: PathBuf,
	cwd: PathBuf,
	test_points: Vec<TestPoint>,
	executable: Executable,
}

impl Tester {

	fn load_test_points(&mut self, test_folder: &Path) -> Result<(), Error> {
		let re = Regex::new(r"testfile(?P<id>)\.txt").unwrap();
		for entry in fs::read_dir(test_folder)? {
			let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                self.load_test_points(&path)?;
            } else {
				let str = path.file_name().unwrap().to_str().unwrap();
                if re.is_match(str) {
					let id = re.captures(str).unwrap().name("id").unwrap().as_str();
					let input = test_folder.join(format!("input{id}.txt"));
					let output = test_folder.join(format!("output{id}.txt"));
					self.test_points.push(TestPoint {
						input: Some(input),
						testfile: path,
						output: Some(output),
					})
				}
            }
		}
		Ok(())
	}

	pub fn new(test_source_folder: PathBuf, cwd: PathBuf, executable: PathBuf) -> Result<Self, Error> {
		let mut result = Self {
			test_source_folder: test_source_folder.clone(),
			cwd,
			test_points: Vec::new(),
			executable: Executable::new(executable)?
		};
		result.load_test_points(test_source_folder.as_path())?;
		Ok(result)
	}

	// pub fn start(&mut self) -> Result<(), Error> {
	// 	let mut stdout = io::stdout().lock();
	// 	for point in &self.test_points {
	// 		stdout.
	// 		match point.copy(self.cwd) {
	// 			Err(e) => {
				
	// 			}
	// 		}
			
	// 	}
	// }
}

enum ExecutableType {
	Exe,
	Jar
}

struct Executable {
	executable: PathBuf,
	ty: ExecutableType,
}

impl Executable {
	pub fn new(executable: PathBuf) -> Result<Self, Error> {
		let ty = match executable.extension() {
			Some(ext) => {
				let ext = ext.to_str().unwrap();
				match ext {
					"exe" => ExecutableType::Exe,
					"jar" => ExecutableType::Jar,
					_ => return Err(Error::InvalidPath)
				}
			}
			None => ExecutableType::Exe
		};
		Ok(Self { executable, ty })
	}

	pub fn run(&self, cwd: &Path) -> Result<String, Error> {
		match self.ty {
			ExecutableType::Exe => {
				let output = Command::new("wsl")
					.args([self.executable])
					.current_dir(cwd)
					.output()?;
				if !output.status.success() {
					return Err(Error::RuntimeError(String::from_utf8(output.stdout).unwrap(), String::from_utf8(output.stderr).unwrap()));
				}
				Command::new("cp")
					.args(["libsysy/lib.ll", cwd.as_os_str().to_str().unwrap()])
					.output()?;
				let output = Command::new("wsl")
					.args(["llvm-link", "testfile.ll", "lib.ll", "-S", "-o", "out.ll"])
					.current_dir(cwd)
					.output()?;
				if !output.status.success() {
					return Err(Error::RuntimeError(String::from_utf8(output.stdout).unwrap(), String::from_utf8(output.stderr).unwrap()));
				}
				let output = Command::new("wsl")
					.args(["lli", "out.ll"])
					.output()?;
				if !output.status.success() {
					return Err(Error::RuntimeError(String::from_utf8(output.stdout).unwrap(), String::from_utf8(output.stderr).unwrap()));
				}
				Ok(String::from_utf8(output.stdout).unwrap())
			}
			ExecutableType::Jar => todo!()
		}
	}
}
