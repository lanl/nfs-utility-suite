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
	Strings many[4];
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
