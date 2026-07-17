import re

with open('desktop/p2ptransfer-cli/src/main.rs', 'r', encoding='utf-8') as f:
    content = f.read()

with open('patch_body.rs', 'r', encoding='utf-8') as f:
    patch = f.read()

start_marker = r'    // --- Signal handling for graceful pause ---'
end_marker = r'Ok\(\)\)\n\}'

# Find start of cmd_send
idx = content.find('async fn cmd_send(')
if idx == -1:
    print('cmd_send not found')
    exit(1)

# Find start marker after cmd_send
start_idx = content.find(start_marker, idx)
if start_idx == -1:
    print('start marker not found')
    exit(1)

# Find end marker after start marker
match = re.search(end_marker, content[start_idx:])
if not match:
    print('end marker not found')
    exit(1)

end_idx = start_idx + match.end()

new_content = content[:start_idx] + patch + content[end_idx:]

with open('desktop/p2ptransfer-cli/src/main.rs', 'w', encoding='utf-8') as f:
    f.write(new_content)

print('Patched successfully')