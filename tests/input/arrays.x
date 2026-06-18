struct OpaqueArrays {
	opaque bytes[3];
	opaque bytes_2<3>;
	opaque bytes_3<>;
};

struct AnInt {
	unsigned int a;
};

struct IntArrays {
	AnInt fixed[4];
	AnInt limited<7>;
	AnInt unlimited<>;
};

struct Strings {
	string str<7>;
	string str_2<>;
};

struct ManyStrings {
    Strings first;
	Strings many[4];
    Strings last;
};

const AMOUNT = 5;
struct IdentifierArray {
	opaque bytes[AMOUNT];
	string str<AMOUNT>;
	int ints<AMOUNT>;
};

struct ManyInts {
	unsigned hyper first[2];
	int second<7>;
	hyper third<>;
};

struct ConstSizeArray {
    opaque bytes[AMOUNT];
    int ints[AMOUNT];
};

struct FixedOpaqueArrays {
	opaque a[1];
	opaque b[2];
	opaque c[3];
	opaque d[4];
};

struct LimitedOpaqueArrays {
	opaque a<1>;
	opaque b<2>;
	opaque c<3>;
	opaque d<4>;
	opaque e<7>;
};

struct UnlimitedOpaqueArray {
	opaque data<>;
};

struct LimitedOpaqueArray {
    string data<4>;
};

struct UnlimitedArrayOfLimited {
    LimitedOpaqueArray a<>;
};
