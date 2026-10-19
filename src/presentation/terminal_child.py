"""Give the demonstration program its own controlling terminal and limits."""
import fcntl
import os
import resource
import sys
import termios

os.setsid()
fcntl.ioctl(0, termios.TIOCSCTTY, 0)
resource.setrlimit(resource.RLIMIT_AS, (1024**3, 1024**3))
resource.setrlimit(resource.RLIMIT_CPU, (45, 46))
resource.setrlimit(resource.RLIMIT_FSIZE, (2 * 1024**2, 2 * 1024**2))
resource.setrlimit(resource.RLIMIT_NOFILE, (128, 128))
resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
os.execv(sys.argv[1], sys.argv[1:])
