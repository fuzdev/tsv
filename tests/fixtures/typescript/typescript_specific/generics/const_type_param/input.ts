// const type parameter - pure TypeScript (no trailing comma needed for arrow functions)
// See const_type_param_svelte/ for Svelte version with trailing comma

function literal<const T>(value: T): T {
	return value;
}

function tuple<const T extends readonly unknown[]>(items: T): T {
	return items;
}

const a = literal('');
const b = tuple([1, 2, 3]);

const arrow = <const T>(x: T) => x;
