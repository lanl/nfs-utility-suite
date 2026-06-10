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
