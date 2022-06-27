extern crate simple_rc;

use simple_rc::*;

fn main() {
    {
        const CONF_FILE: &str = "simple_rc.toml";
        generate(CONF_FILE).unwrap();
    }

    {
        generate_with_conf(&Config {
            outfile: "src/rc.rs".to_owned(),
            confs: vec![ConfigItem {
                inc: "D:/projects/windows/RustDeskTempTopMostWindow/x64/Release/xxx".to_owned(),
                // exc: vec!["*.dll".to_owned(), "*.exe".to_owned()],
                exc: vec![],
                suppressed_front: "D:/projects/windows".to_owned(),
            }],
        })
        .unwrap();
    }
}
