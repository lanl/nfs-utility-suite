typedef unsigned hyper uint64;
typedef hyper int64;
typedef unsigned long uint32;
typedef long int32;
typedef string FileName<>;
typedef string NfsPath<>;
typedef uint64 FileId;
typedef uint64 Cookie;
typedef opaque CookieVerf[NFS3_COOKIEVERFSIZE];
typedef opaque WriteVerf[NFS3_WRITEVERFSIZE];
typedef opaque CreateVerf[NFS3_CREATEVERFSIZE];
typedef uint32 Uid;
typedef uint32 Gid;
typedef uint64 Size;
typedef uint64 Offset;
typedef uint32 Mode;
typedef uint32 Count;

const FHSIZE = 64;
const COOKIEVERFSIZE = 8;
const CREATEVERFSIZE = 8;
const WRITEVERFSIZE = 8;

enum NfsResult {
	Ok          = 0,
	Perm        = 1,
	NoEnt       = 2,
	Io          = 5,
	Nxio        = 6,
	Acces       = 13,
	Exist       = 17,
	XDev        = 18,
	NoDev       = 19,
	NotDir      = 20,
	IsDir       = 21,
	Inval       = 22,
	FBig        = 27,
	NoSpc       = 28,
	RoFs        = 30,
	MLink       = 31,
	NameTooLong = 63,
	NotEmpty    = 66,
	Dquot       = 69,
	Stale       = 70,
	Remote      = 71,
	BadHandle   = 10001,
	NotSync     = 10002,
	BadCookie   = 10003,
	NotSupp     = 10004,
	TooSmall    = 10005,
	ServerFault = 10006,
	Badtype     = 10007,
	Jukebox     = 10008
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

struct SpecData {
	uint32     specdata1;
	uint32     specdata2;
};

struct NfsTime {
	uint32   seconds;
	uint32   nseconds;
};

struct FileAttributes {
	FileType  type;
	Mode      mode;
	uint32    nlink;
	Uid       uid;
	Gid       gid;
	Size      size;
	Size      used;
	SpecData  rdev;
	uint64    fsid;
	FileId    fileid;
	NfsTime   atime;
	NfsTime   mtime;
	NfsTime   ctime;
};

struct FileHandle {
	opaque       data<FHSIZE>;
};

struct GetAttrArgs {
	FileHandle  object;
};

struct GetAttrSuccess {
	FileAttributes   obj_attributes;
};

union GetAttrResult switch (NfsResult status) {
case Ok:
	GetAttrSuccess  resok;
default:
	void;
};
program NFS_PROGRAM {
	version NFS_V3 {
		void NFSPROC3_NULL(void)                    = 0;
		GetAttrResult NFSPROC3_GETATTR(GetAttrArgs) = 1;
	} = 3;
} = 100003;
