struct Foo {
	int a;
	unsigned int b;
	hyper c;
	unsigned hyper d;
};

struct Container {
	Foo first;
	bool middle;
	Foo last;
};

typedef int my_int_type;

struct HasTypedef {
	my_int_type blah;
};
