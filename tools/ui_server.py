"""Minimal foreground server used by Playwright's webServer fixture."""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
import readmd


server = readmd.start_server(28473)
print("ReadMD UI test server ready", flush=True)
try:
    while True:
        time.sleep(60)
finally:
    server.shutdown()
    server.server_close()
