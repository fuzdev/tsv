// line comment trailing `(` - function type
type Fn = (
	// c
	p: T
) => void;

// line comment trailing `(` - function type, multiple params
type Fn2 = (
	// c
	a: T,
	b: U
) => void;

// line comment trailing `(` - constructor type
type Ctor = new (
	// c
	p: T
) => void;

// an author blank BELOW the pulled comment survives — the blank is authorship, not
// the container's leading gap (the blank ABOVE one stays erased, against the delimiter)
type Fn3 = (
	// c

	p: T
) => void;
