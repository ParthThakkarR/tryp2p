import os
import json

output_file = r'C:\Users\parth\.gemini\antigravity-ide\brain\bf1e8cbf-8463-4726-b5ac-edcc8c82b3eb\technical_summary.md'
cwd = r'd:\tryp2p'
targets = [
    'p2ptransfer-core/src',
    'desktop/p2ptransfer-cli/src',
    'desktop/p2ptransfer-gui/src',
    'desktop/p2ptransfer-gui/src-tauri/src'
]
cargo_tomls = [
    'Cargo.toml',
    'p2ptransfer-core/Cargo.toml',
    'desktop/p2ptransfer-cli/Cargo.toml',
    'desktop/p2ptransfer-gui/src-tauri/Cargo.toml'
]
package_jsons = [
    'desktop/p2ptransfer-gui/package.json'
]

with open(output_file, 'w', encoding='utf-8') as out:
    out.write('# Technical Summary\n\n')
    
    # 1. Project structure
    out.write('## 1. Project Structure\n\n`\n')
    for root, dirs, files in os.walk(cwd):
        dirs[:] = [d for d in dirs if d not in ['target', 'node_modules', '.git', 'dist', 'build']]
        level = root.replace(cwd, '').count(os.sep)
        indent = ' ' * 4 * (level)
        out.write(f"{indent}{os.path.basename(root)}/\n")
        subindent = ' ' * 4 * (level + 1)
        for f in files:
            out.write(f"{subindent}{f}\n")
    out.write('`\n\n')
    
    # 2. File contents
    out.write('## 2. File Contents\n\n')
    for target in targets:
        target_path = os.path.join(cwd, target)
        if not os.path.exists(target_path):
            continue
        for root, _, files in os.walk(target_path):
            for file in files:
                filepath = os.path.join(root, file)
                rel_path = os.path.relpath(filepath, cwd)
                out.write(f'### {rel_path}\n\n')
                ext = file.split('.')[-1] if '.' in file else 'text'
                if ext == 'rs': lang = 'rust'
                elif ext in ['ts', 'tsx']: lang = 'typescript'
                elif ext in ['js', 'jsx']: lang = 'javascript'
                else: lang = ext
                out.write(f'`{lang}\n')
                try:
                    with open(filepath, 'r', encoding='utf-8') as f:
                        content = f.read()
                        if len(content.splitlines()) > 300:
                            out.write('// File truncated due to length. Contains main logic for ' + file + '\n')
                            out.write('\n'.join(content.splitlines()[:50]))
                            out.write('\n...\n')
                            out.write('\n'.join(content.splitlines()[-50:]))
                        else:
                            out.write(content)
                except Exception as e:
                    out.write(f'// Error reading file: {e}')
                out.write('\n`\n\n')
                
    # 3. Dependencies
    out.write('## 3. Dependencies\n\n')
    for t in cargo_tomls + package_jsons:
        p = os.path.join(cwd, t)
        if os.path.exists(p):
            out.write(f'### {t}\n\n')
            ext = 'toml' if t.endswith('.toml') else 'json'
            out.write(f'`{ext}\n')
            with open(p, 'r', encoding='utf-8') as f:
                out.write(f.read())
            out.write('\n`\n\n')

