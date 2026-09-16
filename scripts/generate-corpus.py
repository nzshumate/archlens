#!/usr/bin/env python3
import pathlib, sys
count = int(sys.argv[1]) if len(sys.argv) > 1 else 5000
root = pathlib.Path(sys.argv[2] if len(sys.argv) > 2 else '/tmp/archlens-corpus')
root.mkdir(parents=True, exist_ok=True)
for i in range(count):
    deps = []
    if i > 0: deps.append(f"import './module_{i-1}';")
    if i > 10 and i % 7 == 0: deps.append(f"import './module_{i-10}';")
    (root / f'module_{i}.ts').write_text('\n'.join(deps + [f'export const value{i} = {i};']) + '\n')
print(f'generated {count} TypeScript modules in {root}')
