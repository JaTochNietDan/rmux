#!/usr/bin/env python3
"""Extract factual cmux contract inventories at an immutable upstream revision.

Usage: python3 scripts/refresh_cmux_inventory.py --revision <40-character SHA>
Optionally pass --source-dir PATH to use an already downloaded reference tree.
Does not execute upstream code or overwrite the authored roadmap/feature matrix.
"""
import argparse
import hashlib
import json
import re
import urllib.parse
import urllib.request
from pathlib import Path

DEFAULT_REVISION = 'a76816e4f0788003758b9434ab702068a640fb5d'
PATHS = {
    'cli': 'docs/cli-command-tree.txt',
    'settings': 'web/data/cmux.schema.json',
    'shortcuts': 'web/data/cmux-shortcuts.ts',
    'rpc': 'Sources/TerminalController+Capabilities.swift',
}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--revision', default=DEFAULT_REVISION)
    parser.add_argument('--source-dir', type=Path)
    args = parser.parse_args()
    if not re.fullmatch(r'[0-9a-f]{40}', args.revision):
        parser.error('--revision must be a full lowercase commit SHA')
    root = Path(__file__).resolve().parents[1] / 'docs' / 'reference'
    root.mkdir(parents=True, exist_ok=True)
    manifest = {'repository': 'https://github.com/manaflow-ai/cmux', 'revision': args.revision, 'sources': []}
    source = {}
    for key, path in PATHS.items():
        url = f'https://raw.githubusercontent.com/manaflow-ai/cmux/{args.revision}/{urllib.parse.quote(path)}'
        data = (args.source_dir / path).read_bytes() if args.source_dir else urllib.request.urlopen(url, timeout=60).read()
        source[key] = data.decode()
        manifest['sources'].append({'path': path, 'sha256': hashlib.sha256(data).hexdigest(), 'url': f'https://github.com/manaflow-ai/cmux/blob/{args.revision}/{urllib.parse.quote(path)}'})
    def write(name, data):
        (root / name).write_text(json.dumps(data, ensure_ascii=False, indent=2) + '\n')
    commands = []
    current = None
    for number, line in enumerate(source['cli'].splitlines(), 1):
        match = re.fullmatch(r'command  (.*?)  aliases=(.*)', line)
        if match:
            current = {'command': match[1], 'aliases': [] if match[2] == '-' else match[2].split(','), 'source_line': number, 'internal_candidate': any(p.startswith('__') for p in match[1].split()), 'arguments': []}
            commands.append(current)
        elif current is not None and line.startswith('  arg  '):
            match = re.fullmatch(r'  arg  (.*?)  kind=(.*?)  completion=(.*)', line)
            if not match:
                raise ValueError(f'Unknown argument format at line {number}')
            current['arguments'].append(dict(zip(('name', 'kind', 'completion'), match.groups())))
    if not commands:
        raise ValueError('CLI inventory was empty; upstream format changed')
    write('cmux-cli-inventory.json', {'revision': args.revision, 'scope': 'All command records in the checked-in command tree, including grouping nodes, internal commands and aliases. Hand-parsed subcommands may not be expanded. This is not a runtime support claim.', 'count': len(commands), 'commands': commands})
    schema = json.loads(source['settings'])
    settings = []
    def walk(node, path, pointer):
        if not isinstance(node, dict): return
        if path:
            item = {'path': path, 'schema_pointer': pointer}
            for key in ('type', 'enum', 'default', 'minimum', 'maximum', '$ref', 'required'):
                if key in node: item[key] = node[key]
            settings.append(item)
        for key, value in node.get('properties', {}).items():
            walk(value, f'{path}.{key}' if path else key, pointer+'/properties/'+key.replace('~','~0').replace('/','~1'))
        for key, value in node.get('patternProperties', {}).items():
            walk(value, f'{path}.{{{key}}}', pointer+'/patternProperties/'+key.replace('~','~0').replace('/','~1'))
        if isinstance(node.get('additionalProperties'), dict):
            walk(node['additionalProperties'], path+'.*', pointer+'/additionalProperties')
        if isinstance(node.get('items'), dict): walk(node['items'], path+'[]', pointer+'/items')
        for key in ('oneOf', 'anyOf', 'allOf'):
            for i, child in enumerate(node.get(key, [])): walk(child, path+f'<{key}:{i}>', pointer+f'/{key}/{i}')
    walk(schema, '', '')
    for key, value in schema.get('$defs', {}).items(): walk(value, '$defs.'+key, '/$defs/'+key)
    write('cmux-settings-inventory.json', {'revision': args.revision, 'scope': 'Schema nodes, including containers, alternatives, wildcard entries and definitions; count is not the number of independent settings. Descriptions omitted; consult pinned source.', 'count': len(settings), 'settings': settings})
    # Literal IDs in the public shortcut catalog; category IDs are retained and identified.
    shortcut_ids = [{'id': m.group(1), 'source_line': source['shortcuts'].count('\n', 0, m.start())+1} for m in re.finditer(r'\bid:\s*"([^"]+)"', source['shortcuts'])]
    write('cmux-shortcut-inventory.json', {'revision': args.revision, 'scope': 'Literal action and category IDs from the public shortcut catalog; defaults remain in the linked source. Not every ID is a bindable action.', 'count': len(shortcut_ids), 'entries': shortcut_ids})
    # Static declared base plus explicit design-mode additions. Runtime feature filters,
    # simulator method set, and DEBUG additions are intentionally not guessed.
    match = re.search(r'var methods: \[String\] = \[(.*?)\n        \]', source['rpc'], re.S)
    if not match: raise ValueError('RPC capability declaration format changed')
    methods = sorted(set(re.findall(r'"([a-z_]+(?:\.[a-z_]+)+)"', match[1])) | {'browser.design_mode.set', 'browser.design_mode.status'})
    write('cmux-rpc-inventory.json', {'revision': args.revision, 'scope': 'Static base capability declarations plus explicit design-mode additions. Feature-gated methods may be unavailable; dynamic simulator and DEBUG lists excluded. Presence does not prove implementation or authorization.', 'count': len(methods), 'methods': methods})
    write('cmux-contract-sources.json', manifest)
    print(json.dumps({'commands': len(commands), 'schema_nodes': len(settings), 'shortcut_and_category_ids': len(shortcut_ids), 'static_rpc_methods': len(methods)}))

if __name__ == '__main__': main()
