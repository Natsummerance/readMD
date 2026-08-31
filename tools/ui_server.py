"""Minimal foreground server used by Playwright's webServer fixture."""
import os
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

# Data paths are computed from READMD_DATA_DIR at import time; point them at a
# throwaway dir so tests never read or write real user data.
os.environ.setdefault('READMD_DATA_DIR', tempfile.mkdtemp(prefix='readmd-ui-test-'))

import readmd


port = int(sys.argv[1]) if len(sys.argv) > 1 else int(os.environ.get('READMD_UI_PORT', '28473'))
server = readmd.start_server(port)
print("ReadMD UI test server ready", flush=True)
try:
    while True:
        time.sleep(60)
finally:
    server.shutdown()
    server.server_close()
