extern crate amqp;
extern crate env_logger;

use std::collections::HashMap;
use std::fs::File;
use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::path::PathBuf;
use ofborg::nix;
use std::io::Write;

pub struct OutPathDiff {
    calculator: OutPaths,
    pub original: Option<HashMap<String, String>>,
    pub current: Option<HashMap<String, String>>,
}

impl OutPathDiff {
    pub fn new(nix: nix::Nix, path: PathBuf) -> OutPathDiff {
        OutPathDiff {
            calculator: OutPaths::new(nix, path, false),
            original: None,
            current: None,
        }
    }

    fn parse(&self, f: File) -> HashMap<String, String> {
        let mut result: HashMap<String,String>;
        result = HashMap::new();

        {
            BufReader::new(f)
                .lines()
                .filter_map(|line| match line {
                    Ok(line) => Some(line),
                    Err(_) => None
                })
                .map(|x| {
                    let split: Vec<&str> = x.split_whitespace().collect();
                    if split.len() == 2 {
                        result.insert(String::from(split[0]), String::from(split[1]));
                    } else {
                        info!("Warning: not 2 word segments in {:?}", split);
                    }
                }).count();
        }

        return result;
    }

    pub fn find_before(&mut self) -> bool {
        let x = self.run();
        match x {
            Ok(f) => {
                self.original = Some(self.parse(f));
                return true;
            }
            Err(_) => {
                info!("Failed to find Before list");
                return false;
            }
        }
    }

    pub fn find_after(&mut self) -> Result<bool, File> {
        if self.original == None {
            debug!("Before is None, not bothering with After");
            return Ok(false);
        }

        let x = self.run();
        match x {
            Ok(f) => {
                self.current = Some(self.parse(f));
                return Ok(true);
            }
            Err(e) => {
                info!("Failed to find After list");
                return Err(e);
            }
        }
    }

    pub fn calculate_rebuild(self) -> Option<Vec<String>> {
        let mut rebuild: Vec<String> = vec![];

        if let Some(cur) = self.current {
            if let Some(orig) = self.original {
                for key in cur.keys() {
                    trace!("Checking out {}", key);
                    if cur.get(key) != orig.get(key) {
                        trace!("    {:?} != {:?}", cur.get(key), orig.get(key));
                        rebuild.push(key.clone())
                    } else {
                        trace!("    {:?} == {:?}", cur.get(key), orig.get(key));
                    }
                }

                return Some(rebuild);
            }
        }

        return None;
    }

    fn run(&mut self) -> Result<File, File> {
        self.calculator.find()
    }
}

pub struct OutPaths {
    path: PathBuf,
    nix: nix::Nix,
    check_meta: bool,
}

impl OutPaths    {
    pub fn new(nix: nix::Nix, path: PathBuf, check_meta: bool) -> OutPaths    {
        OutPaths {
            nix: nix,
            path: path,
            check_meta: check_meta,
        }
    }

    pub fn find(&self) -> Result<File, File> {
        self.run()
    }

    fn run(&self) -> Result<File, File> {
        self.place_nix();
        let ret = self.execute();
        self.remove_nix();
        return ret
    }

    fn place_nix(&self) {
        let mut file = File::create(self.nix_path()).expect("Failed to create nix out path check");
        file.write_all(include_str!("outpaths.nix").as_bytes()).expect("Failed to place outpaths.nix");
    }

    fn remove_nix(&self) {
        fs::remove_file(self.nix_path()).expect("Failed to delete outpaths.nix");
    }

    fn nix_path(&self) -> PathBuf {
        let mut dest = self.path.clone();
        dest.push(".gc-of-borg-outpaths.nix");

        dest
    }

    fn execute(&self) -> Result<File, File> {
        let check_meta: String;

        if self.check_meta {
            check_meta = String::from("true");
        } else {
            check_meta = String::from("false");
        }

        self.nix.safely(
            "nix-env",
            &self.path,
            vec![
                String::from("-f"),
                String::from(".gc-of-borg-outpaths.nix"),
                String::from("-qaP"),
                String::from("--no-name"),
                String::from("--out-path"),
                String::from("--show-trace"),
                String::from("--arg"), String::from("checkMeta"), check_meta,
            ],
            true
        )
    }
}
