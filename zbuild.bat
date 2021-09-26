cd  C:\Users\fskhan\source\repos\TopsDesk\rustdesk\src
packfolder ui/res resources.rc -v "resources" -binary
cd  C:\Users\fskhan\source\repos\TopsDesk\rustdesk\
cargo  build --features "inline" --release  
upx --best --lzma target/release/rustdesk.exe
.\target\release\rustdesk.exe