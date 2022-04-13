use hbb_common::{bail, ResultType};
use serde_derive::{Deserialize, Serialize};
use std::{collections::HashMap, fs::File, io::prelude::*, path::Path};
use walkdir::WalkDir;

#[derive(Debug, Default, PartialEq, Serialize, Deserialize, Clone)]
pub struct ConfigItem {
    // include directory or file
    pub inc: String,
    // exclude files
    pub exc: Vec<String>,
    // out_path = origin_path - suppressed_front
    pub suppressed_front: String,
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize, Clone)]
pub struct Config {
    // output source file
    pub outfile: String,
    // config items
    pub confs: Vec<ConfigItem>,
}

pub fn get_outin_files<'a>(item: &'a ConfigItem) -> ResultType<HashMap<String, String>> {
    let mut outin_filemap = HashMap::new();

    for entry in WalkDir::new(&item.inc).follow_links(true) {
        let path = entry?.into_path();
        if path.is_file() {
            let mut exclude = false;
            for excfile in item.exc.iter() {
                if excfile.starts_with("*.") {
                    if let Some(ext) = path.extension().and_then(|x| x.to_str()) {
                        if excfile.ends_with(&format!(".{}", ext)) {
                            exclude = true;
                            break;
                        }
                    }
                } else {
                    if path.ends_with(Path::new(excfile)) {
                        exclude = true;
                        break;
                    }
                }
            }
            if exclude {
                continue;
            }

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
                    // Simple replace \ to / here.
                    // A better way is to use lib [path-slash](https://github.com/rhysd/path-slash)
                    Some(s) => s.to_string().replace("\\", "/"),
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

pub fn generate(conf_file: &str) -> ResultType<()> {
    let conf = confy::load_path(conf_file)?;
    generate_with_conf(&conf)?;
    Ok(())
}

pub fn generate_with_conf<'a>(conf: &'a Config) -> ResultType<()> {
    let mut outfile = File::create(&conf.outfile)?;

    outfile.write(
        br##"use hbb_common::{bail, ResultType};
use std::{
    fs::{self, File},
    io::prelude::*,
    path::Path,
};

"##,
    )?;

    outfile.write(b"#[allow(dead_code)]\n")?;
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
#[allow(dead_code)]
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
