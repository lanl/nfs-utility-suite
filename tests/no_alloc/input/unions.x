union MyOption switch (bool yes) {
case TRUE:
    int data;
case FALSE:
    void;
};

enum Cases {
	one = 1,
	two = 2,
	three = 3
};

union Stuff switch (Cases blah) {
case one:
	int a;
case two:
	MyOption b;
case three:
	void;
};

union Things switch (Cases blah) {
case one:
	int a;
case two:
	int b;
case three:
	int c;
default:
	void;
};

union MoreThings switch (Cases blah) {
case one:
	int a;
case two:
	int b;
case three:
	int c;
default:
	int d;
};
