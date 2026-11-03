import sys
import re

content = open('.github/workflows/flutter-build.yml', 'r', encoding='utf-8').read()

# Replace python3 .\build.py with python3 .\build.py then flutter pub run msix:create
new_content = content.replace(
    'python3 .\\build.py --portable --hwcodec --flutter --vram --skip-portable-pack',
    'python3 .\\build.py --portable --hwcodec --flutter --vram --skip-portable-pack\n          cd flutter\n          flutter pub run msix:create\n          cd ..'
)

open('.github/workflows/flutter-build.yml', 'w', encoding='utf-8').write(new_content)
