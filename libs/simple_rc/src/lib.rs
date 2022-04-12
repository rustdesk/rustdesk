use hbb_common::{bail, ResultType};
use serde_derive::{Deserialize, Serialize};
use std::{collections::HashMap, fs::File, io::prelude::*, path::Path};
use walkdir::WalkDir;

const CONF_FILE: &str = "simple_rc.toml";

#[derive(Debug, Default, PartialEq, Serialize, Deserialize, Clone)]
pub struct ConfigItem {
    #[serde(default)]
    pub inc: String,
    #[serde(default)]
    pub exc: Vec<String>,
    #[serde(default)]
    pub suppressed_front: String,
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    confs: Vec<ConfigItem>,
    #[serde(default)]
    outfile: String,
}

pub fn get_outin_files<'a>(item: &'a ConfigItem) -> ResultType<HashMap<String, String>> {
    let mut outin_filemap = HashMap::new();

    for entry in WalkDir::new(&item.inc).follow_links(true) {
        let path = entry?.into_path();
        if path.is_file() {
            let mut suppressed_front = item.suppressed_front.clone();
            if !suppressed_front.is_empty() && suppressed_front.ends_with('/') {
                suppressed_front.push('/');
            }
            let outpath = path.strip_prefix(Path::new(&suppressed_front))?;
            let outfile = if outpath.is_absolute() {
                match outpath
                    .file_name()
                    .and_then(|f| f.to_str())
                    .map(|f| f.to_string())
                {
                    None => {
                        bail!("Failed to get filename of {}", outpath.display());
                    }
                    Some(s) => s,
                }
            } else {
                match outpath.to_str() {
                    None => {
                        bail!("Failed to convert {} to string", outpath.display());
                    }
                    Some(s) => s.to_string(),
                }
            };
            let infile = match path.canonicalize()?.to_str() {
                None => {
                    bail!("Failed to get file path of {}", path.display());
                }
                Some(s) => s.to_string(),
            };
            if let Some(_) = outin_filemap.insert(outfile.clone(), infile) {
                bail!("outfile {} is set before", outfile);
            }
        }
    }

    Ok(outin_filemap)
}

pub fn generate<'a>(conf: &'a Config) -> ResultType<()> {
    let mut outfile = File::create(&conf.outfile)?;

    outfile.write(
        br##"
use hbb_common::{bail, ResultType};
use std::{
    fs::{self, File},
    io::prelude::*,
    path::Path,
};

"##,
    )?;

    outfile.write(b"pub fn extract_resources() -> ResultType<()> {\n")?;
    outfile.write(b"    let mut resources: Vec<(&str, &[u8])> = Vec::new();\n")?;

    let mut outin_files = HashMap::new();
    for item in conf.confs.iter() {
        for (o, i) in get_outin_files(item)?.into_iter() {
            if let Some(_) = outin_files.insert(o.clone(), i) {
                bail!("outfile {} is set before", o);
            }
        }
    }

    let mut count = 1;
    for (o, i) in outin_files.iter() {
        let mut infile = File::open(&i)?;
        let mut buffer = Vec::<u8>::new();
        infile.read_to_end(&mut buffer)?;

        let var_outfile = format!("outfile_{}", count);
        let var_outdata = format!("outdata_{}", count);

        write!(outfile, "    let {} = \"{}\";\n", var_outfile, o)?;
        write!(outfile, "    let {} = [\n        ", var_outdata)?;

        let mut line_num = 20;
        for v in buffer {
            if line_num == 0 {
                write!(outfile, "\n        ")?;
                line_num = 20;
            }
            write!(outfile, "{:#04x}u8, ", v)?;
            line_num -= 1;
        }
        write!(outfile, "\n    ];\n")?;

        write!(
            outfile,
            "    resources.push(({}, &{}));\n",
            var_outfile, var_outdata
        )?;

        count += 1;
    }

    outfile.write(b"    do_extract(resources)?;\n")?;
    outfile.write(b"    Ok(())\n")?;
    outfile.write(b"}\n")?;

    outfile.write(
        br##"
fn do_extract(resources: Vec<(&str, &[u8])>) -> ResultType<()> {
    for (outfile, data) in resources {
        match Path::new(outfile).parent().and_then(|p| p.to_str()) {
            None => {
                bail!("Failed to get parent of {}", outfile);
            }
            Some(p) => {
                fs::create_dir_all(p)?;
                let mut of = File::create(outfile)?;
                of.write_all(data)?;
                of.flush()?;
            }
        }
    }
    Ok(())
}
"##,
    )?;

    outfile.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }

    #[test]
    fn test_generate() {
        use super::*;
        generate(&Config {
            confs: vec![ConfigItem {
                inc: "D:/aa.png".to_owned(),
                exc: vec![],
                suppressed_front: "".to_owned(),
            }],
            outfile: "src/aa.rs".to_owned(),
        })
        .unwrap();
    }

    // #[test]
    // fn test_extract() {
    //     use super::*;
    //     aa::extract_resources().unwrap();
    // }
}
