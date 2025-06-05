struct RpcbString {
    string contents<>;
};

struct RpcService {
    unsigned long prog;
    unsigned long vers;
    string netid<>;
    string addr<>;
    string owner<>;
};

struct RpcbindItem {
    RpcService rpcb_map;
    struct RpcbindItem *rpcb_next;
};

struct RpcbindList {
    RpcbindItem *items;
};

program RPCBPROG {
 version RPCBVERS {
     RpcbString
     RPCBPROC_GETADDR(RpcService) = 3;

     RpcbindList
     RPCBPROC_DUMP(void) = 4;
 } = 3;
} = 100000;
