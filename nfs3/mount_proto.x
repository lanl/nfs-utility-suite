const MNTPATHLEN = 1024;  /* Maximum bytes in a path name */
const MNTNAMLEN  = 255;   /* Maximum bytes in a name */
const FHSIZE3    = 64;    /* Maximum bytes in a V3 file handle */

typedef opaque FileHandle<FHSIZE3>;
typedef string DirPath<MNTPATHLEN>;
typedef string Name<MNTNAMLEN>;

enum MountStatus {
    Ok = 0,                 /* no error */
    Perm = 1,            /* Not owner */
    NoEnt = 2,           /* No such file or directory */
    Io = 5,              /* I/O error */
    Access = 13,          /* Permission denied */
    NotDir = 20,         /* Not a directory */
    Inval = 22,          /* Invalid argument */
    NameTooLong = 63,    /* Filename too long */
    NotSupp = 10004,     /* Operation not supported */
    ServerFault = 10006  /* A failure on the server */
};

struct MountResultOk {
    FileHandle   fhandle;
    int        auth_flavors<>;
};

union MountResult switch (MountStatus fhs_status) {
case Ok:
    MountResultOk  mountinfo;
default:
    void;
};

struct MountBody {
    Name       hostname;
    DirPath    directory;
    MountBody  *next;
};

struct MountList {
    MountBody *inner;
};

struct GroupNode {
    Name     name;
    GroupNode  *next;
};

struct Groups {
    GroupNode *inner;
};

struct ExportNode {
    DirPath  dir;
    Groups   groups;
    ExportNode *next;
};

struct Exports {
    ExportNode *inner;
};

program MOUNT_PROGRAM {
   version MOUNT_V3 {
        void      MOUNTPROC3_NULL(void)    = 0;
        MountResult MOUNTPROC3_MNT(dirpath)  = 1;
        MountList MOUNTPROC3_DUMP(void)    = 2;
        void      MOUNTPROC3_UMNT(dirpath) = 3;
        void      MOUNTPROC3_UMNTALL(void) = 4;
        Exports   MOUNTPROC3_EXPORT(void)  = 5;
    } = 3;
} = 100005;
