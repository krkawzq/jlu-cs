"""Apply per-demo process limits before replacing the helper with Prim."""
import os
import resource
import sys

resource.setrlimit(resource.RLIMIT_AS, (1024**3, 1024**3))
resource.setrlimit(resource.RLIMIT_CPU, (4, 5))
resource.setrlimit(resource.RLIMIT_FSIZE, (256 * 1024, 256 * 1024))
resource.setrlimit(resource.RLIMIT_NOFILE, (64, 64))
resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
os.execv(sys.argv[1], sys.argv[1:])
