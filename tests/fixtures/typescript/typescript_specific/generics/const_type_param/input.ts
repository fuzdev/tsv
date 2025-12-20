function literal<const T>(value: T): T {
	return value;
}

function tuple<const T extends readonly unknown[]>(items: T): T {
	return items;
}

const a = literal('hello');
const b = tuple([1, 2, 3]);

const arrow = <const T>(x: T) => x;
