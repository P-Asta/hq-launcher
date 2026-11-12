"""Prepare HQ's private Android runtime from an explicitly pinned local APK.

Only runtime assets and ARM64 native libraries are reused. DEX, APK resources,
signatures, application data, accounts, and saves are never copied. The matching
GPL source checkout supplies Java/Kotlin/resources and license notices.
"""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import stat
import struct
import subprocess
import tempfile
from xml.etree import ElementTree as ET
from zipfile import ZipFile


ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / 'android/runtime/runtime.lock.json'
LOCAL = ROOT / '.android-runtime.local'
ANDROID = '{http://schemas.android.com/apk/res/android}'
TOOLS = '{http://schemas.android.com/tools}'
OWNER = 'hq-launcher-android-runtime'
ET.register_namespace('android', ANDROID[1:-1])
ET.register_namespace('tools', TOOLS[1:-1])


def digest(path):
    value = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            value.update(block)
    return value.hexdigest()


def git(source, *arguments):
    return subprocess.check_output(['git', '-C', str(source), *arguments], text=True).strip()


def verify_source(source, pin, bootstrap=True):
    if not source.exists():
        if not bootstrap:
            raise ValueError('Pinned runtime source checkout is missing; run prepare-android-runtime.py')
        source.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(['git', 'clone', '--filter=blob:none', '--no-checkout', pin['repository'], str(source)], check=True)
        subprocess.run(['git', '-C', str(source), 'fetch', 'origin', pin['commit']], check=True)
        subprocess.run(['git', '-C', str(source), 'checkout', '--detach', pin['commit']], check=True)
    actual = git(source, 'rev-parse', 'HEAD')
    if actual != pin['commit']:
        raise ValueError(f'Runtime source commit mismatch: expected {pin["commit"]}, found {actual}')
    if git(source, 'status', '--porcelain', '--untracked-files=no'):
        raise ValueError('Runtime source checkout has tracked modifications; restore the pinned source first')
    if git(source, 'ls-files', '--others', '--exclude-standard', '--', 'app/src/main/java', 'app/src/main/res'):
        raise ValueError('Untracked Java/Kotlin/resources in runtime checkout; the library must use only pinned source')
    return actual


def validate_name(name):
    # ZIP uses POSIX separators; reject Windows aliases as well as traversal.
    raw = name[:-1] if name.endswith('/') else name
    parts = raw.split('/')
    devices = {'CON', 'PRN', 'AUX', 'NUL'} | {f'{p}{n}' for p in ('COM', 'LPT') for n in range(1, 10)}
    if (not raw or name.startswith('/') or '\\' in name or
            any(part in ('', '.', '..') or part.rstrip(' .') != part or
                part.split('.')[0].upper() in devices or any(c in part for c in ':<>"|?*\x00') for part in parts)):
        raise ValueError('Unsafe archive member: ' + name)
    return PurePosixPath(raw)


def sanitized_manifest(xml):
    root = ET.fromstring(xml)
    for permission in list(root.findall('uses-permission')):
        if permission.get(ANDROID + 'name') == 'android.permission.REQUEST_INSTALL_PACKAGES':
            root.remove(permission)  # HQ owns its updates.
    app = root.find('application')
    if app is None:
        raise ValueError('Runtime source manifest has no application')
    app.attrib.clear()
    app.set(ANDROID + 'extractNativeLibs', 'true')
    app.set(ANDROID + 'largeHeap', 'true')
    for alias in list(app.findall('activity-alias')):
        app.remove(alias)
    for component in app:
        documents_provider = False
        if component.tag in ('activity', 'service', 'provider', 'receiver'):
            name = component.get(ANDROID + 'name', '')
            if name.startswith('.'):
                component.set(ANDROID + 'name', 'com.droiddeck.launcher' + name)
            if name == 'androidx.core.content.FileProvider':
                component.set(ANDROID + 'name', 'com.droiddeck.launcher.hq.RuntimeLogProvider')
            documents_provider = (component.tag == 'provider' and component.get(ANDROID + 'name') ==
                                  'com.droiddeck.launcher.files.AppFilesProvider')
            if documents_provider:
                # DocumentsProvider.attachInfo requires this protected export.
                component.set(ANDROID + 'exported', 'true')
                component.set(ANDROID + 'permission', 'android.permission.MANAGE_DOCUMENTS')
                component.set(ANDROID + 'grantUriPermissions', 'true')
            else:
                component.set(ANDROID + 'exported', 'false')
        if not documents_provider:
            for intent in list(component.findall('intent-filter')):
                component.remove(intent)
    ET.indent(root, space='    ')
    return '<?xml version="1.0" encoding="utf-8"?>\n' + ET.tostring(root, encoding='unicode') + '\n'


