enum Val {
    one = 1,
    two = 2,
    three = 3
};

struct Another {
    Val val;
	hyper x;
	unsigned hyper y;
};

struct Bar {
	unsigned int a;
	Another one;
	int b;
};

struct Foo {
	int a;
	Bar blah;
	unsigned int b;
	bool no;
	bool yes;
};

struct Simple {
	int a;
	unsigned int b;
	hyper c;
	unsigned hyper d;
};


struct Container {
	Simple first;
	bool middle;
	Simple last;
};

struct Int {
	int a;
};

struct Uint {
	unsigned int a;
};

struct Hyper {
	hyper a;
};

struct Uhyper {
	unsigned hyper a;
};

struct Bool {
	bool a;
};

typedef int my_int_type;

struct HasTypedef {
	my_int_type blah;
};


typedef unsigned hyper uint64;
typedef unsigned int uint32;

struct SpecData {
	uint32     specdata1;
	uint32     specdata2;
};

struct NfsTime {
	uint32   seconds;
	uint32   nseconds;
};

enum FileType {
	Reg    = 1,
	Dir    = 2,
	Blk    = 3,
	Chr    = 4,
	Lnk    = 5,
	Sock   = 6,
	Fifo   = 7
};

struct FileAttributes {
	FileType  typ;
	uint32    mode;
	uint32    nlink;
	uint32    uid;
	uint32    gid;
	uint64    size;
	uint64    used;
	uint32    rdev_1;
	uint32    rdev_2;
	uint32    fsid_major;
	uint32    fsid_minor;
	uint64    fileid;
	uint32    atime_s;
	uint32    atime_ns;
	uint32    mtime_s;
	uint32    mtime_ns;
	uint32    ctime_s;
	uint32    ctime_ns;
};
