fn main() {
    let org = "";
    let app = "RustDesk";
    if let Some(project) = directories_next::ProjectDirs::from("", org, app) {
        println!("Project config dir: {:?}", project.config_dir());
    } else {
        println!("None");
    }
}
