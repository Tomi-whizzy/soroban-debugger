import os
import re

def fix_duplicates(filename):
    with open(filename, 'r', encoding='utf-8') as f:
        content = f.read()
    
    # regex to find struct literals and their contents
    struct_pattern = re.compile(r'(DynamicTraceEvent\s*\{)(.*?)(\})', re.DOTALL)
    
    def repl_struct(match):
        prefix = match.group(1)
        body = match.group(2)
        suffix = match.group(3)
        
        lines = body.split('\n')
        seen_fields = set()
        new_lines = []
        for line in lines:
            field_match = re.search(r'^\s*([a-zA-Z0-9_]+):', line)
            if field_match:
                field = field_match.group(1)
                if field in seen_fields:
                    continue
                seen_fields.add(field)
            new_lines.append(line)
        return prefix + '\n'.join(new_lines) + suffix

    new_content = struct_pattern.sub(repl_struct, content)
    
    with open(filename, 'w', encoding='utf-8') as f:
        f.write(new_content)

if __name__ == '__main__':
    fix_duplicates('src/analyzer/security.rs')
    fix_duplicates('src/runtime/executor.rs')
