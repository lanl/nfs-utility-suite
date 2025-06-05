enum AuthFlavor {
    None       = 0,
    Sys        = 1,
    Short      = 2,
    DH         = 3,
    RpcsecGss  = 6
};

struct OpaqueAuth {
    AuthFlavor flavor;
    opaque body<400>;
};

enum MessageType {
    Call  = 0,
    Reply = 1
};

enum ReplyStatus {
    Accepted = 0,
    Denied   = 1
};

enum AcceptStatus {
    Success      = 0,
    ProgUnavail  = 1,
    ProgMismatch = 2,
    ProcUnavail  = 3,
    GarbageArgs  = 4,
    SystemErr    = 5
};

enum RejectStatus {
    RpcMismatch = 0,
    AuthError   = 1
};

enum AuthStat {
    Ok           = 0,
    BadCred      = 1,
    RejectedCred = 2,
    BadVerf      = 3,
    RejectedVerf = 4,
    TooWeak      = 5,
    InvalidResp  = 6,
    Failed       = 7,
    KerbGeneric = 8,
    TimeExpire =  9,
    TktFile =    10,
    Decode =     11,
    NetAddr =    12,
    RpcsecGssCredProblem = 13,
    RpcsecGssCtxProblem =  14
};

union RpcMessageBody switch (MessageType mtype) {
case Call:
    CallBody cbody;
case Reply:
    ReplyBody rbody;
};

struct RpcMessage {
    unsigned int xid;
    RpcMessageBody body;
};

struct CallBody {
   unsigned int rpcvers;
   unsigned int prog;
   unsigned int vers;
   unsigned int proc;
   OpaqueAuth cred;
   OpaqueAuth verf;
};

union ReplyBody switch (ReplyStatus stat) {
case Accepted:
    AcceptedReply areply;
case Denied:
    RejectedReply rreply;
};

struct ProgMismatchBody {
    unsigned int low;
    unsigned int high;
};

union AcceptedReplyBody switch (AcceptStatus stat) {
case Success:
    opaque results[0];
case ProgMismatch:
    ProgMismatchBody mismatch_info;
case ProgUnavail:
    void;
case ProcUnavail:
    void;
case GarbageArgs:
    void;
case SystemErr:
    void;
};

struct AcceptedReply {
    OpaqueAuth verf;
    AcceptedReplyBody reply_data;
};

struct RpcMismatchBody {
    unsigned int low;
    unsigned int high;
};

union RejectedReply switch (RejectStatus stat) {
case RpcMismatch:
    RpcMismatchBody mismatch_info;
case AuthError:
    AuthStat stat;
};
