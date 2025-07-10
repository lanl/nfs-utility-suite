struct ListNode {
    int data;
    ListNode *next;
};

struct ListBegin {
    ListNode *list;
};

struct NonRecursive {
    int stuff;
};

struct JustAnOption {
    NonRecursive *maybe;
};

/* Example optional types from mount protocol: */

const MNTPATHLEN = 1024;  /* Maximum bytes in a path name */
const MNTNAMLEN  = 255;   /* Maximum bytes in a name */
const FHSIZE3    = 64;    /* Maximum bytes in a V3 file handle */

typedef opaque fhandle3<FHSIZE3>;
typedef string dirpath<MNTPATHLEN>;
typedef string name<MNTNAMLEN>;

typedef struct groupnode *groups;

struct groupnode {
   name     gr_name;
   groups   gr_next;
};

struct exports {
   exportnode *inner;
};

struct exportnode {
   dirpath  ex_dir;
   groups   ex_groups;
   exportnode *ex_next;
};
