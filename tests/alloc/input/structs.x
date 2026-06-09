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

struct Int {
	int a;
};

struct Uint {
	unsigned int a;
};

struct Hyper {
	hyper a;
};

struct Uhyper {
	unsigned hyper a;
};

struct Bool {
	bool a;
};
