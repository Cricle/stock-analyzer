#!/usr/bin/env python3
"""
Convert all include!() patterns to proper mod declarations.
Handles: prelude merging, super:: path adjustment, pub(super) → pub(crate).
"""

import re
import os
import sys
import shutil

SRC_DIR = 'src'

def read_file(path):
    with open(path) as f:
        return f.read()

def write_file(path, content):
    with open(path, 'w') as f:
        f.write(content)

def find_includes(content):
    return re.findall(r'include!\("(.+?)"\)', content)

def extract_use_statements(content):
    uses = []
    for line in content.split('\n'):
        stripped = line.strip()
        if stripped.startswith('use ') and stripped.endswith(';'):
            uses.append(stripped)
    return uses

def adjust_super_refs(content, depth_change):
    """Adjust super:: references when nesting depth changes."""
    if depth_change == 0:
        return content
    # Replace super:: with additional super:: prefixes
    for _ in range(depth_change):
        content = content.replace('super::', 'super::super::')
    return content

def fix_pub_super(content):
    """Change pub(super) to pub(crate) for items that need wider visibility."""
    return content.replace('pub(super)', 'pub(crate)')

def get_parent_imports(path):
    """Get the use statements that the parent file had."""
    content = read_file(path)
    return extract_use_statements(content)

def convert_module(parent_path, dry_run=False):
    """Convert a single parent file from include!() to mod declarations."""
    content = read_file(parent_path)
    includes = find_includes(content)
    if not includes:
        return []

    parent_dir = os.path.dirname(parent_path)
    parent_uses = extract_use_statements(content)
    other_content = re.sub(r'^use\s+.*?;\s*\n', '', content, flags=re.MULTILINE)
    other_content = re.sub(r'^include!\(".*?"\);\s*\n', '', other_content, flags=re.MULTILINE)
    other_content = other_content.strip()

    results = []
    mod_declarations = []

    for inc in includes:
        inc_full = os.path.normpath(os.path.join(parent_dir, inc))
        mod_name = os.path.splitext(os.path.basename(inc))[0]

        if not os.path.exists(inc_full):
            print(f"  WARNING: {inc_full} not found")
            continue

        # Handle prelude/postlude: keep as include!()
        if 'prelude' in mod_name or 'postlude' in mod_name:
            # Merge prelude imports into the next non-prelude file
            if 'prelude' in mod_name:
                prelude_content = read_file(inc_full)
                prelude_uses = extract_use_statements(prelude_content)
                # Find the next include (the main file)
                idx = includes.index(inc)
                if idx + 1 < len(includes):
                    next_inc = includes[idx + 1]
                    next_full = os.path.normpath(os.path.join(parent_dir, next_inc))
                    if os.path.exists(next_full):
                        next_content = read_file(next_full)
                        next_uses = extract_use_statements(next_content)
                        # Merge: add prelude uses that aren't already in the file
                        new_uses = [u for u in prelude_uses if u not in next_uses]
                        if new_uses:
                            use_block = '\n'.join(new_uses) + '\n\n'
                            if not dry_run:
                                write_file(next_full, use_block + next_content)
                            results.append(f"  Merged {len(new_uses)} imports from {inc} into {next_inc}")
            mod_declarations.append(f'include!("{inc}");')
            continue

        # Regular file: convert to mod
        inc_content = read_file(inc_full)

        # Add parent imports if file has no imports
        inc_uses = extract_use_statements(inc_content)
        if not inc_uses and parent_uses:
            use_block = '\n'.join(parent_uses) + '\n\n'
            inc_content = use_block + inc_content

        # Fix super:: references (one level deeper now)
        inc_content = adjust_super_refs(inc_content, 1)

        # Fix pub(super) → pub(crate)
        inc_content = fix_pub_super(inc_content)

        if not dry_run:
            write_file(inc_full, inc_content)

        mod_declarations.append(f'pub(crate) mod {mod_name};')
        results.append(f"  Converted {inc} → mod {mod_name}")

    # Write new parent file
    new_lines = []
    if other_content:
        new_lines.append(other_content)
        new_lines.append('')
    new_lines.extend(mod_declarations)
    new_content = '\n'.join(new_lines) + '\n'

    if not dry_run:
        write_file(parent_path, new_content)

    return results

def collect_all_parents():
    """Find all files with include!(), sorted deepest first."""
    parents = []
    for root, dirs, files in os.walk(SRC_DIR):
        for f in sorted(files):
            if not f.endswith('.rs'):
                continue
            path = os.path.join(root, f)
            content = read_file(path)
            includes = find_includes(content)
            if includes:
                parents.append(path)
    # Sort by depth (deepest first for bottom-up processing)
    parents.sort(key=lambda x: x.count('/'), reverse=True)
    return parents

def main():
    dry_run = '--dry-run' in sys.argv

    parents = collect_all_parents()
    print(f"Found {len(parents)} files with include!()")

    for path in parents:
        content = read_file(path)
        includes = find_includes(content)
        print(f"\n{path} ({len(includes)} includes):")
        results = convert_module(path, dry_run)
        for r in results:
            print(r)

    if dry_run:
        print("\n[DRY RUN] No changes made.")
    else:
        print("\nDone! Run 'CARGO_INCREMENTAL=0 cargo test --no-run' to check.")

if __name__ == '__main__':
    main()
