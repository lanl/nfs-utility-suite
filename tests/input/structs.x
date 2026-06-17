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

struct Simple {
	int a;
	unsigned int b;
	hyper c;
	unsigned hyper d;
};


struct Container {
	Simple first;
	bool middle;
	Simple last;
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

typedef int my_int_type;

struct HasTypedef {
	my_int_type blah;
};
