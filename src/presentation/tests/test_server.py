"""Integration contracts for the outline reader and bounded execution API."""
import json
from pathlib import Path
import sys
import tempfile
import threading
import time
import unittest
from urllib.error import HTTPError
from urllib.request import Request, urlopen

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from content import read_outline
from server import Application, PresentationServer, ROOT, Runner, default_runtime


class OutlineTests(unittest.TestCase):
    def test_approved_outline_has_complete_fields_and_tables(self):
        meta, slides = read_outline(ROOT / 'docs/class-presentation.md')
        self.assertEqual(len(slides), meta['main_slides'] + meta['appendix_slides'])
        self.assertTrue(all(slide.layout and slide.blocks for slide in slides))
        self.assertTrue(any(slide.steps for slide in slides))
        self.assertTrue(any(block.rows for slide in slides for block in slide.blocks))

    def test_missing_slide_is_rejected(self):
        source = (ROOT / 'docs/class-presentation.md').read_text()
        source = source.replace('## 07｜', '## removed｜')
        with tempfile.TemporaryDirectory() as d:
            file = Path(d) / 'outline.md'
            file.write_text(source)
            with self.assertRaises(ValueError):
                read_outline(file)


class ApiTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = Application(ROOT / 'docs/class-presentation.md', None)
        cls.server = PresentationServer(('127.0.0.1', 0), cls.app)
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()
        cls.url = f'http://127.0.0.1:{cls.server.server_port}'

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join(2)

    def post(self, body, token=None, origin=None):
        headers = {'Content-Type':'application/json', 'X-Prim-Token':self.app.token if token is None else token}
        if origin:
            headers['Origin'] = origin
        request = Request(self.url + '/api/run', data=json.dumps(body).encode(), headers=headers)
        try:
            with urlopen(request, timeout=3) as response:
                return response.status, json.load(response)
        except HTTPError as error:
            return error.code, json.load(error)

    def test_deck_and_missing_runtime_are_explicit(self):
        with urlopen(self.url + '/api/deck') as response:
            deck = json.load(response)
            self.assertEqual(len(deck['slides']), deck['metadata']['main_slides'] + deck['metadata']['appendix_slides'])
            demos = {demo['id'] for demo in deck['demos']}
            self.assertTrue(all(slide['options'].get('demo', 'closures') in demos for slide in deck['slides']))
        with urlopen(self.url + '/api/status') as response:
            self.assertFalse(json.load(response)['available'])
        status, result = self.post({'code':'std.print(1);'})
        self.assertEqual(status, 400)
        self.assertIn('执行程序', result['error'])

    def test_cross_origin_and_stale_sessions_are_rejected(self):
        self.assertEqual(self.post({'code':'std.print(1);'}, token='wrong')[0], 403)
        self.assertEqual(self.post({'code':'std.print(1);'}, origin='https://example.invalid')[0], 403)

    def test_input_and_modes_are_validated(self):
        for body in ([], {'code':1}, {'code':''}, {'code':'x'*33000}, {'code':'1','mode':'shell'}):
            with self.subTest(body_type=type(body).__name__):
                self.assertEqual(self.post(body)[0], 400)

    def test_filesystem_is_not_exposed(self):
        with self.assertRaises(HTTPError) as error:
            urlopen(self.url + '/static/../../Cargo.toml')
        self.assertEqual(error.exception.code, 404)


@unittest.skipUnless(default_runtime(), 'No existing Prim executable; no build is attempted')
class RuntimeTests(unittest.TestCase):
    def setUp(self):
        self.work = tempfile.TemporaryDirectory(prefix='prim-runner-test-')
        self.runner = Runner(default_runtime(), Path(self.work.name))

    def tearDown(self):
        self.runner.cancel()
        self.work.cleanup()

    def test_actual_interpreter_and_jit_share_observable_state(self):
        source = 'let n=0; let a=$[n](){ n=n+1; n }; let b=jit(a); std.print(b()); std.print(a()); std.print(b());'
        interpreted = self.runner.run(source, 'interpreter')
        native = self.runner.run(source, 'jit')
        self.assertEqual(interpreted['stdout'], '1\n2\n3\n')
        self.assertEqual(native['stdout'], interpreted['stdout'])
        self.assertEqual(native['exit_code'], 0)
        self.assertIn('compiled=1 native_calls=2', native['stderr'])
        self.assertIn('compiled=0 native_calls=0', interpreted['stderr'])

    def test_error_does_not_replay_output(self):
        result = self.runner.run('let f=@jit $[](){std.print("once"); 1/0}; f();', 'jit')
        self.assertNotEqual(result['exit_code'], 0)
        self.assertEqual(result['stdout'], 'once\n')
        self.assertIn('DivisionByZero', result['stderr'])

    def test_timeout_reaps_process_and_next_run_works(self):
        self.runner.timeout = .001
        self.assertEqual(self.runner.run('loop {};', 'interpreter')['reason'], 'timeout')
        self.runner.timeout = 6
        self.assertEqual(self.runner.run('std.print(3);', 'interpreter')['stdout'], '3\n')
        self.assertFalse(self.runner.status()['busy'])

    def test_output_is_bounded(self):
        result = self.runner.run('loop { std.print("abcdefghijklmnopqrstuvwxyz0123456789"); };', 'interpreter')
        self.assertEqual(result['reason'], 'output_limit')
        self.assertLessEqual(len(result['stdout'].encode()) + len(result['stderr'].encode()), 64*1024)

    def test_cancel_reaps_active_process(self):
        results = []
        worker = threading.Thread(target=lambda: results.append(self.runner.run('std.time.sleep(5000);', 'interpreter')))
        worker.start()
        deadline = time.monotonic() + 2
        while self.runner.active is None and time.monotonic() < deadline:
            time.sleep(.005)
        self.assertTrue(self.runner.cancel())
        worker.join(3)
        self.assertFalse(worker.is_alive())
        self.assertEqual(results[0]['reason'], 'cancelled')
        self.assertFalse(self.runner.status()['busy'])


if __name__ == '__main__':
    unittest.main()
