#!/usr/bin/env bash
cd build/web/
python3 -c 'x=open("./main.dart.js").read();import re;y=re.search("https://.*canvaskit-wasm@([\d\.]+)/bin/",x);dirname="canvaskit@"+y.groups()[0];z=x.replace(y.group(),"/"+dirname+"/");f=open("./main.dart.js", "wt");f.write(z);import os;os.system("ln -s canvaskit " + dirname);'
tar czf x *
scp x sg:/tmp/
ssh sg "sudo tar xzf /tmp/x -C /var/www/html/web.rustdesk.com/ && /bin/rm /tmp/x && sudo chown www-data:www-data /var/www/html/web.rustdesk.com/ -R"
/bin/rm x
cd -
