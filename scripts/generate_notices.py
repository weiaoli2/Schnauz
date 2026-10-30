#!/usr/bin/env python3
"""Collect dependency notices from Cargo's locked Mac dependency graph."""
from pathlib import Path
import argparse
import hashlib
import json
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
PREFIXES = ('LICENSE', 'LICENCE', 'COPYING', 'NOTICE', 'UNLICENSE', 'COPYRIGHT', 'AUTHORS')

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', help='Fail if notices need regeneration')
    args=parser.parse_args()
    metadata=json.loads(subprocess.check_output([
        'cargo','metadata','--locked','--format-version','1',
        '--filter-platform','aarch64-apple-darwin'],cwd=ROOT))
    supplements={}
    for provenance in sorted((ROOT/'vendor/licenses/supplemental').glob('*/provenance.json')):
        entry=json.loads(provenance.read_text())
        files=[]
        for record in entry['files']:
            path=provenance.parent/record['file']
            data=path.read_bytes()
            if hashlib.sha256(data).hexdigest()!=record['sha256']:
                raise SystemExit('Supplemental notice digest mismatch: '+str(path.relative_to(ROOT)))
            files.append((str(path.relative_to(ROOT)),data.decode('utf-8',errors='replace')))
        for package in entry['packages']:
            supplements[(package['name'],package['version'])]=files

    sections=["""SCHNAUZ — THIRD-PARTY NOTICES

Schnauz is developed and designed by Oliver Weber with Codex implementation assistance.
Original Shuffle code: Copyright (c) 2026 Jaime Guzman, MIT license.
The original LICENSE and CREDITS.txt accompany the app.

This file lists the locked Mac dependency graph, including build-time tools.
Each component retains its own terms. OR expressions offer a choice of license;
including multiple texts does not change their original terms.
Cargo package authors are listed as supplied by each package.

Unmodified MPL-2.0 sources (option-ext 0.2.0 and build tool cbindgen 0.28.0)
are available at https://github.com/weiaoli2/Schnauz/tree/main/vendor/mpl-sources
and exact source archives accompany the published release downloads:
https://github.com/weiaoli2/Schnauz/releases
"""]
    texts={};licenses=[];missing=[]
    packages=sorted(metadata['packages'],key=lambda p:(p['name'],p['version']))
    for package in packages:
        if package['name']=='schnauz':continue
        base=Path(package['manifest_path']).parent
        files=[]
        for path in sorted(base.rglob('*')):
            if path.is_file() and path.name.upper().startswith(PREFIXES) and '.git' not in path.parts:
                files.append((str(path.relative_to(base)),path.read_text(errors='replace')))
        files.extend(supplements.get((package['name'],package['version']),[]))
        if not files:
            missing.append(package['name']+' '+package['version']);continue
        heading='\n'+'='*76+'\n'+package['name']+' '+package['version']+'\n'
        heading+='License: '+(package['license'] or 'See supplied license file')+'\n'
        heading+='Authors: '+('; '.join(package['authors']) or 'See supplied notices')+'\n'
        if package['repository']:heading+='Source: '+package['repository']+'\n'
        sections.append(heading)
        for name,content in files:
            digest=hashlib.sha256(content.encode()).hexdigest()
            if digest not in texts:
                identifier='TEXT-'+str(len(texts)+1).zfill(4)
                texts[digest]=identifier
                licenses.append('\n'+'-'*76+'\n'+identifier+'\n'+'-'*76+'\n'+content.rstrip()+'\n')
            sections.append('Notice: '+name+' → '+texts[digest]+'\n')
    if missing:raise SystemExit('Missing notices: '+', '.join(missing))
    content=''.join(sections)+'\nLICENSE AND NOTICE TEXTS\n'+''.join(licenses)
    output=ROOT/'THIRD_PARTY_NOTICES.txt'
    if args.check:
        if not output.exists() or output.read_text()!=content:raise SystemExit('Regenerate THIRD_PARTY_NOTICES.txt')
    else:output.write_text(content)
    print(f'Verified notices for {len(packages)-1} dependencies ({len(texts)} distinct texts).')

if __name__=='__main__':main()
