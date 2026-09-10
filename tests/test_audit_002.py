import sys
import ctypes
import pytest
from ctypes import wintypes
from src.readmd_modules import code_chunk_runner as runner

@pytest.mark.skipif(sys.platform != 'win32', reason='Windows FFI')
def test_reproduce_bug_002():
    assert runner._kernel32.CreateJobObjectW.restype is wintypes.HANDLE
    assert runner._kernel32.AssignProcessToJobObject.argtypes == [
        wintypes.HANDLE, wintypes.HANDLE]
