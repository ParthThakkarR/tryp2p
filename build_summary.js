const fs = require('fs');
const path = require('path');
const cp = require('child_process');

const cwd = 'd:/tryp2p';
const outputFile = 'C:/Users/parth/.gemini/antigravity-ide/brain/bf1e8cbf-8463-4726-b5ac-edcc8c82b3eb/technical_summary.md';
const targets = [
    'p2ptransfer-core/src',
    'desktop/p2ptransfer-cli/src',
    'desktop/p2ptransfer-gui/src',
    'desktop/p2ptransfer-gui/src-tauri/src'
];
const configFiles = [
    'Cargo.toml',
    'p2ptransfer-core/Cargo.toml',
    'desktop/p2ptransfer-cli/Cargo.toml',
    'desktop/p2ptransfer-gui/src-tauri/Cargo.toml',
    'desktop/p2ptransfer-gui/package.json'
];

let out = '# Technical Summary\n\n## 1. Project Structure\n\n`	ext\n';

function walkDir(dir, prefix) {
    const files = fs.readdirSync(dir);
    for (const file of files) {
        if (['target', 'node_modules', '.git', 'dist', 'build'].includes(file)) continue;
        const full = path.join(dir, file);
        const stat = fs.statSync(full);
        if (stat.isDirectory()) {
            if (full.endsWith('src-tauri\\\\target')) continue;
            out += prefix + file + '/\n';
            walkDir(full, prefix + '  ');
        } else {
            out += prefix + file + '\n';
        }
    }
}
walkDir(cwd, '');
out += '`\n\n## 2. File Contents\n\n';

for (const t of targets) {
    const p = path.join(cwd, t);
    if (!fs.existsSync(p)) continue;
    
    function walkFiles(dir) {
        const files = fs.readdirSync(dir);
        for (const file of files) {
            const full = path.join(dir, file);
            if (fs.statSync(full).isDirectory()) {
                walkFiles(full);
            } else {
                const rel = path.relative(cwd, full).replace(/\\\\/g, '/');
                out += '### ' + rel + '\n\n';
                const ext = path.extname(file).replace('.', '');
                let lang = ext;
                if (ext === 'rs') lang = 'rust';
                if (['ts', 'tsx'].includes(ext)) lang = 'typescript';
                if (['js', 'jsx'].includes(ext)) lang = 'javascript';
                
                out += '`' + lang + '\n';
                const content = fs.readFileSync(full, 'utf8');
                if (content.length > 25000) {
                    out += '// File truncated due to length. Contains main logic for ' + file + '\n';
                    const lines = content.split('\n');
                    out += lines.slice(0, 100).join('\n') + '\n...(truncated)...\n' + lines.slice(-50).join('\n') + '\n';
                } else {
                    out += content + '\n';
                }
                out += '`\n\n';
            }
        }
    }
    walkFiles(p);
}

out += '## 3. Dependencies\n\n';
for (const c of configFiles) {
    const full = path.join(cwd, c);
    if (fs.existsSync(full)) {
        const rel = c.replace(/\\\\/g, '/');
        out += '### ' + rel + '\n\n';
        const lang = c.endsWith('.toml') ? 'toml' : 'json';
        out += '`' + lang + '\n';
        out += fs.readFileSync(full, 'utf8') + '\n';
        out += '`\n\n';
    }
}

out += '## 4. Current State\n\n';
out += '**Last Command Ran:** git status; git log --oneline -5; cargo check 2>&1\n';
out += '**Compiler Status:** cargo check failed for getrandom due to a MinGW dlltool error (Invalid bfd target). This occurs on Windows MSYS environments when PATH is not perfectly set. The actual source code compiles successfully under cargo build when MSYS PATH is prefixed.\n\n';

out += '## 5. Git Status\n\n`	ext\n';
try { out += cp.execSync('git status', {cwd}).toString() + '\n'; } catch(e){}
try { out += cp.execSync('git log --oneline -5', {cwd}).toString() + '\n'; } catch(e){}
out += '`\n';

fs.writeFileSync(outputFile, out);
