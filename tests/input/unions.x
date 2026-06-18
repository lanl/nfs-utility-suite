enum PlantKind {
	Tree = 0,
	Grass = 1,
	Flower = 2
};

union Plant switch (PlantKind kind) {
case Tree:
	int num_branches;
case Grass:
	int num_seeds;
case Flower:
	int num_petals;
};

union NumLeaves switch (bool is_plant) {
case TRUE:
    unsigned int leaves;
default:
    void;
};

union MaybeAPlantKind switch (bool yes) {
case TRUE:
    PlantKind plant_kind;
case FALSE:
    void;
};

struct Stuff {
    int a;
    unsigned hyper b;
};

union MaybeStuff switch (bool yes) {
case TRUE:
    Stuff things;
case FALSE:
    void;
};

union HasString switch (bool yes) {
case TRUE:
    string str<>;
case FALSE:
    void;
};

enum Cases {
    one = 1,
    two = 2,
    three = 3,
    four = 4
};

union StuffOrPlant switch (Cases hello) {
case one:
    Stuff things;
case two:
    PlantKind plant_kind;
case three:
    Plant plant;
};

struct SameWidthDifferentStuff {
    int a;
    int b;
    int c;
};

union StuffOrPlant2 switch (Cases hello) {
case one:
    Stuff things;
case two:
    SameWidthDifferentStuff differentThings;
case three:
    Stuff sameThings;
case four:
    int hi;
default:
    Cases dead;
};

struct HasUnion {
    StuffOrPlant a;
    StuffOrPlant2 b;
    NumLeaves c;
    Plant nocache;
};

union Bar switch (Cases blah) {
case one:
        int a;
case two:
        void;
};

union AnOption switch (bool yes) {
case TRUE:
    int a;
case FALSE:
    void;
};

union Foo switch(Cases blah) {
case one:
        int *a;
case two:
        void;
};
