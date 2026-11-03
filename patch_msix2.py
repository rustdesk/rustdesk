import sys

content = open('.github/workflows/flutter-build.yml', 'r', encoding='utf-8').read()

target = 'name: rustdesk-unsigned-windows-${{ matrix.job.arch }}'
# find index
idx = content.find(target)
if idx == -1:
    print("target not found")
    sys.exit(1)

# Find the next artifact block to insert ours before it, or just insert after
idx2 = content.find('path: ', idx)
idx3 = content.find('\n', idx2)

msix_block = """
      - name: Upload MSIX
        if: env.UPLOAD_ARTIFACT == 'true'
        uses: actions/upload-artifact@master
        with:
          name: rocketdesk-windows-${{ matrix.job.arch }}.msix
          path: flutter/build/windows/x64/runner/Release/RocketDesk.msix
"""

content = content[:idx3+1] + msix_block + content[idx3+1:]
open('.github/workflows/flutter-build.yml', 'w', encoding='utf-8').write(content)