def verify_donor(apk, lock):
    if not apk.is_file():
        raise ValueError(f'Local runtime donor APK is missing: {apk}. Supply the pinned APK documented in android/runtime/README.md.')
    if digest(apk) != lock['donor']['sha256'] or apk.stat().st_size != lock['donor']['size']:
        raise ValueError('Runtime donor checksum or size mismatch; no files were extracted')


def lock_digest(lock):
    return hashlib.sha256(json.dumps(lock, sort_keys=True).encode('utf-8')).hexdigest()


def prepare_payload(apk, destination, lock):
    verify_donor(apk, lock)
    required_native = {'lib/arm64-v8a/' + n for n in lock['required_native_libraries']}
    required_assets = {'assets/' + n for n in lock['required_assets']}
    payload = {}
    donor_files = {}
    with ZipFile(apk) as archive:
        names = set()
        for info in archive.infolist():
            validate_name(info.orig_filename)
            if info.filename in names:
                raise ValueError('Duplicate archive member: ' + info.filename)
            names.add(info.filename)
            kind = stat.S_IFMT(info.external_attr >> 16)
            if kind not in (0, stat.S_IFREG, stat.S_IFDIR):
                raise ValueError('Archive member is not a regular file or directory: ' + info.filename)
            if info.is_dir():
                continue
            name = info.filename
            if not name.startswith(('assets/', 'lib/arm64-v8a/')):
                continue
            if name.startswith('assets/dexopt/') or name.lower().endswith('.dex'):
                continue
            data = archive.read(info)
            donor_files[name] = {'sha256': hashlib.sha256(data).hexdigest(), 'size': len(data)}
            if name.startswith('lib/'):
                if (len(data) < 64 or data[:6] != b'\x7fELF\x02\x01' or
                        struct.unpack_from('<H', data, 18)[0] != 183):
                    raise ValueError('Native input is not an ARM64 ELF: ' + name)
                if name.rsplit('/', 1)[1] in lock.get('aar_owned_libraries', {}):
                    continue
                target = 'jniLibs/arm64-v8a/' + name.rsplit('/', 1)[1]
            else:
                target = name
            payload[target] = data
        missing = (required_native | required_assets) - donor_files.keys()
        if missing:
            raise ValueError('Missing required runtime inputs: ' + ', '.join(sorted(missing)))
        unexpected = {name for name in donor_files if name.startswith('lib/')} - required_native
        if unexpected:
            raise ValueError('Unexpected native inputs outside the lock: ' + ', '.join(sorted(unexpected)))
    provenance = {
        'owner': OWNER,
        'version': 1,
        'source_commit': lock['source']['commit'],
        'runtime_lock_sha256': lock_digest(lock),
        'preparation_script_sha256': digest(Path(__file__)),
        'donor_sha256': lock['donor']['sha256'],
        'aar_owned_libraries': lock.get('aar_owned_libraries', {}),
        'donor_files': donor_files,
        'outputs': {name: hashlib.sha256(data).hexdigest() for name, data in sorted(payload.items())},
    }
    destination = destination.absolute()
    if destination.is_symlink():
        raise ValueError('Prepared runtime directory must not be a symlink')
    if destination.exists():
        marker = destination / 'preparation.json'
        if not marker.is_file() or json.loads(marker.read_text(encoding='utf-8')).get('owner') != OWNER:
            raise ValueError('Refusing to replace a directory not owned by runtime preparation')
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='hq-runtime-prepare-', dir=destination.parent) as temporary:
        staging = Path(temporary) / 'next'
        staging.mkdir()
        for name, data in payload.items():
            path = staging / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        (staging / 'preparation.json').write_text(json.dumps(provenance, indent=2, sort_keys=True) + '\n', encoding='utf-8')
        if destination.exists():
            destination.rename(Path(temporary) / 'previous')
        staging.rename(destination)
    return provenance


def stage_java_sources(source, destination, patches):
    shutil.copytree(source, destination)
    applied = []
    for patch_path in patches:
        patch = json.loads(patch_path.read_text(encoding='utf-8'))
        relative = validate_name(patch['source'])
        target = destination / relative
        before = digest(target)
        text = target.read_text(encoding='utf-8')
        for replacement in patch['replacements']:
            if text.count(replacement['before']) != 1:
                raise ValueError('Pinned source patch does not match exactly once: ' + patch_path.name)
            text = text.replace(replacement['before'], replacement['after'])
        target.write_text(text, encoding='utf-8', newline='\n')
        applied.append({'source': patch['source'], 'patch': patch_path.name, 'patch_sha256': digest(patch_path),
                        'before_sha256': before, 'after_sha256': digest(target)})
    return applied


