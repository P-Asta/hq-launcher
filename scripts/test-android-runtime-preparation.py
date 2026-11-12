"""Focused, offline checks for the APK-to-library preparation boundary."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import struct
import subprocess
import sys
import tempfile
import unittest
import warnings
from zipfile import ZipFile, ZipInfo


SCRIPT = Path(__file__).with_name('prepare-android-runtime.py')
sys.dont_write_bytecode = True


def arm64_elf():
    data = bytearray(64)
    data[:6] = b'\x7fELF\x02\x01'
    struct.pack_into('<HH', data, 16, 3, 183)
    return bytes(data)


class RuntimePreparationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='hq-runtime-test-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.apk = self.root / 'donor.apk'
        self.output = self.root / 'prepared'

    def implementation(self):
        self.assertTrue(SCRIPT.is_file(), 'Runtime preparation is not implemented yet')
        spec = importlib.util.spec_from_file_location('hq_runtime_preparation', SCRIPT)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def fixture(self, extra=(), omit=(), native=None):
        entries = {
            'classes.dex': b'private code must not be copied',
            'assets/dexopt/baseline.prof': b'old package profile',
            'assets/linuxfs/usr/local/bin/droiddeck-session': b'#!/bin/sh\n',
            'assets/licenses/COPYING': b'license text',
            'lib/arm64-v8a/libproot.so': native or arm64_elf(),
            'lib/arm64-v8a/libopencv_java4.so': arm64_elf(),
        }
        with ZipFile(self.apk, 'w') as archive:
            for name, data in entries.items():
                if name not in omit:
                    archive.writestr(name, data)
            with warnings.catch_warnings():
                warnings.simplefilter('ignore', UserWarning)
                for name, data in extra:
                    archive.writestr(name, data)
        # Python's Windows ZIP writer normalizes backslashes. Put the hostile
        # bytes into both ZIP headers so this fixture represents an input ZIP.
        for name, _ in extra:
            if isinstance(name, str) and '\\' in name:
                self.apk.write_bytes(self.apk.read_bytes().replace(name.replace('\\', '/').encode(), name.encode()))
        return {
            'version': 1,
            'source': {'commit': 'a' * 40},
            'donor': {'sha256': hashlib.sha256(self.apk.read_bytes()).hexdigest(), 'size': self.apk.stat().st_size},
            'required_native_libraries': ['libproot.so', 'libopencv_java4.so'],
            'aar_owned_libraries': {'libopencv_java4.so': 'org.opencv:opencv:4.13.0'},
            'required_assets': ['linuxfs/usr/local/bin/droiddeck-session', 'licenses/COPYING'],
        }

    def test_extracts_runtime_and_licenses_without_dex_or_duplicate_aar_jni(self):
        lock = self.fixture()
        provenance = self.implementation().prepare_payload(self.apk, self.output, lock)
        self.assertEqual((self.output / 'jniLibs/arm64-v8a/libproot.so').read_bytes(), arm64_elf())
        self.assertEqual((self.output / 'assets/licenses/COPYING').read_bytes(), b'license text')
        self.assertFalse((self.output / 'jniLibs/arm64-v8a/libopencv_java4.so').exists())
        self.assertFalse(list(self.output.rglob('*.dex')))
        self.assertFalse((self.output / 'assets/dexopt').exists())
        self.assertIn('lib/arm64-v8a/libopencv_java4.so', provenance['donor_files'])
        self.assertEqual(provenance['source_commit'], 'a' * 40)

    def test_wrong_donor_digest_is_rejected_before_writing(self):
        lock = self.fixture()
        lock['donor']['sha256'] = '0' * 64
        with self.assertRaisesRegex(ValueError, 'checksum'):
            self.implementation().prepare_payload(self.apk, self.output, lock)
        self.assertFalse(self.output.exists())

    def source_fixture(self):
        source = self.root / 'source'
        java = source / 'app/src/main/java'
        java.mkdir(parents=True)
        (java / 'Fixture.kt').write_text('class Fixture\n', encoding='utf-8')
        (java.parent / 'AndroidManifest.xml').write_text(
            '<manifest xmlns:android="http://schemas.android.com/apk/res/android"><application /></manifest>',
            encoding='utf-8')
        (source / 'LICENSE').write_text('GPL fixture\n', encoding='utf-8')
        return source

    def prepare_full_fixture(self, implementation, lock, source, apk=None):
        self.assertTrue(callable(getattr(implementation, 'prepare_runtime', None)), 'Idempotent runtime preparation is not implemented yet')
        return implementation.prepare_runtime(apk or self.apk, self.output, source, lock)

    def test_repeated_valid_preparation_preserves_every_output_mtime(self):
        lock, source = self.fixture(), self.source_fixture()
        implementation = self.implementation()
        first = self.prepare_full_fixture(implementation, lock, source)
        files = [p for p in self.output.rglob('*') if p.is_file()]
        for p in files:
            os.utime(p, ns=(1500000000000000000, 1500000000000000000))
        before = {p.relative_to(self.output).as_posix(): p.stat().st_mtime_ns for p in files}
        second = self.prepare_full_fixture(implementation, lock, source)
        after = {p.relative_to(self.output).as_posix(): p.stat().st_mtime_ns
                 for p in self.output.rglob('*') if p.is_file()}
        self.assertEqual(second, first)
        self.assertEqual(after, before)

    def test_valid_cache_does_not_allow_a_wrong_custom_donor(self):
        lock, source = self.fixture(), self.source_fixture()
        implementation = self.implementation()
        self.prepare_full_fixture(implementation, lock, source)
        marker = self.output / 'preparation.json'
        before = marker.read_bytes(), marker.stat().st_mtime_ns
        wrong = self.root / 'wrong-custom.apk'
        wrong.write_bytes(self.apk.read_bytes() + b'wrong donor')
        with self.assertRaisesRegex(ValueError, 'checksum'):
            self.prepare_full_fixture(implementation, lock, source, wrong)
        self.assertEqual((marker.read_bytes(), marker.stat().st_mtime_ns), before)

    def test_incomplete_preparation_without_manifest_and_license_is_not_ready(self):
        lock = self.fixture()
        implementation = self.implementation()
        implementation.prepare_payload(self.apk, self.output, lock)
        with self.assertRaisesRegex(ValueError, 'manifest.*license'):
            implementation.verify_prepared(self.output, lock)

    def test_archive_paths_cannot_escape_or_alias_the_destination(self):
        implementation = self.implementation()
        for bad in ['assets/../../escape', '/assets/escape', 'assets\\escape', 'assets/C:/escape',
                    'assets/./escape', 'assets//escape', 'assets/CON']:
            with self.subTest(path=bad):
                lock = self.fixture(extra=[(bad, b'bad')])
                with self.assertRaisesRegex(ValueError, 'Unsafe archive'):
                    implementation.prepare_payload(self.apk, self.output, lock)
                self.assertFalse((self.root / 'escape').exists())

    def test_duplicate_entries_are_rejected(self):
        lock = self.fixture(extra=[('lib/arm64-v8a/libproot.so', arm64_elf())])
        with self.assertRaisesRegex(ValueError, 'Duplicate'):
            self.implementation().prepare_payload(self.apk, self.output, lock)

    def test_archive_symlinks_are_rejected(self):
        link = ZipInfo('assets/linked')
        link.create_system = 3
        link.external_attr = (stat.S_IFLNK | 0o777) << 16
        lock = self.fixture(extra=[(link, b'../../escape')])
        with self.assertRaisesRegex(ValueError, 'regular'):
            self.implementation().prepare_payload(self.apk, self.output, lock)

    def test_missing_proot_is_rejected_before_writing(self):
        lock = self.fixture(omit=['lib/arm64-v8a/libproot.so'])
        with self.assertRaisesRegex(ValueError, 'Missing.*libproot'):
            self.implementation().prepare_payload(self.apk, self.output, lock)
        self.assertFalse(self.output.exists())

    def test_native_library_with_wrong_abi_is_rejected(self):
        data = bytearray(arm64_elf())
        struct.pack_into('<H', data, 18, 62)
        lock = self.fixture(native=data)
        with self.assertRaisesRegex(ValueError, 'ARM64 ELF'):
            self.implementation().prepare_payload(self.apk, self.output, lock)

    def test_existing_checkout_with_wrong_commit_is_not_silently_changed(self):
        source = self.root / 'source'
        subprocess.run(['git', 'init', '--quiet', str(source)], check=True)
        (source / 'fixture.txt').write_text('source\n')
        subprocess.run(['git', '-C', str(source), 'add', '.'], check=True)
        subprocess.run(['git', '-C', str(source), '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                        'commit', '--quiet', '-m', 'fixture'], check=True)
        actual = subprocess.check_output(['git', '-C', str(source), 'rev-parse', 'HEAD']).decode().strip()
        with self.assertRaisesRegex(ValueError, 'commit'):
            self.implementation().verify_source(source, {'commit': '0' * 40, 'repository': 'unused'}, bootstrap=False)
        self.assertEqual(subprocess.check_output(['git', '-C', str(source), 'rev-parse', 'HEAD']).decode().strip(), actual)

    def test_untracked_java_cannot_be_injected_into_the_pinned_checkout(self):
        source = self.root / 'source'
        subprocess.run(['git', 'init', '--quiet', str(source)], check=True)
        (source / 'fixture.txt').write_text('source\n')
        subprocess.run(['git', '-C', str(source), 'add', '.'], check=True)
        subprocess.run(['git', '-C', str(source), '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                        'commit', '--quiet', '-m', 'fixture'], check=True)
        actual = subprocess.check_output(['git', '-C', str(source), 'rev-parse', 'HEAD']).decode().strip()
        injected = source / 'app/src/main/java/Injected.java'
        injected.parent.mkdir(parents=True)
        injected.write_text('class Injected {}')
        with self.assertRaisesRegex(ValueError, 'Untracked'):
            self.implementation().verify_source(source, {'commit': actual, 'repository': 'unused'}, bootstrap=False)

    def test_library_manifest_keeps_only_internal_surfaces_and_own_log_provider(self):
        xml = '''<manifest xmlns:android="http://schemas.android.com/apk/res/android"><application
            android:name=".App" android:label="Old" android:theme="@style/Old">
            <activity android:name=".SessionActivity" android:exported="true"><intent-filter /></activity>
            <activity-alias android:name=".Launcher" android:targetActivity=".SessionActivity" />
            <provider android:name="androidx.core.content.FileProvider" android:authorities="${applicationId}.logs" />
            </application></manifest>'''
        from xml.etree import ElementTree as ET
        root = ET.fromstring(self.implementation().sanitized_manifest(xml))
        app = root.find('application')
        key = '{http://schemas.android.com/apk/res/android}'
        self.assertNotIn(key + 'name', app.attrib)
        self.assertNotIn(key + 'label', app.attrib)
        self.assertNotIn(key + 'theme', app.attrib)
        self.assertEqual(app.get(key + 'extractNativeLibs'), 'true')
        self.assertFalse(app.findall('activity-alias'))
        self.assertFalse(list(app.iter('intent-filter')))
        self.assertEqual(app.find('activity').get(key + 'exported'), 'false')
        self.assertEqual(app.find('activity').get(key + 'name'), 'com.droiddeck.launcher.SessionActivity')
        self.assertEqual(app.find('provider').get(key + 'name'), 'com.droiddeck.launcher.hq.RuntimeLogProvider')

    def test_notification_patch_changes_staged_copy_and_preserves_pinned_source(self):
        source = self.root / 'java'
        source.mkdir()
        original = source / 'RuntimeInstallService.kt'
        original.write_text('Intent(this, MainActivity::class.java)\n', encoding='utf-8')
        patch = self.root / 'notification.json'
        patch.write_text(json.dumps({'source': 'RuntimeInstallService.kt', 'replacements': [{
            'before': 'Intent(this, MainActivity::class.java)',
            'after': 'packageManager.getLaunchIntentForPackage(packageName)',
        }]}), encoding='utf-8')
        staged = self.root / 'patched-java'
        implementation = self.implementation()
        self.assertTrue(callable(getattr(implementation, 'stage_java_sources', None)), 'Source patch staging is not implemented yet')
        details = implementation.stage_java_sources(source, staged, [patch])
        self.assertEqual(original.read_text(), 'Intent(this, MainActivity::class.java)\n')
        self.assertEqual((staged / 'RuntimeInstallService.kt').read_text(),
                         'packageManager.getLaunchIntentForPackage(packageName)\n')
        self.assertNotEqual(details[0]['before_sha256'], details[0]['after_sha256'])

    def test_documents_provider_retains_android_export_and_permission_contract(self):
        from xml.etree import ElementTree as ET
        xml = '''<manifest xmlns:android="http://schemas.android.com/apk/res/android"><application>
            <provider android:name=".files.AppFilesProvider" android:authorities="${applicationId}.documents"
                android:exported="true" android:permission="android.permission.MANAGE_DOCUMENTS"
                android:grantUriPermissions="true">
                <intent-filter><action android:name="android.content.action.DOCUMENTS_PROVIDER" /></intent-filter>
            </provider>
            <provider android:name=".agent.AgentBridgeProvider" android:exported="true" />
            </application></manifest>'''
        root = ET.fromstring(self.implementation().sanitized_manifest(xml))
        key = '{http://schemas.android.com/apk/res/android}'
        documents, agent = root.find('application').findall('provider')
        self.assertEqual(documents.get(key + 'name'), 'com.droiddeck.launcher.files.AppFilesProvider')
        self.assertEqual(documents.get(key + 'exported'), 'true')
        self.assertEqual(documents.get(key + 'permission'), 'android.permission.MANAGE_DOCUMENTS')
        self.assertEqual(documents.get(key + 'grantUriPermissions'), 'true')
        self.assertEqual(documents.get(key + 'authorities'), '${applicationId}.documents')
        self.assertEqual(documents.find('intent-filter/action').get(key + 'name'),
                         'android.content.action.DOCUMENTS_PROVIDER')
        self.assertEqual(agent.get(key + 'exported'), 'false')


if __name__ == '__main__':
    unittest.main(verbosity=2)
