struct OpaqueArrays {
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

struct Strings {
	string lim<10>;
	string unlim<>;
};

struct AnInt {
	unsigned int a;
};

struct IntArrays {
	AnInt fixed[4];
	AnInt limited<7>;
	AnInt unlimited<>;
};
