Intro
=====

Panopticon is a cross platform disassembler for reverse engineering.
It consists of a C++ library for disassembling, analysing decompiling
and patching binaries for various platforms and instruction sets.

Panopticon comes with GUI for browsing control flow graphs, displaying
analysis results, controlling debugger instances and editing the on-disk
as well as in-memory representation of the program.

Building
========

In order to compile Panopticon the following needs to be installed first:

- Qt 5.3
- CMake 2.8
- g++ 4.7 or Clang 3.4
- Boost 1.53
- Kyoto Cabinet 1.2.76
- libarchive 3.1.2
- googletest 1.6.0 (only needed to run the test suite)

Linux
-----

After cloning the repository onto disk, create a build directory and
call cmake and the path to the source as argument. Compile the project
using GNU Make.

```bash
git clone https://github.com/das-labor/panopticon.git
mkdir panop-build
cd panop-build
cmake ../panopticon
make -j4
sudo make install
```

Windows
-------

After installing the prerequisites on Windows use the CMake GUI to
generate Visual Studio project files or Mingw Makefiles. Panopticon
can be compiled using VC++ 2013 or Mingw g++.

Contributing
============

Panopticon is licensed under GPLv3 and is Free Software. Hackers are
always welcome. See http://panopticon.re for our wiki and issue tracker.