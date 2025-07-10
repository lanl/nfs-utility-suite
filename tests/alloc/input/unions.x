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

enum Cases {
    one = 1,
    two = 2,
    three = 3
};

union StuffOrPlant switch (Cases hello) {
case one:
    Stuff things;
case two:
    PlantKind plant_kind;
case three:
    Plant plant;
};
