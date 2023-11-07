use std::{path::{Path, PathBuf}, fs::{self, read_to_string}, process::Command};
use regex::Regex;
use std::sync::mpsc::{ channel, Receiver };
use std::thread::spawn;
use imara_diff::intern::InternedInput;
use imara_diff::sink::Counter;
use imara_diff::{diff, Algorithm};
use crate::error::Error;

struct TestPoint {
	input: PathBuf,
	testfile: PathBuf,
	output: PathBuf,
}

impl TestPoint {
	pub fn copy(&self, dest_folder: &Path) -> Result<(), Error> {
		fs::copy(&self.input, dest_folder.join("input.txt"))?;
		fs::copy(&self.testfile, dest_folder.join("testfile.txt"))?;
		fs::copy(&self.testfile, dest_folder.join("testfile.c"))?;
		fs::copy(&self.output, dest_folder.join("std.txt"))?;
		Ok(())
	}
}

pub struct Tester {
	test_source_folder: PathBuf,
	cwd: PathBuf,
	test_points: Vec<TestPoint>,
	executable: Executable,
}

impl Tester {

	fn load_test_points(&mut self, test_folder: &Path) -> Result<(), Error> {
		let re = Regex::new(r"testfile(?P<id>\d+)\.txt").unwrap();
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
						input: input,
						testfile: path,
						output: output,
					})
				}
            }
		}
		Ok(())
	}

	// fn run_std(&self) -> String {
	// 	Command::new("wsl")
	// 		.args(["clang", "-S", "-emit-llvm", "testfile.c", "-o", "testfile.ll", "-O0"])
	// 		.current_dir(&self.cwd)
	// 		.output().unwrap();
	// 	Command::new("wsl")
	// 		.args(["llvm-link", "testfile.ll", "lib.ll", "-S", "-o", "std.ll"])
	// 		.current_dir(&self.cwd)
	// 		.output().unwrap();
	// 	String::from_utf8(Command::new("wsl")
	// 		.args(["lli", "std.ll"])
	// 		.current_dir(&self.cwd)
	// 		.output().unwrap().stdout).unwrap()
	// }

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

	pub fn start(self) -> (i32, Receiver<Info>) {
		let (sender, receiver) = channel();
		let total = self.test_points.len() as i32;
		spawn(move || {
			fs::copy("libsysy/lib.ll", &self.cwd.join("lib.ll")).unwrap();
			let mut percentage: i32 = 0;
			for point in &self.test_points {
				percentage += 1;
				let message = match point.copy(&self.cwd) {
					Err(e) => Message::OtherError(e.to_string()),
					Ok(_) => match self.executable.run(&self.cwd) {
						Err(Error::RuntimeError(stdout, stderr)) => Message::RuntimeError(stdout, stderr),
						Err(Error::LinkError(stdout, stderr)) => Message::LinkError(stdout, stderr),
						Err(Error::ObjectError(stdout, stderr)) => Message::ObjectError(stdout, stderr),
						Err(e) => Message::OtherError(e.to_string()),
						Ok(output) => {
							fs::write(self.cwd.join("out.txt"), &output).unwrap();
							let std = fs::read_to_string(self.cwd.join("std.txt")).unwrap();
							let input = InternedInput::new(std.as_str(), output.as_str());
							let changes = diff(Algorithm::Histogram, &input, Counter::default());
							if changes.insertions == 0 && changes.removals == 0 {
								Message::Accepted
							} else {
								Message::WrongAnswer(output)
							}
						}
					}
				};
				sender.send(Info { percentage, testfile: point.testfile.clone(), message }).unwrap();
			}
		});
		
		(total, receiver)
	}
}

#[derive(Debug)]
pub struct Info {
	percentage: i32,
	testfile: PathBuf,
	message: Message
}

#[derive(Debug)]
pub enum Message {
	Accepted,
	RuntimeError(String, String),
	LinkError(String, String),
	ObjectError(String, String),
	WrongAnswer(String),
	OtherError(String)
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
		fs::remove_file(cwd.join("output.txt")).ok();
		fs::remove_file(cwd.join("error.txt")).ok();
		fs::remove_file(cwd.join("llvm_ir.txt")).ok();
		match self.ty {
			ExecutableType::Exe => {
				let output = Command::new(&self.executable)
					.current_dir(cwd)
					.output()?;
				if !output.status.success() {
					return Err(Error::RuntimeError(String::from_utf8(output.stdout).unwrap(), String::from_utf8(output.stderr).unwrap()));
				}
				let output = Command::new("wsl")
					.args(["llvm-link", "llvm_ir.txt", "lib.ll", "-S", "-o", "out.ll"])
					.current_dir(cwd)
					.output()?;
				if !output.status.success() {
					return Err(Error::LinkError(String::from_utf8(output.stdout).unwrap(), String::from_utf8(output.stderr).unwrap()));
				}
				let file = fs::File::open(cwd.join("input.txt")).unwrap();
				let output = Command::new("wsl")
					.args(["lli", "out.ll"])
					.stdin(file)
					.current_dir(cwd)
					.output()?;
				if !output.status.success() {
					return Err(Error::ObjectError(String::from_utf8(output.stdout).unwrap(), String::from_utf8(output.stderr).unwrap()));
				}
				Ok(String::from_utf8(output.stdout).unwrap())
			}
			ExecutableType::Jar => todo!()
		}
	}
}
