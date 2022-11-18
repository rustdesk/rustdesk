use hbb_common::{bail, ResultType};
use std::{
    fs::{self, File},
    io::prelude::*,
    path::Path,
};

#[allow(dead_code)]
pub fn extract_resources(root_path: &str) -> ResultType<()> {
    let mut resources: Vec<(&str, &[u8])> = Vec::new();
    resources.push((outfile_4, &outdata_4));
    do_extract(root_path, resources)?;
    Ok(())
}

#[allow(dead_code)]
fn do_extract(root_path: &str, resources: Vec<(&str, &[u8])>) -> ResultType<()> {
    let mut root_path = root_path.replace("\\", "/");
    if !root_path.ends_with('/') {
        root_path.push('/');
    }
    let root_path = Path::new(&root_path);
    for (outfile, data) in resources {
        let outfile_path = root_path.join(outfile);
        match outfile_path.parent().and_then(|p| p.to_str()) {
            None => {
                bail!("Failed to get parent of {}", outfile_path.display());
            }
            Some(p) => {
                fs::create_dir_all(p)?;
                let mut of = File::create(outfile_path)?;
                of.write_all(data)?;
                of.flush()?;
            }
        }
    }
    Ok(())
}
