struct Another {
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
