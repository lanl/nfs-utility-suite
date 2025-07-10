typedef int uid;
typedef string filename<>;
typedef opaque data<1024>;

struct TimestampsData {
    int atime;
    int ctime;
    int mtime;
};

typedef TimestampsData Timestamps;

struct File {
    uid owner;
    filename name;
    data contents;
    Timestamps t;
};
