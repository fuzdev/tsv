// an own-line directive between `<` and the type stays own-line and freezes the type
const a = <
	// prettier-ignore
	{x:   1}
>value;

// a plain own-line comment keeps its position and the type formats normally
const b = <
	// c
	{ x: 1 }
>value;
