// const type parameter - pure TypeScript (arrow type params stay bare)
// See const_type_param_arrow_prettier_divergence/ for the Svelte arrow (tsv bare, prettier `<const T,>`)

function literal<const T>(value: T): T {
	return value;
}

function tuple<const T extends readonly unknown[]>(items: T): T {
	return items;
}

const a = literal('');
const b = tuple([1, 2, 3]);

const arrow = <const T>(x: T) => x;
