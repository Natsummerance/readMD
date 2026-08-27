import sys, threading
sys.path.insert(0, '.')
import readmd

srv = readmd.start_server(8765)
print('READY http://127.0.0.1:%d/' % srv.server_port, flush=True)
threading.Event().wait()