def stage_source_notices(source, output, lock, provenance):
    patches = [LOCK.parent / relative for relative in lock.get('source_patches', [])]
    provenance['source_patches'] = stage_java_sources(source / 'app/src/main/java', output / 'java', patches)
    manifest = output / 'AndroidManifest.xml'
    manifest.write_text(sanitized_manifest((source / 'app/src/main/AndroidManifest.xml').read_text(encoding='utf-8')), encoding='utf-8')
    notices = [(source / 'LICENSE', 'LICENSE')]
    notices += [(p, 'third-party/' + p.relative_to(source / 'docs/third-party').as_posix())
                for p in (source / 'docs/third-party').rglob('*') if p.is_file()]
    notices += [(p, 'native/' + p.relative_to(source / 'app/src/main/cpp').as_posix())
                for p in (source / 'app/src/main/cpp').rglob('*')
                if p.is_file() and ('LICENSE' in p.name.upper() or 'NOTICE' in p.name.upper())]
    for path, relative in notices:
        target = output / 'assets/licenses/droiddeck' / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, target)
    attribution = output / 'assets/licenses/droiddeck/SOURCE.json'
    attribution.write_text(json.dumps({'source': lock['source'], 'donor': lock['donor'],
                                       'modifications': 'Embedded runtime library; HQ owns application, UI and updates.'}, indent=2) + '\n', encoding='utf-8')
    provenance['outputs'] = {p.relative_to(output).as_posix(): digest(p)
                             for p in output.rglob('*') if p.is_file() and p.name != 'preparation.json'}
    (output / 'preparation.json').write_text(json.dumps(provenance, indent=2, sort_keys=True) + '\n', encoding='utf-8')


def verify_prepared(output, lock):
    marker = output / 'preparation.json'
    if not marker.is_file():
        raise ValueError('Runtime has not been prepared; run python scripts/prepare-android-runtime.py')
    provenance = json.loads(marker.read_text(encoding='utf-8'))
    if (provenance.get('owner') != OWNER or provenance.get('source_commit') != lock['source']['commit'] or
            provenance.get('donor_sha256') != lock['donor']['sha256'] or
            provenance.get('runtime_lock_sha256') != lock_digest(lock)):
        raise ValueError('Runtime preparation does not match the tracked lock')
    if provenance.get('preparation_script_sha256') != digest(Path(__file__)):
        raise ValueError('Runtime preparation script changed; run python scripts/prepare-android-runtime.py')
    current_patches = {Path(p).name: digest(LOCK.parent / p) for p in lock.get('source_patches', [])}
    if current_patches != {p['patch']: p['patch_sha256'] for p in provenance.get('source_patches', [])}:
        raise ValueError('Runtime source patches changed; run python scripts/prepare-android-runtime.py')
    required = {'AndroidManifest.xml', 'assets/licenses/droiddeck/LICENSE', 'assets/licenses/droiddeck/SOURCE.json'}
    if not required.issubset(provenance['outputs']):
        raise ValueError('Prepared runtime is missing its manifest or source license attribution')
    actual = {p.relative_to(output).as_posix(): digest(p)
              for p in output.rglob('*') if p.is_file() and p.name != 'preparation.json'}
    if actual != provenance['outputs']:
        raise ValueError('Prepared runtime inputs changed; run python scripts/prepare-android-runtime.py')
    return provenance


def prepare_runtime(apk, output, source, lock):
    # A custom --apk must be checked even when all prepared outputs are current.
    verify_donor(apk, lock)
    try:
        return verify_prepared(output, lock)
    except (ValueError, OSError, KeyError):
        pass
    provenance = prepare_payload(apk, output, lock)
    stage_source_notices(source, output, lock, provenance)
    return verify_prepared(output, lock)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--apk', type=Path, default=LOCAL / 'droiddeck-donor.apk')
    parser.add_argument('--verify-only', action='store_true')
    args = parser.parse_args()
    try:
        lock = json.loads(LOCK.read_text(encoding='utf-8'))
        source, output = LOCAL / 'DroidDeck', LOCAL / 'prepared'
        verify_source(source, lock['source'], bootstrap=not args.verify_only)
        if args.verify_only:
            verify_donor(args.apk.resolve(), lock)
            provenance = verify_prepared(output, lock)
        else:
            provenance = prepare_runtime(args.apk.resolve(), output, source, lock)
        print(f'HQ Android runtime verified: {len(provenance["donor_files"])} pinned donor files; source {lock["source"]["commit"][:12]}')
    except (ValueError, OSError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, str(error) + '\n')


if __name__ == '__main__':
    main()
