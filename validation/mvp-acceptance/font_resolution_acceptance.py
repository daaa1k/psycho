import argparse
import csv
import json
import os
import subprocess
from pathlib import Path

from gui_session import Gui, root


def build(out, label):
    environment = dict(os.environ, CC='/usr/bin/clang', CXX='/usr/bin/clang++')
    environment['PATH'] = '/usr/bin:/bin:/usr/sbin:/sbin:' + environment['PATH']
    with (out / label).open('w') as log:
        subprocess.run([str(Path.home() / '.cargo/bin/cargo'), 'clean', '-p', 'gpui_macos'],
                       cwd=root, env=environment, stdout=log, stderr=subprocess.STDOUT, check=True)
        subprocess.run([str(Path.home() / '.cargo/bin/cargo'), 'build', '--features', 'gui', '--bin', 'psycho'],
                       cwd=root, env=environment, stdout=log, stderr=subprocess.STDOUT, check=True)


args = argparse.ArgumentParser()
args.add_argument('--output', required=True)
args = args.parse_args()
out = (root / args.output).resolve()
assert out.is_relative_to(root / 'target') and not out.exists(), 'Use a fresh target directory'
out.mkdir(parents=True)
metadata = json.loads(subprocess.check_output([str(Path.home() / '.cargo/bin/cargo'), 'metadata', '--features', 'gui', '--locked', '--format-version', '1'], cwd=root))
package = next(p for p in metadata['packages'] if p['name'] == 'gpui_macos')
assert '14dd03e' in package['source'], 'Probe is tied to the recorded GPUI revision'
backend = Path(package['manifest_path']).parent / 'src/text_system.rs'
original = backend.read_bytes()
needle = '            let font_id = self.id_for_native_font(font);\n'
assert original.decode().count(needle) == 1
probe = '''            if let Some(path) = std::env::var_os("PSYCHO_FONT_RESOLUTION_TRACE") {
                use std::io::Write;
                if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                    let text_hex = text.as_bytes().iter().map(|byte| format!("{byte:02x}")).collect::<String>();
                    let _ = writeln!(file, "{}\\t{}\\t{}\\t{}", font_id.0, self.postscript_names_by_font_id[&font_id], f32::from(font_size), text_hex);
                }
            }
'''
(out / 'backend-probe.txt').write_text(probe)
gui = None
prior = os.environ.get('PSYCHO_FONT_RESOLUTION_TRACE')
try:
    backend.write_text(original.decode().replace(needle, needle + probe))
    build(out, 'build-probe.txt')
    os.environ['PSYCHO_FONT_RESOLUTION_TRACE'] = str(out / 'font-runs.tsv')
    image = os.path.relpath(root / 'assets/build-time.png', out / 'scratch')
    source = (Path(__file__).with_name('layout.kdl')).read_text().replace('../../assets/build-time.png', image)
    gui = Gui(out / 'scratch', source)
    gui.snapshot('equal-columns')
    gui.select_slide(2)
    gui.snapshot('unequal-columns')
    rows = list(csv.reader((out / 'font-runs.tsv').open(), delimiter='\t'))
    names = sorted({row[1] for row in rows})
    roles = {}
    for role, prefix in [('heading', '日本語と English の配置比較'), ('body', '日本語と English が混在'), ('code', 'println!'), ('caption', '画像の Caption')]:
        matching = [row for row in rows if prefix in bytes.fromhex(row[3]).decode()]
        assert matching, (role, names)
        roles[role] = sorted({row[1] for row in matching})
    revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
    (out / 'font-resolution.json').write_text(json.dumps(dict(source_commit=revision, gpui=package['source'], roles=roles, all_names=names,
        measurement='Resolved CTFont from each actual GPUI CoreText glyph run. Probe only appends font ID, PostScript name, size and text bytes.'), ensure_ascii=False, indent=2))
    print(json.dumps(roles, ensure_ascii=False))
finally:
    if gui is not None:
        gui.stop()
    if prior is None:
        os.environ.pop('PSYCHO_FONT_RESOLUTION_TRACE', None)
    else:
        os.environ['PSYCHO_FONT_RESOLUTION_TRACE'] = prior
    backend.write_bytes(original)
    assert backend.read_bytes() == original
    build(out, 'build-restored.txt')
print('PASS actual GPUI font names; dependency and binary restored')
