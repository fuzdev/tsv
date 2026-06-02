// line comment trailing `(` - function type
type Fn = ( // c
	p: T,
) => void;

// line comment trailing `(` - function type, multiple params
type Fn2 = ( // c
	a: T,
	b: U,
) => void;

// line comment trailing `(` - constructor type
type Ctor = new ( // c
	p: T,
) => void;
